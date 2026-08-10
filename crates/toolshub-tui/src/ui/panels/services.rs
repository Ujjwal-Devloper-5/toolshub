//! Systemd services panel — live list with status colors and service control keys.

use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

use crate::app::AppState;

pub fn render_services(f: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title(" ⚙ Services  [r]estart  [s]tart  [S]top ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let header_cells = ["SERVICE", "ACTIVE", "SUB", "DESCRIPTION"]
        .iter()
        .map(|h| {
            ratatui::widgets::Cell::from(*h).style(
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows: Vec<Row> = state
        .services
        .iter()
        .map(|svc| {
            let active_style = match svc.active.as_str() {
                "active"     => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                "failed"     => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                "activating" => Style::default().fg(Color::Yellow),
                _            => Style::default().fg(Color::DarkGray),
            };

            Row::new(vec![
                ratatui::widgets::Cell::from(format!("  {}", svc.name))
                    .style(Style::default().fg(Color::White)),
                ratatui::widgets::Cell::from(svc.active.clone()).style(active_style),
                ratatui::widgets::Cell::from(svc.sub.clone())
                    .style(Style::default().fg(Color::DarkGray)),
                ratatui::widgets::Cell::from(svc.description.clone())
                    .style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(24),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Min(0),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    // Borrow the list state temporarily as a TableState
    let selected = state.svc_list_state.selected();
    let mut table_state = TableState::default();
    table_state.select(selected);

    f.render_stateful_widget(table, area, &mut table_state);
}
