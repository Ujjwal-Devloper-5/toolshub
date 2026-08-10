//! Command input box for the Output panel.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render_input(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" Command / Question ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let input_text = Paragraph::new(Line::from(vec![
        Span::styled("  › ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(state.input.clone()),
    ]))
    .block(block);

    f.render_widget(input_text, area);

    // Position cursor
    f.set_cursor(
        area.x + 5 + state.input_cursor as u16,
        area.y + 1,
    );
}
