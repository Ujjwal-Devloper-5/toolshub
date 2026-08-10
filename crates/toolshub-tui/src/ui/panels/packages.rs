//! Package browser panel — search + results list.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render_packages(f: &mut Frame, area: Rect, state: &mut AppState) {
    // Layout: search box (3 rows) | results list
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_search_box(f, rows[0], state);
    render_results(f, rows[1], state);
}

fn render_search_box(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" 📦 Search packages ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let text = if state.pkg_query.is_empty() {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "Type a package name and press Enter…",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("  › ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::styled(
                state.pkg_query.clone(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ])
    };

    let search = Paragraph::new(text).block(block);
    f.render_widget(search, area);

    // Cursor
    if !state.pkg_query.is_empty() {
        f.set_cursor(
            area.x + 5 + state.pkg_query.len() as u16,
            area.y + 1,
        );
    }
}

fn render_results(f: &mut Frame, area: Rect, state: &mut AppState) {
    let title = if state.pkg_results.is_empty() {
        " Results ".to_string()
    } else {
        format!(" Results ({}) ", state.pkg_results.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    if state.pkg_results.is_empty() {
        let msg = if state.pkg_query.is_empty() {
            "  Type a package name above and press Enter to search across all managers"
        } else {
            "  No results — press Enter to search"
        };
        let empty = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
            .block(block);
        f.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .pkg_results
        .iter()
        .map(|r| ListItem::new(format!("  {r}")))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut state.pkg_list_state);
}
