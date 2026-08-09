//! `toolshub-env` — Environment detection for ToolsHub.
//!
//! This crate exposes a single high-level entry point, [`EnvironmentSnapshot::capture`],
//! which concurrently gathers information about:
//!
//! * The host operating system and distribution
//! * The active shell
//! * Installed package managers
//! * Hardware (CPU, RAM, GPU, architecture)
//! * Every executable tool found on the user's `$PATH`
//!
//! # Example
//!
//! ```no_run
//! use toolshub_env::EnvironmentSnapshot;
//!
//! #[tokio::main]
//! async fn main() {
//!     let snapshot = EnvironmentSnapshot::capture().await.unwrap();
//!     println!("OS: {:?}", snapshot.os);
//!     println!("Tools found: {}", snapshot.installed_tools.len());
//! }
//! ```

pub mod hardware;
pub mod os;
pub mod packages;
pub mod shell;
pub mod tools;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, instrument};

use toolshub_core::types::{
    DetectedPackageManager, HardwareInfo, InstalledTool, OperatingSystem, Shell,
};
use toolshub_core::Result;

/// A point-in-time snapshot of the user's development environment.
///
/// Construct this via [`EnvironmentSnapshot::capture`]. The struct is
/// fully serialisable so it can be stored in ToolsHub's SQLite database
/// or sent to AI backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Detected operating system.
    pub os: OperatingSystem,

    /// Detected interactive shell.
    pub shell: Shell,

    /// Hardware information (CPU, RAM, GPU, architecture).
    pub hardware: HardwareInfo,

    /// Every package manager found on the system.
    pub package_managers: Vec<DetectedPackageManager>,

    /// Every executable tool found on `$PATH`.
    pub installed_tools: Vec<InstalledTool>,

    /// Ordered list of directories that make up `$PATH`.
    pub path_entries: Vec<PathBuf>,

    /// The user's home directory.
    pub home_dir: PathBuf,

    /// UTC timestamp when the snapshot was taken.
    pub detected_at: DateTime<Utc>,
}

impl EnvironmentSnapshot {
    /// Capture a complete environment snapshot.
    ///
    /// All sub-probes are run concurrently where possible to keep the total
    /// wall-clock time low. Failures inside individual probes are handled
    /// gracefully — the snapshot is always returned, possibly with partial data.
    #[instrument(name = "env_capture")]
    pub async fn capture() -> Result<Self> {
        info!("Starting environment snapshot capture");

        let path_entries = collect_path_entries();
        let home_dir = dirs_home();

        debug!(path_entries = path_entries.len(), home_dir = %home_dir.display());

        // Launch independent probes concurrently.
        let (os_result, hardware, package_managers) = tokio::join!(
            os::detect_os(),
            hardware::detect_hardware(),
            packages::detect_package_managers(),
        );

        let os = os_result?;

        // Shell detection is synchronous (reads env vars / /proc).
        let shell = shell::detect_shell();

        // Tool discovery depends on path_entries — run after path is known.
        let installed_tools = tools::discover_tools(&path_entries).await;

        info!(
            os = ?os,
            shell = ?shell,
            package_managers = package_managers.len(),
            installed_tools = installed_tools.len(),
            "Environment snapshot capture complete"
        );

        Ok(EnvironmentSnapshot {
            os,
            shell,
            hardware,
            package_managers,
            installed_tools,
            path_entries,
            home_dir,
            detected_at: Utc::now(),
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse `$PATH` into an ordered list of [`PathBuf`]s, deduplicating entries
/// while preserving order (first occurrence wins).
fn collect_path_entries() -> Vec<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let separator = if cfg!(target_os = "windows") { ';' } else { ':' };

    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();

    for part in path_var.split(separator) {
        if part.is_empty() {
            continue;
        }
        let pb = PathBuf::from(part);
        if seen.insert(pb.clone()) {
            entries.push(pb);
        }
    }

    entries
}

/// Return the user's home directory. Falls back to `/root` → `/` if all else
/// fails.
fn dirs_home() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        return home;
    }

    // Windows: USERPROFILE or HOMEDRIVE + HOMEPATH
    #[cfg(target_os = "windows")]
    {
        if let Ok(up) = std::env::var("USERPROFILE") {
            return PathBuf::from(up);
        }
        if let (Ok(drive), Ok(path)) = (
            std::env::var("HOMEDRIVE"),
            std::env::var("HOMEPATH"),
        ) {
            return PathBuf::from(format!("{drive}{path}"));
        }
    }

    PathBuf::from("/")
}
