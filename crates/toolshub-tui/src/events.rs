//! Keyboard and mouse input handlers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{ActivePanel, AppState, ChatMessage, ChatRole};

// ── Keyboard ──────────────────────────────────────────────────────────────────

pub fn handle_key(key: KeyEvent, state: &mut AppState) {
    // Global quit: Ctrl+Q or Ctrl+C
    if matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('q'), KeyModifiers::CONTROL)
            | (KeyCode::Char('c'), KeyModifiers::CONTROL)
    ) {
        state.should_quit = true;
        return;
    }

    // Number keys 1-6: jump to panel
    if key.modifiers == KeyModifiers::NONE {
        match key.code {
            KeyCode::Char('1') => { state.active_panel = ActivePanel::Output;   update_status(state); return; }
            KeyCode::Char('2') => { state.active_panel = ActivePanel::Tools;    update_status(state); return; }
            KeyCode::Char('3') => { state.active_panel = ActivePanel::Chat;     update_status(state); return; }
            KeyCode::Char('4') => { state.active_panel = ActivePanel::Packages; update_status(state); return; }
            KeyCode::Char('5') => { state.active_panel = ActivePanel::Services; update_status(state); return; }
            KeyCode::Char('6') => { state.active_panel = ActivePanel::Memory;   update_status(state); return; }
            _ => {}
        }
    }

    // Tab / Shift+Tab: cycle panels
    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        state.active_panel = state.active_panel.next();
        update_status(state);
        return;
    }
    if key.code == KeyCode::BackTab {
        state.active_panel = state.active_panel.prev();
        update_status(state);
        return;
    }

    // Delegate to panel-specific handler
    match state.active_panel {
        ActivePanel::Output   => handle_output(key, state),
        ActivePanel::Tools    => handle_tools(key, state),
        ActivePanel::Chat     => handle_chat(key, state),
        ActivePanel::Packages => handle_packages(key, state),
        ActivePanel::Services => handle_services(key, state),
        ActivePanel::Memory   => handle_memory(key, state),
    }
}

// ── Mouse ─────────────────────────────────────────────────────────────────────

pub fn handle_mouse(mouse: MouseEvent, state: &mut AppState) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            match state.active_panel {
                ActivePanel::Output => {
                    state.output_scroll = state.output_scroll.saturating_sub(3);
                }
                ActivePanel::Tools => { state.tools_up(); }
                ActivePanel::Services => { state.svc_up(); }
                ActivePanel::Memory => { state.mem_up(); }
                ActivePanel::Packages => { state.pkg_up(); }
                _ => {}
            }
        }
        MouseEventKind::ScrollDown => {
            match state.active_panel {
                ActivePanel::Output => {
                    let max = state.output.len().saturating_sub(1);
                    state.output_scroll = (state.output_scroll + 3).min(max);
                }
                ActivePanel::Tools => { state.tools_down(); }
                ActivePanel::Services => { state.svc_down(); }
                ActivePanel::Memory => { state.mem_down(); }
                ActivePanel::Packages => { state.pkg_down(); }
                _ => {}
            }
        }
        // Click on tab bar — tabs are roughly the top 3 rows
        MouseEventKind::Down(_) => {
            if mouse.row <= 2 {
                // Approximate tab positions based on column
                let col = mouse.column;
                let panel = if col < 12 {
                    ActivePanel::Output
                } else if col < 22 {
                    ActivePanel::Tools
                } else if col < 32 {
                    ActivePanel::Chat
                } else if col < 46 {
                    ActivePanel::Packages
                } else if col < 60 {
                    ActivePanel::Services
                } else {
                    ActivePanel::Memory
                };
                state.active_panel = panel;
                update_status(state);
            }
        }
        _ => {}
    }
}

// ── Panel handlers ────────────────────────────────────────────────────────────

fn handle_output(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Up => {
            state.output_scroll = state.output_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            let max = state.output.len().saturating_sub(1);
            state.output_scroll = (state.output_scroll + 1).min(max);
        }
        KeyCode::PageUp => {
            state.output_scroll = state.output_scroll.saturating_sub(20);
        }
        KeyCode::PageDown => {
            let max = state.output.len().saturating_sub(1);
            state.output_scroll = (state.output_scroll + 20).min(max);
        }
        KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
            state.output_scroll = 0;
        }
        KeyCode::Char('G') => {
            state.output_scroll = state.output.len().saturating_sub(1);
        }
        // Input editing
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            state.input.insert(state.input_cursor, c);
            state.input_cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if state.input_cursor > 0 {
                state.input_cursor -= 1;
                state.input.remove(state.input_cursor);
            }
        }
        KeyCode::Left => {
            if state.input_cursor > 0 { state.input_cursor -= 1; }
        }
        KeyCode::Right => {
            if state.input_cursor < state.input.len() { state.input_cursor += 1; }
        }
        KeyCode::Home => { state.input_cursor = 0; }
        KeyCode::End  => { state.input_cursor = state.input.len(); }
        KeyCode::Enter => {
            let cmd = state.input.trim().to_string();
            if !cmd.is_empty() {
                state.push_output(format!("  > {cmd}"));
                state.push_output(format!("  [Run: toolshub {cmd}]"));
                state.input.clear();
                state.input_cursor = 0;
                state.status_msg = format!("Ran: {cmd}");
            }
        }
        _ => {}
    }
}

fn handle_tools(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Up   => { state.tools_up(); }
        KeyCode::Down => { state.tools_down(); }
        KeyCode::Enter => {
            if let Some(tool) = state.selected_tool() {
                state.push_output(format!("  > toolshub explain {tool}"));
                state.push_output(format!("  Tip: run 'toolshub explain {tool}' in your terminal for full output"));
                state.active_panel = ActivePanel::Output;
                state.status_msg = format!("Explaining {tool}...");
            }
        }
        KeyCode::Backspace => {
            state.tools_filter.pop();
            // Reset selection when filter changes
            if !state.filtered_tools().is_empty() {
                state.tools_list_state.select(Some(0));
            }
        }
        KeyCode::Esc => {
            state.tools_filter.clear();
            if !state.list_items.is_empty() {
                state.tools_list_state.select(Some(0));
            }
        }
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            state.tools_filter.push(c);
            if !state.filtered_tools().is_empty() {
                state.tools_list_state.select(Some(0));
            } else {
                state.tools_list_state.select(None);
            }
        }
        _ => {}
    }
}

fn handle_chat(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            state.chat_input.insert(state.chat_cursor, c);
            state.chat_cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if state.chat_cursor > 0 {
                state.chat_cursor -= 1;
                state.chat_input.remove(state.chat_cursor);
            }
        }
        KeyCode::Left  => { if state.chat_cursor > 0 { state.chat_cursor -= 1; } }
        KeyCode::Right => { if state.chat_cursor < state.chat_input.len() { state.chat_cursor += 1; } }
        KeyCode::Home  => { state.chat_cursor = 0; }
        KeyCode::End   => { state.chat_cursor = state.chat_input.len(); }
        KeyCode::Esc   => { state.chat_input.clear(); state.chat_cursor = 0; }
        KeyCode::Enter => {
            let msg = state.chat_input.trim().to_string();
            if !msg.is_empty() {
                state.chat_history.push(ChatMessage { role: ChatRole::User, content: msg.clone() });
                state.chat_history.push(ChatMessage {
                    role: ChatRole::Ai,
                    content: "[AI is thinking… run 'toolshub ask' in terminal for live responses]".into(),
                });
                state.chat_input.clear();
                state.chat_cursor = 0;
                state.status_msg = "Message sent — check terminal for AI responses".into();
            }
        }
        _ => {}
    }
}

fn handle_packages(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            state.pkg_query.insert(state.pkg_cursor, c);
            state.pkg_cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if state.pkg_cursor > 0 {
                state.pkg_cursor -= 1;
                state.pkg_query.remove(state.pkg_cursor);
            }
        }
        KeyCode::Left  => { if state.pkg_cursor > 0 { state.pkg_cursor -= 1; } }
        KeyCode::Right => { if state.pkg_cursor < state.pkg_query.len() { state.pkg_cursor += 1; } }
        KeyCode::Esc   => { state.pkg_query.clear(); state.pkg_cursor = 0; state.pkg_results.clear(); }
        KeyCode::Up    => { state.pkg_up(); }
        KeyCode::Down  => { state.pkg_down(); }
        KeyCode::Enter => {
            let q = state.pkg_query.trim().to_string();
            if !q.is_empty() {
                // Simulate search results for demo purposes
                state.pkg_results = vec![
                    format!("{q} (pacman) — v1.0.0"),
                    format!("{q}-git (AUR) — latest"),
                    format!("lib{q} (pacman) — v0.9.0"),
                    format!("{q}-utils (AUR) — v2.1.0"),
                ];
                if !state.pkg_results.is_empty() {
                    state.pkg_list_state.select(Some(0));
                }
                state.status_msg = format!("Found {} results for '{q}'", state.pkg_results.len());
            }
        }
        _ => {}
    }
}

fn handle_services(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Up   => { state.svc_up(); }
        KeyCode::Down => { state.svc_down(); }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::NONE => {
            if let Some(i) = state.svc_list_state.selected() {
                if let Some(svc) = state.services.get_mut(i) {
                    svc.active = "activating".into();
                    svc.sub    = "start".into();
                    state.status_msg = format!("Restarting {}...", svc.name);
                }
            }
        }
        KeyCode::Char('s') if key.modifiers == KeyModifiers::NONE => {
            if let Some(i) = state.svc_list_state.selected() {
                if let Some(svc) = state.services.get_mut(i) {
                    svc.active = "active".into();
                    svc.sub    = "running".into();
                    state.status_msg = format!("Started {}", svc.name);
                }
            }
        }
        KeyCode::Char('S') => {
            if let Some(i) = state.svc_list_state.selected() {
                if let Some(svc) = state.services.get_mut(i) {
                    svc.active = "inactive".into();
                    svc.sub    = "dead".into();
                    state.status_msg = format!("Stopped {}", svc.name);
                }
            }
        }
        _ => {}
    }
}

fn handle_memory(key: KeyEvent, state: &mut AppState) {
    match key.code {
        KeyCode::Up   => { state.mem_up(); }
        KeyCode::Down => { state.mem_down(); }
        KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
            state.mem_toggle_mark();
            state.status_msg = "Fact marked for deletion (press D to confirm)".into();
        }
        KeyCode::Char('D') => {
            // Remove all marked facts
            let mut i = 0;
            while i < state.memory_facts.len() {
                if state.memory_marked.get(i).copied().unwrap_or(false) {
                    state.memory_facts.remove(i);
                    state.memory_marked.remove(i);
                } else {
                    i += 1;
                }
            }
            state.memory_list_state.select(
                if state.memory_facts.is_empty() { None } else { Some(0) }
            );
            state.status_msg = "Marked facts deleted".into();
        }
        _ => {}
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn update_status(state: &mut AppState) {
    state.status_msg = match state.active_panel {
        ActivePanel::Output   => "Output — PgUp/PgDn scroll, g/G top/bottom, type below".into(),
        ActivePanel::Tools    => "Tools — Type to filter, ↑↓ navigate, Enter to explain".into(),
        ActivePanel::Chat     => "Chat — Type a question and press Enter".into(),
        ActivePanel::Packages => "Packages — Type package name and press Enter to search".into(),
        ActivePanel::Services => "Services — ↑↓ navigate, r=restart s=start S=stop".into(),
        ActivePanel::Memory   => "Memory — ↑↓ navigate, d=mark for deletion, D=delete marked".into(),
    };
}
