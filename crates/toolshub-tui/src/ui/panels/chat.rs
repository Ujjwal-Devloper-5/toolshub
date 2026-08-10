//! AI Chat panel — split view: message history + input box.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{AppState, ChatRole};

pub fn render_chat(f: &mut Frame, area: Rect, state: &mut AppState) {
    // Split: history (left 65%) | input (right 35%)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    render_history(f, cols[0], state);
    render_chat_input(f, cols[1], state);
}

fn render_history(f: &mut Frame, area: Rect, state: &AppState) {
    let inner_h = area.height.saturating_sub(2) as usize;
    let msgs = &state.chat_history;

    // Build lines from messages
    let mut all_lines: Vec<Line> = Vec::new();
    for msg in msgs {
        match msg.role {
            ChatRole::System => {
                all_lines.push(Line::from(Span::styled(
                    format!("  ── {} ──", msg.content),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                )));
            }
            ChatRole::User => {
                all_lines.push(Line::from(vec![])); // spacer
                all_lines.push(Line::from(vec![
                    Span::styled("  You  ", Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(msg.content.clone(), Style::default().fg(Color::Green)),
                ]));
            }
            ChatRole::Ai => {
                let is_thinking = msg.content.starts_with("[AI is thinking");
                let style = if is_thinking {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                all_lines.push(Line::from(vec![
                    Span::styled("  AI   ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(msg.content.clone(), style),
                ]));
            }
        }
    }

    // Show last N lines
    let skip = all_lines.len().saturating_sub(inner_h);
    let visible: Vec<Line> = all_lines[skip..].to_vec();

    let block = Block::default()
        .title(" 💬 Chat History ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let para = Paragraph::new(visible)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_chat_input(f: &mut Frame, area: Rect, state: &AppState) {
    // Split input area vertically: instructions + input box
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    // Tips area
    let tips = Paragraph::new(vec![
        Line::from(Span::styled("  AI Chat", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
        Line::from(Span::raw("")),
        Line::from(Span::styled("  For live streaming", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  responses, use:", Style::default().fg(Color::DarkGray))),
        Line::from(Span::raw("")),
        Line::from(Span::styled("  toolshub ask", Style::default().fg(Color::Cyan))),
        Line::from(Span::raw("")),
        Line::from(Span::styled("  This panel stores", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  your question for", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  reference.", Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(tips, rows[0]);

    // Input box
    let block = Block::default()
        .title(" Ask anything… ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  › ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(state.chat_input.clone()),
    ]))
    .block(block);
    f.render_widget(input, rows[1]);

    // Cursor
    f.set_cursor(
        rows[1].x + 5 + state.chat_cursor as u16,
        rows[1].y + 1,
    );
}
