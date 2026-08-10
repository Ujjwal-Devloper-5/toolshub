//! Status bar — shows panel badge, status message, and keybindings.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{ActivePanel, AppState};

pub fn render_status(f: &mut Frame, area: Rect, state: &AppState) {
    let (badge_text, badge_color) = panel_badge(state.active_panel);

    let line = Line::from(vec![
        Span::styled(
            format!(" {badge_text} "),
            Style::default().bg(badge_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(&state.status_msg, Style::default().fg(Color::DarkGray)),
        Span::raw("   "),
        Span::styled(
            "Ctrl+Q:quit  Tab:next  1-6:jump panel",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]);

    let status = Paragraph::new(line).alignment(Alignment::Left);
    f.render_widget(status, area);
}

fn panel_badge(panel: ActivePanel) -> (&'static str, Color) {
    match panel {
        ActivePanel::Output   => ("OUTPUT",   Color::White),
        ActivePanel::Tools    => ("TOOLS",    Color::Yellow),
        ActivePanel::Chat     => ("CHAT",     Color::Green),
        ActivePanel::Packages => ("PACKAGES", Color::Blue),
        ActivePanel::Services => ("SERVICES", Color::Red),
        ActivePanel::Memory   => ("MEMORY",   Color::Magenta),
    }
}
