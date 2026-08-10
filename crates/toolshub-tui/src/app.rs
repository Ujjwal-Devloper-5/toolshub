//! Application state and the top-level `App` struct.

use ratatui::widgets::ListState;

// ── Panel / Tab enums ─────────────────────────────────────────────────────────

/// The currently active panel / tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Output   = 0,
    Tools    = 1,
    Chat     = 2,
    Packages = 3,
    Services = 4,
    Memory   = 5,
}

impl ActivePanel {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Output,
            1 => Self::Tools,
            2 => Self::Chat,
            3 => Self::Packages,
            4 => Self::Services,
            5 => Self::Memory,
            _ => Self::Output,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 6)
    }

    pub fn prev(self) -> Self {
        Self::from_index((self.index() + 5) % 6)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Output   => "Output",
            Self::Tools    => "Tools",
            Self::Chat     => "Chat",
            Self::Packages => "Packages",
            Self::Services => "Services",
            Self::Memory   => "Memory",
        }
    }
}

// ── Chat types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Ai,
    System,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

// ── Service row ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name: String,
    pub active: String,
    pub sub: String,
    pub description: String,
}

// ── AppState ──────────────────────────────────────────────────────────────────

/// All mutable state for the interactive TUI session.
pub struct AppState {
    // ── General ───────────────────────────────────────────────────────────
    pub should_quit: bool,
    pub active_panel: ActivePanel,
    pub status_msg: String,

    // ── Output panel ──────────────────────────────────────────────────────
    pub output: Vec<String>,
    pub output_scroll: usize,

    // ── Tools panel ───────────────────────────────────────────────────────
    /// All discovered tools (full list).
    pub list_items: Vec<String>,
    /// Current fuzzy filter string.
    pub tools_filter: String,
    pub tools_list_state: ListState,

    // ── Chat panel ────────────────────────────────────────────────────────
    pub chat_history: Vec<ChatMessage>,
    pub chat_input: String,
    pub chat_cursor: usize,
    pub chat_waiting: bool,

    // ── Packages panel ────────────────────────────────────────────────────
    pub pkg_query: String,
    pub pkg_cursor: usize,
    pub pkg_results: Vec<String>,
    pub pkg_list_state: ListState,

    // ── Services panel ────────────────────────────────────────────────────
    pub services: Vec<ServiceRow>,
    pub svc_list_state: ListState,

    // ── Memory panel ──────────────────────────────────────────────────────
    pub memory_facts: Vec<String>,
    pub memory_list_state: ListState,
    pub memory_marked: Vec<bool>,

    // ── Input box (Output panel command input) ────────────────────────────
    pub input: String,
    pub input_cursor: usize,
}

impl AppState {
    pub fn new() -> Self {
        let mut s = Self {
            should_quit: false,
            active_panel: ActivePanel::Output,
            status_msg: "Ready — press 1-6 to switch panels, Ctrl+Q to quit".into(),

            output: vec![
                "  ╔══════════════════════════════════════════════════════╗".into(),
                "  ║        ToolsHub — Intelligent CLI Companion         ║".into(),
                "  ╚══════════════════════════════════════════════════════╝".into(),
                String::new(),
                "  Press 1-6 to switch panels:".into(),
                "  [1] Output  [2] Tools  [3] Chat  [4] Packages  [5] Services  [6] Memory".into(),
                String::new(),
                "  Type a command below and press Enter to run it through ToolsHub.".into(),
                "  Try: explain nvim    ask 'how do I find large files?'    run ls -la".into(),
                String::new(),
            ],
            output_scroll: 0,

            list_items: Vec::new(),
            tools_filter: String::new(),
            tools_list_state: ListState::default(),

            chat_history: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "AI Chat — ask anything. Responses will appear here.".into(),
                },
            ],
            chat_input: String::new(),
            chat_cursor: 0,
            chat_waiting: false,

            pkg_query: String::new(),
            pkg_cursor: 0,
            pkg_results: Vec::new(),
            pkg_list_state: ListState::default(),

            services: demo_services(),
            svc_list_state: {
                let mut s = ListState::default();
                s.select(Some(0));
                s
            },

            memory_facts: vec![
                "[os] name: Arch Linux".into(),
                "[os] kernel: 6.x".into(),
                "[shell] name: zsh".into(),
                "[preference] editor: nvim".into(),
            ],
            memory_list_state: ListState::default(),
            memory_marked: vec![false; 4],

            input: String::new(),
            input_cursor: 0,
        };
        s.tools_list_state.select(Some(0));
        s.memory_list_state.select(Some(0));
        s
    }

    // ── Output helpers ────────────────────────────────────────────────────

    pub fn push_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
        // Auto-scroll to bottom on new output.
        self.output_scroll = self.output.len().saturating_sub(1);
    }

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.output.extend(lines);
        self.output_scroll = self.output.len().saturating_sub(1);
    }

    // ── Tools helpers ─────────────────────────────────────────────────────

    pub fn set_tools(&mut self, items: Vec<String>) {
        self.list_items = items;
        self.tools_list_state = ListState::default();
        if !self.list_items.is_empty() {
            self.tools_list_state.select(Some(0));
        }
    }

    /// Returns filtered tools (case-insensitive substring match).
    pub fn filtered_tools(&self) -> Vec<&str> {
        if self.tools_filter.is_empty() {
            return self.list_items.iter().map(|s| s.as_str()).collect();
        }
        let q = self.tools_filter.to_lowercase();
        self.list_items
            .iter()
            .filter(|t| t.to_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn tools_up(&mut self) {
        let items = self.filtered_tools();
        if items.is_empty() { return; }
        let len = items.len();
        let i = self.tools_list_state.selected().unwrap_or(0);
        self.tools_list_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn tools_down(&mut self) {
        let items = self.filtered_tools();
        if items.is_empty() { return; }
        let len = items.len();
        let i = self.tools_list_state.selected().unwrap_or(0);
        self.tools_list_state.select(Some((i + 1) % len));
    }

    pub fn selected_tool(&self) -> Option<String> {
        let filtered = self.filtered_tools();
        self.tools_list_state
            .selected()
            .and_then(|i| filtered.get(i))
            .map(|s| s.to_string())
    }

    // ── Services helpers ──────────────────────────────────────────────────

    pub fn svc_up(&mut self) {
        if self.services.is_empty() { return; }
        let len = self.services.len();
        let i = self.svc_list_state.selected().unwrap_or(0);
        self.svc_list_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn svc_down(&mut self) {
        if self.services.is_empty() { return; }
        let len = self.services.len();
        let i = self.svc_list_state.selected().unwrap_or(0);
        self.svc_list_state.select(Some((i + 1) % len));
    }

    // ── Memory helpers ────────────────────────────────────────────────────

    pub fn mem_up(&mut self) {
        if self.memory_facts.is_empty() { return; }
        let len = self.memory_facts.len();
        let i = self.memory_list_state.selected().unwrap_or(0);
        self.memory_list_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn mem_down(&mut self) {
        if self.memory_facts.is_empty() { return; }
        let len = self.memory_facts.len();
        let i = self.memory_list_state.selected().unwrap_or(0);
        self.memory_list_state.select(Some((i + 1) % len));
    }

    pub fn mem_toggle_mark(&mut self) {
        if let Some(i) = self.memory_list_state.selected() {
            if let Some(m) = self.memory_marked.get_mut(i) {
                *m = !*m;
            }
        }
    }

    // ── Package helpers ───────────────────────────────────────────────────

    pub fn pkg_up(&mut self) {
        if self.pkg_results.is_empty() { return; }
        let len = self.pkg_results.len();
        let i = self.pkg_list_state.selected().unwrap_or(0);
        self.pkg_list_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn pkg_down(&mut self) {
        if self.pkg_results.is_empty() { return; }
        let len = self.pkg_results.len();
        let i = self.pkg_list_state.selected().unwrap_or(0);
        self.pkg_list_state.select(Some((i + 1) % len));
    }
}

impl Default for AppState {
    fn default() -> Self { Self::new() }
}

// ── Demo data ─────────────────────────────────────────────────────────────────

fn demo_services() -> Vec<ServiceRow> {
    vec![
        ServiceRow { name: "NetworkManager".into(),  active: "active".into(),   sub: "running".into(), description: "Network connection manager".into() },
        ServiceRow { name: "sshd".into(),            active: "active".into(),   sub: "running".into(), description: "OpenSSH server daemon".into() },
        ServiceRow { name: "bluetooth".into(),        active: "active".into(),   sub: "running".into(), description: "Bluetooth service".into() },
        ServiceRow { name: "cronie".into(),           active: "active".into(),   sub: "running".into(), description: "Periodic command scheduler".into() },
        ServiceRow { name: "docker".into(),           active: "inactive".into(), sub: "dead".into(),    description: "Docker Application Container Engine".into() },
        ServiceRow { name: "nginx".into(),            active: "inactive".into(), sub: "dead".into(),    description: "Lightweight HTTP server".into() },
        ServiceRow { name: "postgresql".into(),       active: "failed".into(),   sub: "failed".into(),  description: "PostgreSQL database".into() },
        ServiceRow { name: "pipewire".into(),         active: "active".into(),   sub: "running".into(), description: "PipeWire audio/video daemon".into() },
        ServiceRow { name: "systemd-resolved".into(), active: "active".into(),   sub: "running".into(), description: "DNS resolver".into() },
    ]
}

// ── App ───────────────────────────────────────────────────────────────────────

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

/// The top-level interactive TUI application.
pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self { state: AppState::new() }
    }

    /// Run the interactive event loop until the user quits.
    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        // Always restore terminal, even on error.
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> io::Result<()> {
        loop {
            terminal.draw(|f| crate::ui::render(f, &mut self.state))?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        crate::events::handle_key(key, &mut self.state);
                    }
                    Event::Mouse(mouse) => {
                        crate::events::handle_mouse(mouse, &mut self.state);
                    }
                    _ => {}
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
    fn default() -> Self { Self::new() }
}
