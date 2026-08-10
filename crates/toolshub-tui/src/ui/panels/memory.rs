//! Memory browser panel — stored facts viewer with delete marking.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render_memory(f: &mut Frame, area: Rect, state: &mut AppState) {
    let title = format!(" 🧠 Memory Facts ({}) ", state.memory_facts.len());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    if state.memory_facts.is_empty() {
        let empty = Paragraph::new("  No facts stored yet — facts are learned as you use ToolsHub")
            .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
            .block(block);
        f.render_widget(empty, area);
        return;
    }

    // Ensure marked vec is in sync
    while state.memory_marked.len() < state.memory_facts.len() {
        state.memory_marked.push(false);
    }
    state.memory_marked.truncate(state.memory_facts.len());

    let items: Vec<ListItem> = state
        .memory_facts
        .iter()
        .enumerate()
        .map(|(i, fact)| {
            let is_marked = state.memory_marked.get(i).copied().unwrap_or(false);
            if is_marked {
                ListItem::new(format!("  ✗ {fact}"))
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::CROSSED_OUT))
            } else {
                ListItem::new(format!("  {fact}"))
                    .style(Style::default().fg(Color::White))
            }
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut state.memory_list_state);
}
