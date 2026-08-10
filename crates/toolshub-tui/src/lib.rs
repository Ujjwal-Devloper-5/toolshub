//! `toolshub-tui` — modular Terminal User Interface for ToolsHub.
//!
//! # Architecture
//! ```text
//! lib.rs      — public API (re-exports)
//! app.rs      — AppState, App, enums
//! events.rs   — keyboard + mouse input
//! ui/
//!   mod.rs    — layout router
//!   header.rs — tab bar
//!   input.rs  — command input
//!   status.rs — status bar
//!   panels/
//!     output.rs   — scrollable output
//!     tools.rs    — tools list + fuzzy filter
//!     chat.rs     — AI chat history
//!     packages.rs — package browser
//!     services.rs — systemd services
//!     memory.rs   — stored facts viewer
//! ```

pub mod app;
pub mod events;
pub mod ui;

pub use app::{ActivePanel, App, AppState};

use std::io;

/// Convenience function: create and run the TUI app.
pub fn run() -> io::Result<()> {
    let mut app = App::new();
    app.run()
}
