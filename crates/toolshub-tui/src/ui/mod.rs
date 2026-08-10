//! Top-level UI layout router — splits the screen and dispatches to panels.

pub mod header;
pub mod input;
pub mod panels;
pub mod status;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::app::{ActivePanel, AppState};

/// Main render function — called every frame.
pub fn render(f: &mut Frame, state: &mut AppState) {
    let area = f.size();

    // ── Vertical layout: tab-bar / body / input / status ─────────────────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar / header
            Constraint::Min(5),    // Main content
            Constraint::Length(3), // Command input (Output panel only)
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    header::render_header(f, rows[0], state);

    // ── Main content area: panel-specific rendering ───────────────────────
    match state.active_panel {
        ActivePanel::Output => {
            panels::output::render_output(f, rows[1], state);
            input::render_input(f, rows[2], state);
        }
        ActivePanel::Tools => {
            panels::tools::render_tools(f, rows[1], state);
            render_hint(f, rows[2], "[2] Tools  ↑↓ navigate  Type to filter  Enter: explain  Esc: clear");
        }
        ActivePanel::Chat => {
            panels::chat::render_chat(f, rows[1], state);
            render_hint(f, rows[2], "[3] Chat  Type your question and press Enter  Esc: clear");
        }
        ActivePanel::Packages => {
            panels::packages::render_packages(f, rows[1], state);
            render_hint(f, rows[2], "[4] Packages  Type package name  Enter: search  ↑↓ navigate");
        }
        ActivePanel::Services => {
            panels::services::render_services(f, rows[1], state);
            render_hint(f, rows[2], "[5] Services  ↑↓ navigate  r=restart  s=start  S=stop");
        }
        ActivePanel::Memory => {
            panels::memory::render_memory(f, rows[1], state);
            render_hint(f, rows[2], "[6] Memory  ↑↓ navigate  d=mark  D=delete marked");
        }
    }

    status::render_status(f, rows[3], state);
}

fn render_hint(f: &mut Frame, area: ratatui::layout::Rect, hint: &str) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph},
    };
    let p = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(hint, Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}
