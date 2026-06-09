//! The interactive ratatui application: a tabbed manager for Howdy.
use std::time::Duration;

use howdy::config::Config;
use howdy::doctor::{self, Status};
use howdy::model::{self, Model};
use howdy::test::{self, TestResult};
use howdy::{camera, Camera};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::ui::{self, ACCENT, BAD, DIM, OK, WARN};

const TICK: Duration = Duration::from_millis(200);

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
    fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
}

struct App {
    demo: bool,
    user: String,
    tab: Tab,
    quit: bool,
    status: String,

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
            quit: false,
            status: "ready".into(),
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
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab | KeyCode::Right => self.cycle_tab(1),
            KeyCode::BackTab | KeyCode::Left => self.cycle_tab(-1),
            KeyCode::Up => self.move_sel(-1),
            KeyCode::Down => self.move_sel(1),
            KeyCode::Char('r') => {
                self.load();
                self.status = "refreshed".into();
            }
            other => self.on_tab_key(other, terminal),
        }
    }

    fn on_tab_key(&mut self, code: KeyCode, terminal: &mut DefaultTerminal) {
        match self.tab {
            Tab::Cameras => {
                if code == KeyCode::Enter {
                    self.set_active_camera(terminal);
                }
            }
            Tab::Models => match code {
                KeyCode::Char('a') => self.enroll(terminal),
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

    fn cycle_tab(&mut self, delta: i32) {
        let n = Tab::ALL.len() as i32;
        let i = (self.tab.index() as i32 + delta).rem_euclid(n) as usize;
        self.tab = Tab::ALL[i];
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

    fn enroll(&mut self, terminal: &mut DefaultTerminal) {
        if self.guard_demo() {
            return;
        }
        let user = self.user.clone();
        let ok = self.privileged(terminal, &["--user", &user, "add"], true);
        self.reload_models();
        self.status = if ok {
            "enrolled a new model".into()
        } else {
            "enrollment cancelled or failed".into()
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
        let area = ui::centered(f.area(), 76, 24);
        let [logo, tabs, body, status, footer] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        self.draw_logo(f, logo);
        self.draw_tabs(f, tabs);
        match self.tab {
            Tab::Cameras => self.draw_cameras(f, body),
            Tab::Models => self.draw_models(f, body),
            Tab::Test => self.draw_test(f, body),
            Tab::Doctor => self.draw_doctor(f, body),
        }
        self.draw_status(f, status);
        self.draw_footer(f, footer);
    }

    fn draw_logo(&self, f: &mut Frame, area: Rect) {
        let subtitle = if self.demo {
            format!(
                "face unlock for Howdy · demo (no device) · user: {}",
                self.user
            )
        } else {
            format!("face unlock for Howdy · user: {}", self.user)
        };
        let lines = vec![
            Line::from("[ ◔ ‿ ◔ ]").fg(ACCENT).bold(),
            Line::from("howdyctl").fg(ACCENT).bold(),
            Line::from(subtitle).fg(DIM).italic(),
        ];
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let mut spans = Vec::new();
        for t in Tab::ALL {
            let style = if t == self.tab {
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            spans.push(Span::styled(format!("  {}  ", t.label()), style));
            spans.push(Span::raw(" "));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
            area,
        );
    }

    fn draw_cameras(&self, f: &mut Frame, area: Rect) {
        let block = ui::titled("Cameras");
        if self.cameras.is_empty() {
            f.render_widget(
                Paragraph::new("no /dev/video* devices found")
                    .fg(DIM)
                    .block(block),
                area,
            );
            return;
        }
        let items: Vec<ListItem> = self
            .cameras
            .iter()
            .map(|c| {
                let active = c.path.to_string_lossy() == self.device_path;
                let dot = if active { "● " } else { "  " };
                let kind = if c.is_ir { "IR " } else { "RGB" };
                let kind_color = if c.is_ir { OK } else { DIM };
                let cap = if c.can_capture {
                    Span::styled("capture", Style::default().fg(DIM))
                } else {
                    Span::styled("metadata", Style::default().fg(WARN))
                };
                ListItem::new(Line::from(vec![
                    Span::styled(dot, Style::default().fg(OK)),
                    Span::styled(format!("{:<12}", c.path.display()), Style::default().bold()),
                    Span::styled(format!("{:<24}", c.name), Style::default().fg(DIM)),
                    Span::styled(format!("{kind}  "), Style::default().fg(kind_color)),
                    cap,
                ]))
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.cam_sel));
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(list, area, &mut state);
    }

    fn draw_models(&self, f: &mut Frame, area: Rect) {
        let block = ui::titled("Face models");
        if self.models.is_empty() {
            f.render_widget(
                Paragraph::new("no models enrolled — press  a  to add one")
                    .fg(DIM)
                    .block(block),
                area,
            );
            return;
        }
        let items: Vec<ListItem> = self
            .models
            .iter()
            .map(|m| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<3}", m.id), Style::default().fg(DIM)),
                    Span::styled(
                        format!("{}  ", model::format_time(m.time)),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(m.label.clone(), Style::default().bold()),
                ]))
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.model_sel));
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(list, area, &mut state);
    }

    fn draw_test(&self, f: &mut Frame, area: Rect) {
        let block = ui::titled("Recognition test");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let pending = (self.pending_certainty - self.certainty).abs() > 1e-6;
        let thr_line = if pending {
            Line::from(vec![
                Span::styled("Threshold (certainty)  ", Style::default().fg(DIM)),
                Span::styled(format!("{:.1}", self.certainty), Style::default().bold()),
                Span::styled("  →  ", Style::default().fg(DIM)),
                Span::styled(
                    format!("{:.1} (unsaved — s to save)", self.pending_certainty),
                    Style::default().fg(WARN).bold(),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("Threshold (certainty)  ", Style::default().fg(DIM)),
                Span::styled(format!("{:.1}", self.certainty), Style::default().bold()),
                Span::styled("   (lower = stricter)", Style::default().fg(DIM)),
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
        let block = ui::titled("Doctor");
        let inner = block.inner(area);
        f.render_widget(block, area);
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
                    Span::styled(format!(" {glyph} "), Style::default().fg(color).bold()),
                    Span::styled(format!("{:<28}", c.name), Style::default().bold()),
                    Span::styled(c.detail.clone(), Style::default().fg(DIM)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" • ", Style::default().fg(ACCENT)),
                Span::styled(self.status.clone(), Style::default().fg(DIM)),
            ])),
            area,
        );
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let hints: &[(&str, &str)] = match self.tab {
            Tab::Cameras => &[
                ("↑↓", "select"),
                ("↵", "set active"),
                ("⇥", "tab"),
                ("r", "refresh"),
                ("q", "quit"),
            ],
            Tab::Models => &[
                ("↑↓", "select"),
                ("a", "add"),
                ("d", "delete"),
                ("⇥", "tab"),
                ("q", "quit"),
            ],
            Tab::Test => &[
                ("↵", "run"),
                ("±", "threshold"),
                ("s", "save"),
                ("e", "detail"),
                ("q", "quit"),
            ],
            Tab::Doctor => &[("↵", "re-check"), ("f", "fix"), ("⇥", "tab"), ("q", "quit")],
        };
        f.render_widget(
            Paragraph::new(ui::footer(hints)).alignment(Alignment::Center),
            area,
        );
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
}
