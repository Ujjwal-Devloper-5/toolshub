//! Scrollable rich output panel.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::AppState;

pub fn render_output(f: &mut Frame, area: Rect, state: &AppState) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let total = state.output.len();

    // Clamp scroll to valid range
    let scroll_top = state.output_scroll.min(total.saturating_sub(inner_height));
    let end = (scroll_top + inner_height).min(total);
    let visible = &state.output[scroll_top..end];

    let mut in_code = false;
    let lines: Vec<Line> = visible
        .iter()
        .map(|raw| {
            if raw.trim_start().starts_with("```") {
                in_code = !in_code;
                return Line::from(Span::styled(
                    raw.clone(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            if in_code {
                return Line::from(Span::styled(
                    raw.clone(),
                    Style::default().fg(Color::Green),
                ));
            }
            color_line(raw)
        })
        .collect();

    // Scroll indicator
    let title = if scroll_top > 0 {
        format!(" Output  ↑ {} lines above ", scroll_top)
    } else {
        " Output ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn color_line(raw: &str) -> Line<'_> {
    let style = if raw.starts_with("  ✓") || raw.starts_with("  ▶") {
        Style::default().fg(Color::Green)
    } else if raw.starts_with("  ✗") || raw.starts_with("  ⚠") {
        Style::default().fg(Color::Red)
    } else if raw.starts_with("  >") || raw.starts_with("  [Run") {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else if raw.starts_with("  Processing") || raw.starts_with("  Thinking") {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
    } else if raw.starts_with("  ╔") || raw.starts_with("  ║") || raw.starts_with("  ╚") {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if raw.starts_with("  [1]") {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    Line::from(Span::styled(raw.to_string(), style))
}
