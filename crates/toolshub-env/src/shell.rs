//! Shell detection module.
//!
//! Determines the user's current interactive shell using multiple heuristics:
//! 1. `$SHELL` environment variable (most reliable on Unix-like systems)
//! 2. `$0` environment variable (set by some shells to their name)
//! 3. `/proc/self/status` parent-process inspection on Linux
//! 4. Default fallback

use toolshub_core::types::Shell;
use tracing::debug;

/// Detect the current shell and return a typed [`Shell`] value.
///
/// This function never fails — it returns [`Shell::Unknown`] when detection
/// is not possible.
pub fn detect_shell() -> Shell {
    // Strategy 1: $SHELL variable (set by login shells on Unix).
    if let Ok(shell_path) = std::env::var("SHELL") {
        let shell = parse_shell_path(&shell_path);
        if !matches!(shell, Shell::Unknown(_)) {
            debug!(shell_path = %shell_path, "Detected shell from $SHELL");
            return shell;
        }
    }

    // Strategy 2: $0 (some shells set this to their argv[0]).
    if let Ok(zero) = std::env::var("0") {
        let shell = parse_shell_path(&zero);
        if !matches!(shell, Shell::Unknown(_)) {
            debug!(zero = %zero, "Detected shell from $0");
            return shell;
        }
    }

    // Strategy 3: Inspect the parent-process name via /proc/self/status (Linux).
    #[cfg(target_os = "linux")]
    if let Some(shell) = detect_shell_from_proc() {
        debug!(?shell, "Detected shell from /proc/self/status");
        return shell;
    }

    // Strategy 4: Look at the COMSPEC or PSModulePath env vars on Windows.
    #[cfg(target_os = "windows")]
    {
        if std::env::var("PSModulePath").is_ok() {
            debug!("Detected PowerShell from PSModulePath env var");
            return Shell::PowerShell;
        }
        if std::env::var("COMSPEC").is_ok() {
            debug!("Detected Cmd from COMSPEC env var");
            return Shell::Cmd;
        }
    }

    let unknown = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());
    debug!(unknown = %unknown, "Could not detect shell, returning Unknown");
    Shell::Unknown(unknown)
}

/// Parse a shell binary path (e.g. `/usr/bin/zsh`, `bash`) into a [`Shell`].
fn parse_shell_path(path: &str) -> Shell {
    // Take only the final component of the path for matching.
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_lowercase();

    match name.as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "nu" | "nushell" => Shell::Nu,
        "sh" => Shell::Sh,
        "dash" => Shell::Dash,
        "ksh" | "ksh93" | "ksh2020" => Shell::Ksh,
        "tcsh" | "csh" => Shell::Tcsh,
        "elvish" => Shell::Elvish,
        "xonsh" => Shell::Xonsh,
        "ion" => Shell::Ion,
        "powershell" | "pwsh" => Shell::PowerShell,
        "cmd" | "cmd.exe" => Shell::Cmd,
        _ => Shell::Unknown(path.to_string()),
    }
}

/// On Linux, walk up the process tree via `/proc/<pid>/status` to find the
/// parent process name. We look for a process whose name matches a known shell.
#[cfg(target_os = "linux")]
fn detect_shell_from_proc() -> Option<Shell> {
    use std::fs;

    let status = fs::read_to_string("/proc/self/status").ok()?;
    let ppid_str = status
        .lines()
        .find(|l| l.starts_with("PPid:"))?
        .split_whitespace()
        .nth(1)?;

    let ppid: u32 = ppid_str.parse().ok()?;
    let comm_path = format!("/proc/{ppid}/comm");
    let comm = fs::read_to_string(comm_path).ok()?;
    let comm = comm.trim();

    let shell = parse_shell_path(comm);
    if matches!(shell, Shell::Unknown(_)) {
        None
    } else {
        Some(shell)
    }
}
