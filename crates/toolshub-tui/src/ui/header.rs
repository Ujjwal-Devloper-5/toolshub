//! Tab bar / header rendering.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{ActivePanel, AppState};

const TABS: &[(ActivePanel, &str, &str)] = &[
    (ActivePanel::Output,   "[1]", "Output"),
    (ActivePanel::Tools,    "[2]", "Tools"),
    (ActivePanel::Chat,     "[3]", "Chat"),
    (ActivePanel::Packages, "[4]", "Packages"),
    (ActivePanel::Services, "[5]", "Services"),
    (ActivePanel::Memory,   "[6]", "Memory"),
];

pub fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    // Split header: logo left, tabs right
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22), // Logo
            Constraint::Min(0),     // Tabs
        ])
        .split(area);

    render_logo(f, cols[0]);
    render_tabs(f, cols[1], state);
}

fn render_logo(f: &mut Frame, area: Rect) {
    let logo = Paragraph::new(Line::from(vec![
        Span::styled(" ToolsHub ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("◆", Style::default().fg(Color::Green)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(logo, area);
}

fn render_tabs(f: &mut Frame, area: Rect, state: &AppState) {
    let mut spans = vec![Span::raw("  ")];

    for (panel, key, label) in TABS {
        let active = state.active_panel == *panel;
        if active {
            spans.push(Span::styled(
                format!(" {key} {label} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {key} {label} "),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ));
        }
        spans.push(Span::raw("  "));
    }

    let tabs = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(tabs, area);
}
