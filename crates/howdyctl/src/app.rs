//! The interactive ratatui application: a tabbed manager for Howdy.
use std::time::Duration;

use howdy::config::Config;
use howdy::doctor::{self, Status};
use howdy::model::{self, Model};
use howdy::test::{self, TestResult};
use howdy::{camera, Camera};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::ui::{self, ACCENT, AQUA, BAD, DIM, OK, TEXT, WARN};

const TICK: Duration = Duration::from_millis(200);

/// How far the unfocused pane is faded toward the background (0 = none, 1 = gone).
/// ~0.35 ≈ 65% opacity.
const UNFOCUSED_FADE: f64 = 0.35;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Cameras,
    Models,
    Test,
    Doctor,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Cameras, Tab::Models, Tab::Test, Tab::Doctor];
    fn label(self) -> &'static str {
        match self {
            Tab::Cameras => "Cameras",
            Tab::Models => "Models",
            Tab::Test => "Test",
            Tab::Doctor => "Doctor",
        }
    }
    /// Title shown on the content panel.
    fn title(self) -> &'static str {
        match self {
            Tab::Cameras => "Cameras",
            Tab::Models => "Face models",
            Tab::Test => "Recognition test",
            Tab::Doctor => "Doctor",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
}

/// Which pane currently has the keyboard.
#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Menu,
    Content,
}

struct App {
    demo: bool,
    user: String,
    tab: Tab,
    focus: Focus,
    quit: bool,
    status: String,
    /// `Some(buffer)` while the user is typing a label for a new model.
    enrolling: Option<String>,

    cameras: Vec<Camera>,
    cam_sel: usize,

    models: Vec<Model>,
    model_sel: usize,

    device_path: String,
    certainty: f64,
    pending_certainty: f64,
    timeout: u32,
    end_report: bool,

    last_test: Option<TestResult>,
    checks: Vec<doctor::Check>,
}

impl App {
    fn new(demo: bool, user: String) -> Self {
        App {
            demo,
            user,
            tab: Tab::Cameras,
            focus: Focus::Menu,
            quit: false,
            status: "ready".into(),
            enrolling: None,
            cameras: Vec::new(),
            cam_sel: 0,
            models: Vec::new(),
            model_sel: 0,
            device_path: String::new(),
            certainty: 3.5,
            pending_certainty: 3.5,
            timeout: 5,
            end_report: false,
            last_test: None,
            checks: Vec::new(),
        }
    }

    // ---- data loading ------------------------------------------------------

    fn load(&mut self) {
        if self.demo {
            self.load_demo();
            return;
        }
        self.reload_config();
        self.reload_cameras();
        self.reload_models();
        self.reload_doctor();
    }

    fn load_demo(&mut self) {
        self.device_path = "/dev/video2".into();
        self.certainty = 4.0;
        self.pending_certainty = 4.0;
        self.timeout = 8;
        self.end_report = true;
        self.cameras = vec![
            Camera {
                path: "/dev/video0".into(),
                index: 0,
                name: "Integrated FHD webcam".into(),
                is_ir: false,
                can_capture: true,
                accessible: true,
            },
            Camera {
                path: "/dev/video2".into(),
                index: 2,
                name: "Integrated IR camera".into(),
                is_ir: true,
                can_capture: true,
                accessible: true,
            },
        ];
        self.models = vec![
            Model {
                id: 0,
                label: "Initial model".into(),
                time: 1_781_031_338,
            },
            Model {
                id: 1,
                label: "No glasses".into(),
                time: 1_781_031_810,
            },
        ];
        self.last_test = Some(TestResult {
            matched: true,
            exit_code: 0,
            threshold: 4.0,
            distance: Some(2.9),
            frames: Some(118),
            fps: Some(14.8),
            message: "Face matched".into(),
        });
        self.checks = doctor_demo();
        self.status = "demo mode — no changes are written".into();
    }

    fn reload_config(&mut self) {
        if let Ok(c) = Config::load() {
            self.device_path = c.get("device_path").unwrap_or_default();
            self.certainty = c.get_f64("certainty").unwrap_or(3.5);
            self.timeout = c.get_u32("timeout").unwrap_or(5);
            self.end_report = c.get_bool("end_report").unwrap_or(false);
        }
        self.pending_certainty = self.certainty;
    }

    fn reload_cameras(&mut self) {
        self.cameras = camera::detect();
        self.cam_sel = self.cam_sel.min(self.cameras.len().saturating_sub(1));
    }

    fn reload_models(&mut self) {
        self.models = model::list(&self.user).unwrap_or_default();
        self.model_sel = self.model_sel.min(self.models.len().saturating_sub(1));
    }

    fn reload_doctor(&mut self) {
        self.checks = doctor::run(&self.user);
    }

    // ---- event loop --------------------------------------------------------

    fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        while !self.quit {
            terminal.draw(|f| self.draw(f))?;
            if event::poll(TICK)? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        self.on_key(k.code, terminal);
                    }
                }
            }
        }
        Ok(())
    }

    fn on_key(&mut self, code: KeyCode, terminal: &mut DefaultTerminal) {
        // While typing a model label, the keyboard belongs to the text field.
        if self.enrolling.is_some() {
            self.on_label_key(code, terminal);
            return;
        }
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Esc => {
                if self.focus == Focus::Content {
                    self.focus = Focus::Menu;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Tab | KeyCode::BackTab => self.toggle_focus(),
            KeyCode::Char('r') => {
                self.load();
                self.status = "refreshed".into();
            }
            // navigation depends on which pane has focus
            KeyCode::Up if self.focus == Focus::Menu => self.menu_step(-1),
            KeyCode::Down if self.focus == Focus::Menu => self.menu_step(1),
            KeyCode::Up => self.move_sel(-1),
            KeyCode::Down => self.move_sel(1),
            KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Menu => {
                self.focus = Focus::Content;
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Content => {
                self.focus = Focus::Menu;
            }
            KeyCode::Enter if self.focus == Focus::Menu => self.focus = Focus::Content,
            other if self.focus == Focus::Content => self.on_content_key(other, terminal),
            _ => {}
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Menu => Focus::Content,
            Focus::Content => Focus::Menu,
        };
    }

    /// Move the menu selection (the active section), wrapping; content follows live.
    fn menu_step(&mut self, delta: i32) {
        let n = Tab::ALL.len() as i32;
        let i = (self.tab.index() as i32 + delta).rem_euclid(n) as usize;
        self.tab = Tab::ALL[i];
    }

    fn on_content_key(&mut self, code: KeyCode, terminal: &mut DefaultTerminal) {
        match self.tab {
            Tab::Cameras => {
                if code == KeyCode::Enter {
                    self.set_active_camera(terminal);
                }
            }
            Tab::Models => match code {
                KeyCode::Char('a') => self.begin_enroll(),
                KeyCode::Char('d') | KeyCode::Delete => self.delete_model(terminal),
                _ => {}
            },
            Tab::Test => match code {
                KeyCode::Enter => self.run_test(terminal),
                KeyCode::Char('+') | KeyCode::Char('=') => self.nudge_certainty(0.1),
                KeyCode::Char('-') | KeyCode::Char('_') => self.nudge_certainty(-0.1),
                KeyCode::Char('s') => self.save_certainty(terminal),
                KeyCode::Char('e') => self.toggle_end_report(terminal),
                _ => {}
            },
            Tab::Doctor => match code {
                KeyCode::Enter => {
                    self.reload_doctor();
                    self.status = "re-ran checks".into();
                }
                KeyCode::Char('f') => self.run_fix(terminal),
                _ => {}
            },
        }
    }

    fn move_sel(&mut self, delta: i32) {
        let len = match self.tab {
            Tab::Cameras => self.cameras.len(),
            Tab::Models => self.models.len(),
            _ => return,
        };
        if len == 0 {
            return;
        }
        let sel = match self.tab {
            Tab::Cameras => &mut self.cam_sel,
            Tab::Models => &mut self.model_sel,
            _ => return,
        };
        *sel = (*sel as i32 + delta).clamp(0, len as i32 - 1) as usize;
    }

    fn nudge_certainty(&mut self, delta: f64) {
        self.pending_certainty = (self.pending_certainty + delta).clamp(0.0, 10.0);
        // round to one decimal to avoid float drift
        self.pending_certainty = (self.pending_certainty * 10.0).round() / 10.0;
    }

    // ---- actions -----------------------------------------------------------

    fn run_test(&mut self, terminal: &mut DefaultTerminal) {
        if self.demo {
            self.status = "demo mode — test disabled".into();
            return;
        }
        // draw a "scanning" frame before we block on the camera
        self.status = "scanning — look at the camera…".into();
        let _ = terminal.draw(|f| self.draw(f));
        match test::run(&self.user) {
            Ok(r) => {
                self.status = r.message.clone();
                self.last_test = Some(r);
            }
            Err(e) => self.status = format!("test error: {e}"),
        }
    }

    fn set_active_camera(&mut self, terminal: &mut DefaultTerminal) {
        let Some(cam) = self.cameras.get(self.cam_sel) else {
            return;
        };
        if !cam.can_capture {
            self.status = "that node can't capture video — pick a capture device".into();
            return;
        }
        let path = cam.path.to_string_lossy().into_owned();
        if self.guard_demo() {
            return;
        }
        let user = self.user.clone();
        if self.privileged(terminal, &["--user", &user, "set-camera", &path], false) {
            self.reload_config();
            self.status = format!("camera set to {path}");
        }
    }

    /// Start the inline label prompt for a new model.
    fn begin_enroll(&mut self) {
        self.enrolling = Some(String::new());
        self.status = "type a label, Enter to enroll (empty = default), Esc to cancel".into();
    }

    /// Keystrokes while the label field is active.
    fn on_label_key(&mut self, code: KeyCode, terminal: &mut DefaultTerminal) {
        match code {
            KeyCode::Esc => {
                self.enrolling = None;
                self.status = "enrollment cancelled".into();
            }
            KeyCode::Enter => {
                let label = self.enrolling.take().unwrap_or_default();
                self.do_enroll(terminal, label.trim().to_string());
            }
            KeyCode::Backspace => {
                if let Some(buf) = self.enrolling.as_mut() {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = self.enrolling.as_mut() {
                    buf.push(c);
                }
            }
            _ => {}
        }
    }

    /// Run the (privileged) enrollment, passing `label` if non-empty.
    fn do_enroll(&mut self, terminal: &mut DefaultTerminal, label: String) {
        if self.guard_demo() {
            return;
        }
        let user = self.user.clone();
        let mut args = vec!["--user", &user, "add"];
        if !label.is_empty() {
            args.push(&label);
        }
        let ok = self.privileged(terminal, &args, true);
        self.reload_models();
        self.status = if !ok {
            "enrollment cancelled or failed".into()
        } else if label.is_empty() {
            "enrolled a new model".into()
        } else {
            format!("enrolled '{label}'")
        };
    }

    fn delete_model(&mut self, terminal: &mut DefaultTerminal) {
        let Some(m) = self.models.get(self.model_sel) else {
            self.status = "no model selected".into();
            return;
        };
        let id = m.id.to_string();
        if self.guard_demo() {
            return;
        }
        let user = self.user.clone();
        if self.privileged(terminal, &["--user", &user, "remove", &id], false) {
            self.reload_models();
            self.status = format!("removed model {id}");
        }
    }

    fn save_certainty(&mut self, terminal: &mut DefaultTerminal) {
        if self.guard_demo() {
            return;
        }
        let val = format!("{:.1}", self.pending_certainty);
        let user = self.user.clone();
        if self.privileged(terminal, &["--user", &user, "certainty", &val], false) {
            self.reload_config();
            self.status = format!("certainty set to {val}");
        }
    }

    fn run_fix(&mut self, terminal: &mut DefaultTerminal) {
        if self.guard_demo() {
            return;
        }
        let user = self.user.clone();
        let ok = self.privileged(terminal, &["--user", &user, "doctor", "--fix"], true);
        self.reload_doctor();
        self.status = if ok {
            "ran doctor --fix".into()
        } else {
            "doctor --fix failed".into()
        };
    }

    fn toggle_end_report(&mut self, terminal: &mut DefaultTerminal) {
        if self.guard_demo() {
            return;
        }
        let next = (!self.end_report).to_string();
        let user = self.user.clone();
        if self.privileged(
            terminal,
            &["--user", &user, "set-config", "end_report", &next],
            false,
        ) {
            self.reload_config();
            self.status = format!(
                "detailed report {}",
                if self.end_report { "on" } else { "off" }
            );
        }
    }

    fn guard_demo(&mut self) -> bool {
        if self.demo {
            self.status = "demo mode — no changes are written".into();
        }
        self.demo
    }

    /// Suspend the TUI, run `howdyctl <args>` with privilege, then restore.
    /// `interactive` keeps the inherited output on screen and waits for Enter.
    fn privileged(
        &mut self,
        terminal: &mut DefaultTerminal,
        args: &[&str],
        interactive: bool,
    ) -> bool {
        ratatui::restore();
        if interactive {
            println!("\n\x1b[1m▶ howdyctl: running a privileged action\x1b[0m");
            println!("  (a password prompt may appear)\n");
        }
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let result = crate::elevate::run(&owned);
        let ok = matches!(&result, Ok(s) if s.success());
        if interactive || !ok {
            if let Err(e) = &result {
                println!("\nfailed to run: {e}");
            }
            println!("\nPress Enter to return to howdyctl…");
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s);
        }
        *terminal = ratatui::init();
        ok
    }

    // ---- drawing -----------------------------------------------------------

    fn draw(&self, f: &mut Frame) {
        // transparent: no background fill, so the terminal paints one uniform colour
        let area = f.area().inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let [header, _gap, main, status, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        self.draw_header(f, header);

        // left Menu box + right Content box (with a 1-col gap between)
        let [menu, _gap2, content] = Layout::horizontal([
            Constraint::Length(18),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(main);

        self.draw_menu(f, menu);

        // both panels keep a green border; the unfocused one is faded as a whole
        let block = ui::panel(self.tab.title(), true);
        let cinner = block.inner(content).inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        f.render_widget(block, content);
        match self.tab {
            Tab::Cameras => self.draw_cameras(f, cinner),
            Tab::Models => self.draw_models(f, cinner),
            Tab::Test => self.draw_test(f, cinner),
            Tab::Doctor => self.draw_doctor(f, cinner),
        }

        // fade whichever pane is not focused (border + content alike)
        let buf = f.buffer_mut();
        match self.focus {
            Focus::Menu => ui::dim_rect(buf, content, UNFOCUSED_FADE),
            Focus::Content => ui::dim_rect(buf, menu, UNFOCUSED_FADE),
        }

        self.draw_status(f, status);
        self.draw_help(f, help);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let [left, right] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(22)]).areas(area);
        let title = Line::from(vec![
            Span::styled("◔‿◔", Style::default().fg(ACCENT)),
            Span::styled("  howdyctl", Style::default().fg(ACCENT).bold()),
        ]);
        f.render_widget(Paragraph::new(title), left);

        let who = if self.demo {
            format!("user: {} · demo", self.user)
        } else {
            format!("user: {}", self.user)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(who, Style::default().fg(DIM))))
                .alignment(Alignment::Right),
            right,
        );
    }

    fn draw_menu(&self, f: &mut Frame, area: Rect) {
        let block = ui::panel("Menu", true);
        let inner = block.inner(area).inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        f.render_widget(block, area);

        let items: Vec<ListItem> = Tab::ALL
            .iter()
            .map(|t| {
                ListItem::new(vec![
                    Line::from(Span::styled(
                        format!(" {}", t.label()),
                        Style::default().fg(TEXT),
                    )),
                    Line::from(""), // spacing between menu items
                ])
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.tab.index()));
        let list = List::new(items).highlight_style(ui::selection());
        f.render_stateful_widget(list, inner, &mut state);
    }

    fn draw_cameras(&self, f: &mut Frame, area: Rect) {
        if self.cameras.is_empty() {
            f.render_widget(Paragraph::new("no /dev/video* devices found").fg(DIM), area);
            return;
        }

        let [head, _gap, body] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        f.render_widget(
            Paragraph::new(ui::header(format!(
                "  {:<14}{:<18}{:<7}{}",
                "DEVICE", "NAME", "TYPE", "STATUS"
            ))),
            head,
        );

        let items: Vec<ListItem> = self
            .cameras
            .iter()
            .map(|c| {
                let active = c.path.to_string_lossy() == self.device_path;
                let dot = if active { "● " } else { "  " };
                let kind = if c.is_ir { "IR" } else { "RGB" };
                let kind_color = if c.is_ir { AQUA } else { DIM };
                let tag = if active {
                    Span::styled("active", Style::default().fg(ACCENT).bold())
                } else if !c.can_capture {
                    Span::styled("meta", Style::default().fg(DIM))
                } else {
                    Span::raw("")
                };
                let content = Line::from(vec![
                    Span::styled(dot, Style::default().fg(ACCENT)),
                    Span::styled(
                        format!("{:<14}", c.path.display()),
                        Style::default().fg(TEXT),
                    ),
                    Span::styled(
                        format!("{}  ", fit(tidy_name(&c.name), 16)),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(format!("{kind:<7}"), Style::default().fg(kind_color)),
                    tag,
                ]);
                // a trailing blank line gives breathing room between rows
                ListItem::new(vec![content, Line::from("")])
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.cam_sel));
        let list = List::new(items).highlight_style(ui::selection());
        f.render_stateful_widget(list, body, &mut state);
    }

    fn draw_models(&self, f: &mut Frame, area: Rect) {
        if self.models.is_empty() {
            f.render_widget(
                Paragraph::new("no models enrolled — press  a  to add one").fg(DIM),
                area,
            );
            return;
        }

        let [head, _gap, body] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        f.render_widget(
            Paragraph::new(ui::header(format!(
                "  {:<5}{:<20}{}",
                "ID", "ENROLLED", "LABEL"
            ))),
            head,
        );

        let items: Vec<ListItem> = self
            .models
            .iter()
            .map(|m| {
                let content = Line::from(vec![
                    Span::styled(format!("  {:<5}", m.id), Style::default().fg(DIM)),
                    Span::styled(
                        format!("{:<20}", model::format_time(m.time)),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(m.label.clone(), Style::default().fg(TEXT)),
                ]);
                ListItem::new(vec![content, Line::from("")])
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.model_sel));
        let list = List::new(items).highlight_style(ui::selection());
        f.render_stateful_widget(list, body, &mut state);
    }

    fn draw_test(&self, f: &mut Frame, area: Rect) {
        let inner = area;

        let pending = (self.pending_certainty - self.certainty).abs() > 1e-6;
        let thr_line = if pending {
            Line::from(vec![
                Span::styled("Threshold  ", Style::default().fg(DIM)),
                Span::styled(format!("{:.1}", self.certainty), Style::default().fg(TEXT)),
                Span::styled("  →  ", Style::default().fg(DIM)),
                Span::styled(
                    format!("{:.1} (unsaved — s to save)", self.pending_certainty),
                    Style::default().fg(WARN).bold(),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("Threshold  ", Style::default().fg(DIM)),
                Span::styled(format!("{:.1}", self.certainty), Style::default().fg(TEXT)),
                Span::styled("   lower = stricter", Style::default().fg(DIM)),
            ])
        };

        let shown = if pending {
            self.pending_certainty
        } else {
            self.certainty
        };
        let dist = self.last_test.as_ref().and_then(|r| r.distance);

        let mut lines = vec![thr_line, Line::from("")];
        lines.extend(ui::gauge(shown, dist));
        lines.push(Line::from(""));
        lines.push(self.test_result_line());
        if let Some(r) = &self.last_test {
            if let (Some(fr), Some(fps)) = (r.frames, r.fps) {
                lines.push(Line::from(Span::styled(
                    format!("frames {fr} @ {fps:.1} fps"),
                    Style::default().fg(DIM),
                )));
            }
            if r.distance.is_none() {
                lines.push(Line::from(Span::styled(
                    format!(
                        "tip: exact distance needs detailed report — it is {} (e to toggle)",
                        if self.end_report { "ON" } else { "OFF" }
                    ),
                    Style::default().fg(DIM).italic(),
                )));
            }
        }
        f.render_widget(Paragraph::new(lines), inner);
    }

    fn test_result_line(&self) -> Line<'static> {
        match &self.last_test {
            None => Line::from(Span::styled(
                "press  ↵  to run a recognition test",
                Style::default().fg(DIM),
            )),
            Some(r) if r.matched => {
                let mut spans = vec![Span::styled("✓ matched", Style::default().fg(OK).bold())];
                if let (Some(d), Some(m)) = (r.distance, r.margin()) {
                    spans.push(Span::styled(
                        format!("  best distance {d:.1}  (margin {m:+.1})"),
                        Style::default().fg(DIM),
                    ));
                }
                Line::from(spans)
            }
            Some(r) => Line::from(vec![
                Span::styled("✗ ", Style::default().fg(BAD).bold()),
                Span::styled(r.message.clone(), Style::default().fg(BAD)),
            ]),
        }
    }

    fn draw_doctor(&self, f: &mut Frame, area: Rect) {
        let inner = area;
        let lines: Vec<Line> = self
            .checks
            .iter()
            .map(|c| {
                let (glyph, color) = match c.status {
                    Status::Ok => ("✓", OK),
                    Status::Warn => ("!", WARN),
                    Status::Fail => ("✗", BAD),
                };
                Line::from(vec![
                    Span::styled(format!("{glyph}  "), Style::default().fg(color).bold()),
                    Span::styled(format!("{:<28}", c.name), Style::default().fg(TEXT)),
                    Span::styled(c.detail.clone(), Style::default().fg(DIM)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let line = if let Some(buf) = &self.enrolling {
            // active label-entry field, with a block cursor
            Line::from(vec![
                Span::styled(" label ▸ ", Style::default().fg(ACCENT).bold()),
                Span::styled(buf.clone(), Style::default().fg(TEXT)),
                Span::styled(
                    "▏",
                    Style::default()
                        .fg(ACCENT)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(" • ", Style::default().fg(ACCENT)),
                Span::styled(self.status.clone(), Style::default().fg(DIM)),
            ])
        };
        f.render_widget(Paragraph::new(line), area);
    }

    fn draw_help(&self, f: &mut Frame, area: Rect) {
        let hints: &[(&str, &str)] = if self.enrolling.is_some() {
            &[("type", "label"), ("↵", "enroll"), ("Esc", "cancel")]
        } else if self.focus == Focus::Menu {
            &[
                ("↑↓", "section"),
                ("⇥/→", "open"),
                ("r", "refresh"),
                ("q", "quit"),
            ]
        } else {
            match self.tab {
                Tab::Cameras => &[
                    ("↑↓", "select"),
                    ("↵", "set active"),
                    ("⇥/←", "menu"),
                    ("q", "quit"),
                ],
                Tab::Models => &[
                    ("↑↓", "select"),
                    ("a", "add"),
                    ("d", "delete"),
                    ("⇥/←", "menu"),
                    ("q", "quit"),
                ],
                Tab::Test => &[
                    ("↵", "run"),
                    ("± ", "threshold"),
                    ("s", "save"),
                    ("⇥/←", "menu"),
                    ("q", "quit"),
                ],
                Tab::Doctor => &[
                    ("↵", "re-check"),
                    ("f", "fix"),
                    ("⇥/←", "menu"),
                    ("q", "quit"),
                ],
            }
        };
        f.render_widget(Paragraph::new(ui::help_bar(hints)), area);
    }
}

/// Trim a kernel v4l2 card name down to its meaningful tail, e.g.
/// `"ASUS FHD webcam: ASUS IR camera"` → `"ASUS IR camera"`.
fn tidy_name(name: &str) -> &str {
    match name.rsplit_once(": ") {
        Some((_, tail)) if !tail.trim().is_empty() => tail.trim(),
        _ => name.trim(),
    }
}

/// Fit `s` into exactly `width` columns: left-pad if short, truncate with `…` if long.
fn fit(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        format!("{s:<width$}")
    } else {
        let cut: String = chars[..width.saturating_sub(1)].iter().collect();
        format!("{cut}…")
    }
}

fn doctor_demo() -> Vec<doctor::Check> {
    use howdy::doctor::Check;
    // Hand-built sample so `--demo` renders without touching the system.
    [
        ("Howdy installed", Status::Ok, "/lib/security/howdy"),
        (
            "dlib (face recognition)",
            Status::Ok,
            "importable under python3",
        ),
        ("OpenCV (cv2)", Status::Ok, "importable under python3"),
        ("dlib model data", Status::Ok, "all 3 data files present"),
        (
            "pam.py Python 3 compatible",
            Status::Ok,
            "no Python 2 imports",
        ),
        (
            "Directories traversable",
            Status::Ok,
            "world-execute bit set",
        ),
        (
            "PAM wired in",
            Status::Ok,
            "present in /etc/pam.d/common-auth",
        ),
        (
            "Camera configured",
            Status::Ok,
            "/dev/video2 (Integrated IR camera, IR)",
        ),
        ("Face model enrolled", Status::Ok, "2 model(s) for demo"),
    ]
    .into_iter()
    .map(|(name, status, detail)| Check {
        name: name.to_string(),
        status,
        detail: detail.to_string(),
    })
    .collect()
}

/// Entry point from `main`.
pub fn run(demo: bool, user: String) -> anyhow::Result<()> {
    let mut app = App::new(demo, user);
    app.load();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        format!("{}", terminal.backend())
    }

    #[test]
    fn every_tab_renders_in_demo_mode() {
        let mut app = App::new(true, "demo".into());
        app.load();
        for tab in Tab::ALL {
            app.tab = tab;
            let screen = render(&app);
            // logo + the active tab's content are always on screen
            assert!(
                screen.contains("howdyctl"),
                "logo missing on {}",
                tab.label()
            );
            assert!(
                screen.contains("Cameras"),
                "tab bar missing on {}",
                tab.label()
            );
        }
    }

    #[test]
    fn cameras_tab_shows_devices_and_active_marker() {
        let mut app = App::new(true, "demo".into());
        app.load();
        app.tab = Tab::Cameras;
        let screen = render(&app);
        assert!(screen.contains("/dev/video2"));
        assert!(screen.contains("IR"));
    }

    #[test]
    fn tidy_name_takes_meaningful_tail() {
        assert_eq!(
            tidy_name("ASUS FHD webcam: ASUS IR camera"),
            "ASUS IR camera"
        );
        assert_eq!(tidy_name("Integrated Camera"), "Integrated Camera");
        assert_eq!(tidy_name("Foo: "), "Foo:"); // empty tail → keep whole (trimmed)
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("abc", 6), "abc   ");
        assert_eq!(fit("abcdef", 6), "abcdef");
        assert_eq!(fit("ASUS FHD webcam: ASUS IR camera", 10), "ASUS FHD …");
        assert_eq!(fit("ASUS FHD …", 10).chars().count(), 10);
    }

    #[test]
    fn enroll_label_field_renders_what_you_type() {
        let mut app = App::new(true, "demo".into());
        app.load();
        app.tab = Tab::Models;
        app.begin_enroll();
        if let Some(buf) = app.enrolling.as_mut() {
            buf.push_str("Work laptop");
        }
        let screen = render(&app);
        assert!(screen.contains("label"), "label prompt missing");
        assert!(screen.contains("Work laptop"), "typed text missing");
        assert!(screen.contains("enroll"), "footer hint missing");
    }
}
