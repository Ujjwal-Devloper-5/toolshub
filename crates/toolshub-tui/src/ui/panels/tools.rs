//! Tools panel — filterable list of discovered tools.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render_tools(f: &mut Frame, area: Rect, state: &mut AppState) {
    // Split: filter box on top (3 rows), list below
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_filter_box(f, rows[0], state);
    render_list(f, rows[1], state);
}

fn render_filter_box(f: &mut Frame, area: Rect, state: &AppState) {
    let query = &state.tools_filter;
    let block = Block::default()
        .title(" 🔍 Filter tools ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let text = if query.is_empty() {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("Type to filter…", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  › ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(query.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ])
    };

    let filter = Paragraph::new(text).block(block);
    f.render_widget(filter, area);

    // Cursor
    if !query.is_empty() {
        f.set_cursor(
            area.x + 5 + query.len() as u16,
            area.y + 1,
        );
    }
}

fn render_list(f: &mut Frame, area: Rect, state: &mut AppState) {
    let filtered = state.filtered_tools();
    let count = filtered.len();
    let total = state.list_items.len();

    let title = if state.tools_filter.is_empty() {
        format!(" Tools ({total}) ")
    } else {
        format!(" Tools ({count}/{total} matching) ")
    };

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|t| ListItem::new(format!("  {t}")))
        .collect();

    let empty_msg = if state.list_items.is_empty() {
        "  No tools discovered yet — run 'toolshub tools' in your terminal"
    } else {
        "  No tools match the current filter"
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    if items.is_empty() {
        let para = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
            .block(block);
        f.render_widget(para, area);
        return;
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut state.tools_list_state);
}
