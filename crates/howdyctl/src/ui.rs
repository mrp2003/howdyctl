//! Shared TUI styling and small widgets, in the spirit of `lampctl`: a restrained
//! monochrome palette with a single bright accent, rounded titled blocks, and a
//! keycap footer.
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

pub const ACCENT: Color = Color::Rgb(0xF5, 0xF5, 0xF5); // bright white accent
pub const DIM: Color = Color::Rgb(0xA6, 0xAE, 0xB0); // legible gray
pub const OK: Color = Color::Rgb(0x6B, 0xD6, 0x8A); // green
pub const WARN: Color = Color::Rgb(0xE6, 0xC3, 0x84); // amber
pub const BAD: Color = Color::Rgb(0xE8, 0x8A, 0x8A); // red

/// A rounded, dim-bordered block with a bright title.
pub fn titled(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).bold(),
        ))
}

/// Centre a `width` x `height` region inside `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [_, row, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height.min(area.height)),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, mid, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width.min(area.width)),
        Constraint::Fill(1),
    ])
    .areas(row);
    mid
}

/// A footer of `key`/`description` hints, e.g. ` ↑↓ move   q quit `.
pub fn footer(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (k, d) in hints {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::default().fg(Color::Black).bg(ACCENT),
        ));
        spans.push(Span::styled(format!(" {d}   "), Style::default().fg(DIM)));
    }
    Line::from(spans)
}

/// A horizontal 0..10 gauge marking the certainty `threshold` and, optionally, the
/// best `distance` achieved — coloured green when it matched, red when it didn't.
pub fn gauge(threshold: f64, distance: Option<f64>) -> Vec<Line<'static>> {
    const W: usize = 40; // cells spanning 0..10
    let pos = |v: f64| ((v / 10.0).clamp(0.0, 1.0) * (W as f64 - 1.0)).round() as usize;
    let tp = pos(threshold);

    let before = "─".repeat(tp);
    let after = "─".repeat(W - tp - 1);
    let bar = Line::from(vec![
        Span::styled("0 ", Style::default().fg(DIM)),
        Span::styled(before, Style::default().fg(DIM)),
        Span::styled(
            "┃",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(after, Style::default().fg(DIM)),
        Span::styled(" 10", Style::default().fg(DIM)),
    ]);

    let mut marker = vec![' '; W + 2]; // +2 for the leading "0 "
    let mcolor = match distance {
        Some(d) if d <= threshold => OK,
        Some(_) => BAD,
        None => DIM,
    };
    if let Some(d) = distance {
        let dp = pos(d) + 2;
        if dp < marker.len() {
            marker[dp] = '▲';
        }
    }
    let marker_line = Line::from(Span::styled(
        marker.into_iter().collect::<String>(),
        Style::default().fg(mcolor),
    ));

    vec![bar, marker_line]
}
