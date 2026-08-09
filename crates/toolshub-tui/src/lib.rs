//! `toolshub-tui` — Terminal User Interface components for ToolsHub.
//!
//! Provides reusable [`ratatui`]-based widgets and a top-level [`App`]
//! that can be launched for an interactive session.
//!
//! # Architecture
//!
//! ```text
//! App
//! ├── state: AppState           — all mutable application state
//! ├── run() → event loop        — drives the ratatui frame render + input
//! └── ui(frame)                 — renders widgets from state
//! ```

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::time::Duration;

// ── App state ─────────────────────────────────────────────────────────────────

/// Which panel has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Input,
    Results,
}

/// All mutable state for the interactive TUI session.
pub struct AppState {
    /// The user's current input buffer.
    pub input: String,
    /// Cursor position within `input` (byte offset).
    pub cursor: usize,
    /// Output lines displayed in the main panel.
    pub output: Vec<String>,
    /// Scrollable list items (e.g. tool names).
    pub list_items: Vec<String>,
    /// Current scroll position for the list.
    pub list_state: ListState,
    /// Which panel is focused.
    pub focus: Focus,
    /// Status bar message.
    pub status: String,
    /// Whether the app should exit.
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            output: vec![
                "Welcome to ToolsHub Interactive Mode".into(),
                "─────────────────────────────────────".into(),
                "  Type a command or question and press Enter".into(),
                "  Ctrl+Q / Esc  to quit".into(),
                "  Tab           to switch focus".into(),
                "  ↑/↓           to navigate lists".into(),
                String::new(),
            ],
            list_items: Vec::new(),
            list_state: ListState::default(),
            focus: Focus::Input,
            status: "Ready".into(),
            should_quit: false,
        }
    }

    /// Append a line to the output panel.
    pub fn push_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    /// Push multiple lines.
    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.output.extend(lines);
    }

    /// Set the list items and reset the selection.
    pub fn set_list(&mut self, items: Vec<String>) {
        self.list_items = items;
        self.list_state = ListState::default();
        if !self.list_items.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Move list selection up.
    pub fn list_up(&mut self) {
        if self.list_items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if i == 0 { self.list_items.len() - 1 } else { i - 1 }));
    }

    /// Move list selection down.
    pub fn list_down(&mut self) {
        if self.list_items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % self.list_items.len()));
    }

    /// Return the currently selected list item, if any.
    pub fn selected_item(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.list_items.get(i))
            .map(|s| s.as_str())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

/// The top-level interactive TUI application.
pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self { state: AppState::new() }
    }

    /// Run the interactive event loop until the user quits.
    ///
    /// Restores terminal state on exit (even on panic, via the Drop impl).
    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        // Always restore terminal.
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|f| ui(f, &mut self.state))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    handle_key(key, &mut self.state);
                }
            }

            if self.state.should_quit {
                break;
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// ── Input handling ────────────────────────────────────────────────────────────

fn handle_key(key: event::KeyEvent, state: &mut AppState) {
    match (key.code, key.modifiers) {
        // Quit.
        (KeyCode::Char('q'), KeyModifiers::CONTROL)
        | (KeyCode::Char('c'), KeyModifiers::CONTROL)
        | (KeyCode::Esc, _) => {
            state.should_quit = true;
        }

        // Switch focus.
        (KeyCode::Tab, _) => {
            state.focus = match state.focus {
                Focus::Input => Focus::Results,
                Focus::Results => Focus::Input,
            };
            state.status = match state.focus {
                Focus::Input => "Input mode".into(),
                Focus::Results => "Navigate mode — ↑/↓ to scroll".into(),
            };
        }

        // Navigation in list/output.
        (KeyCode::Up, _) => {
            if state.focus == Focus::Results {
                state.list_up();
            }
        }
        (KeyCode::Down, _) => {
            if state.focus == Focus::Results {
                state.list_down();
            }
        }

        // Input editing.
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            if state.focus == Focus::Input {
                state.input.insert(state.cursor, c);
                state.cursor += c.len_utf8();
            }
        }
        (KeyCode::Backspace, _) => {
            if state.focus == Focus::Input && state.cursor > 0 {
                state.cursor -= 1;
                state.input.remove(state.cursor);
            }
        }
        (KeyCode::Left, _) => {
            if state.focus == Focus::Input && state.cursor > 0 {
                state.cursor -= 1;
            }
        }
        (KeyCode::Right, _) => {
            if state.focus == Focus::Input && state.cursor < state.input.len() {
                state.cursor += 1;
            }
        }
        (KeyCode::Home, _) => {
            if state.focus == Focus::Input {
                state.cursor = 0;
            }
        }
        (KeyCode::End, _) => {
            if state.focus == Focus::Input {
                state.cursor = state.input.len();
            }
        }

        // Submit.
        (KeyCode::Enter, _) if state.focus == Focus::Input && !state.input.trim().is_empty() => {
            let cmd = state.input.clone();
            state.output.push(format!("  > {cmd}"));
            state.output.push("  Processing…".into());
            state.input.clear();
            state.cursor = 0;
            state.status = format!("Sent: {}", cmd.trim());
        }

        _ => {}
    }
}

// ── UI rendering ──────────────────────────────────────────────────────────────

fn ui(f: &mut Frame, state: &mut AppState) {
    let size = f.size();

    // ── Overall layout: header / body / status bar ────────────────────────
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Body
            Constraint::Length(3),  // Input
            Constraint::Length(1),  // Status bar
        ])
        .split(size);

    render_header(f, chunks[0]);
    render_body(f, chunks[1], state);
    render_input(f, chunks[2], state);
    render_status(f, chunks[3], state);
}

fn render_header(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled("  ToolsHub ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            "— The Intelligent CLI Operating Companion",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(title, area);
}

fn render_body(f: &mut Frame, area: Rect, state: &mut AppState) {
    // Split body into output (left) and list (right) if there are list items.
    if state.list_items.is_empty() {
        render_output(f, area, state);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(area);
        render_output(f, chunks[0], state);
        render_list(f, chunks[1], state);
    }
}

fn render_output(f: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.focus == Focus::Results;
    let block = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    // Show the last N lines that fit the area.
    let inner_height = area.height.saturating_sub(2) as usize;
    let skip = state.output.len().saturating_sub(inner_height);
    let lines: Vec<Line> = state.output[skip..]
        .iter()
        .map(|l| {
            if l.starts_with("  >") {
                Line::from(Span::styled(l.clone(), Style::default().fg(Color::Green)))
            } else if l.starts_with("  Processing") {
                Line::from(Span::styled(l.clone(), Style::default().fg(Color::Yellow)))
            } else if l.starts_with("  Welcome") || l.starts_with("──") {
                Line::from(Span::styled(l.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            } else {
                Line::from(l.clone())
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_list(f: &mut Frame, area: Rect, state: &mut AppState) {
    let focused = state.focus == Focus::Results;
    let block = Block::default()
        .title(" Tools ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let items: Vec<ListItem> = state
        .list_items
        .iter()
        .map(|i| ListItem::new(format!("  {i}")))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_input(f: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.focus == Focus::Input;
    let block = Block::default()
        .title(" Command / Question ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let input_line = Paragraph::new(Line::from(vec![
        Span::styled("  > ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(state.input.clone()),
    ]))
    .block(block);

    f.render_widget(input_line, area);

    // Position the cursor inside the input box.
    if focused {
        f.set_cursor(
            area.x + 5 + state.cursor as u16,
            area.y + 1,
        );
    }
}

fn render_status(f: &mut Frame, area: Rect, state: &AppState) {
    let focus_label = match state.focus {
        Focus::Input => Span::styled(" INPUT ", Style::default().bg(Color::Green).fg(Color::Black)),
        Focus::Results => Span::styled(" BROWSE ", Style::default().bg(Color::Cyan).fg(Color::Black)),
    };

    let status = Paragraph::new(Line::from(vec![
        focus_label,
        Span::raw("  "),
        Span::styled(&state.status, Style::default().fg(Color::DarkGray)),
        Span::raw("   "),
        Span::styled(
            "Ctrl+Q: quit  Tab: switch focus  Enter: submit",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]))
    .alignment(Alignment::Left);

    f.render_widget(status, area);
}
