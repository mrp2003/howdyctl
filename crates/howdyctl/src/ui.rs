//! Shared TUI styling and small widgets — a pythops-flavoured layout (slim tab row,
//! a single focused panel with a coloured border, a quiet help line) themed with the
//! **Everforest Dark (medium)** palette.
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

// ── Everforest Dark (medium) ────────────────────────────────────────────────
pub const BG: Color = Color::Rgb(0x2d, 0x35, 0x3b); // base background
pub const SURFACE: Color = Color::Rgb(0x34, 0x3f, 0x44); // panels / overlays
pub const TEXT: Color = Color::Rgb(0xd3, 0xc6, 0xaa); // primary cream text
pub const DIM: Color = Color::Rgb(0x85, 0x92, 0x89); // labels / secondary
pub const BORDER: Color = Color::Rgb(0x4f, 0x58, 0x5e); // unfocused borders
pub const ACCENT: Color = Color::Rgb(0xa7, 0xc0, 0x80); // green — focus
pub const SELECT: Color = Color::Rgb(0xc8, 0xd2, 0x7e); // green-yellow lime — selection
pub const AQUA: Color = Color::Rgb(0x83, 0xc0, 0x92); // IR / secondary highlight
pub const OK: Color = Color::Rgb(0xa7, 0xc0, 0x80); // green
pub const WARN: Color = Color::Rgb(0xdb, 0xbc, 0x7f); // yellow
pub const BAD: Color = Color::Rgb(0xe6, 0x7e, 0x80); // red

/// Style for the selected row: the text recolours to a green-yellow lime
/// (no background bar) as the cursor moves.
pub fn selection() -> Style {
    Style::default().fg(SELECT).add_modifier(Modifier::BOLD)
}

/// A dim, bold column-header line.
pub fn header(text: String) -> Line<'static> {
    Line::from(Span::styled(text, Style::default().fg(DIM).bold()))
}

/// A rounded block whose border is green when `focused`, dim otherwise.
pub fn panel(title: &str, focused: bool) -> Block<'_> {
    let border = if focused { ACCENT } else { BORDER };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        // a slightly lighter surface than the base background, for depth
        .style(Style::default().bg(SURFACE))
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(if focused { ACCENT } else { DIM })
                .bold(),
        ))
}

/// The slim tab row plus an underline beneath the active tab (rendered into a
/// 2-line area). `active` is the index into `names`.
pub fn tab_bar(names: &[&str], active: usize) -> (Line<'static>, Line<'static>) {
    let mut spans = Vec::new();
    let mut underline_cols: Option<(usize, usize)> = None;
    let mut col = 2usize; // leading indent
    spans.push(Span::raw("  "));
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
            col += 3;
        }
        if i == active {
            underline_cols = Some((col, name.len()));
            spans.push(Span::styled(
                name.to_string(),
                Style::default().fg(ACCENT).bold(),
            ));
        } else {
            spans.push(Span::styled(name.to_string(), Style::default().fg(DIM)));
        }
        col += name.len();
    }

    let underline = match underline_cols {
        Some((start, len)) => {
            let mut s = String::new();
            s.push_str(&" ".repeat(start));
            s.push_str(&"─".repeat(len));
            Line::from(Span::styled(s, Style::default().fg(ACCENT)))
        }
        None => Line::from(""),
    };
    (Line::from(spans), underline)
}

/// A footer of `key`/`description` hints, e.g. ` ↑↓ move   q quit `.
pub fn help_bar(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (k, d) in hints {
        spans.push(Span::styled(
            (*k).to_string(),
            Style::default().fg(ACCENT).bold(),
        ));
        spans.push(Span::styled(format!(" {d}    "), Style::default().fg(DIM)));
    }
    Line::from(spans)
}

/// A horizontal 0..10 gauge: a gradient-filled bar up to the certainty `threshold`,
/// with a marker showing the best `distance` achieved (green if it matched, red if not).
pub fn gauge(threshold: f64, distance: Option<f64>) -> Vec<Line<'static>> {
    const W: usize = 40; // cells spanning 0..10
    let pos = |v: f64| ((v / 10.0).clamp(0.0, 1.0) * (W as f64 - 1.0)).round() as usize;
    let tp = pos(threshold);

    // gradient from green (0) to aqua (threshold) for the filled region
    let mut bar_spans = vec![Span::styled("0 ", Style::default().fg(DIM))];
    for i in 0..W {
        if i < tp {
            let t = if tp == 0 { 0.0 } else { i as f64 / tp as f64 };
            bar_spans.push(Span::styled(
                "█",
                Style::default().fg(lerp(ACCENT, AQUA, t)),
            ));
        } else if i == tp {
            bar_spans.push(Span::styled(
                "┃",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ));
        } else {
            bar_spans.push(Span::styled("░", Style::default().fg(BORDER)));
        }
    }
    bar_spans.push(Span::styled(" 10", Style::default().fg(DIM)));

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

    vec![Line::from(bar_spans), marker_line]
}

/// Linear interpolation between two RGB colours.
fn lerp(a: Color, b: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}
