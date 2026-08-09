//! Tool discovery module.
//!
//! Scans each directory in `$PATH`, enumerates executable files, and
//! attempts to determine the version and owning package manager for each.
//!
//! Discovery is parallelised with [`tokio::task::spawn`] — each PATH entry
//! is processed concurrently so the scan completes quickly even on machines
//! with large PATHs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::process::Command;
use tracing::{debug, trace, warn};

use toolshub_core::types::{InstalledTool, PackageManagerKind};

/// Known tool-name → package manager mappings.
///
/// This is a best-effort hint; it won't be correct for every system but
/// covers the most common cases.
static TOOL_PM_HINTS: &[(&str, PackageManagerKind)] = &[
    // Rust toolchain
    ("cargo", PackageManagerKind::Cargo),
    ("rustc", PackageManagerKind::Cargo),
    ("rustup", PackageManagerKind::Cargo),
    ("clippy-driver", PackageManagerKind::Cargo),
    // Node ecosystem
    ("node", PackageManagerKind::Npm),
    ("npm", PackageManagerKind::Npm),
    ("npx", PackageManagerKind::Npm),
    ("pnpm", PackageManagerKind::Pnpm),
    ("yarn", PackageManagerKind::Yarn),
    ("bun", PackageManagerKind::Bun),
    ("bunx", PackageManagerKind::Bun),
    // Python ecosystem
    ("python3", PackageManagerKind::Pip),
    ("python", PackageManagerKind::Pip),
    ("pip3", PackageManagerKind::Pip),
    ("pip", PackageManagerKind::Pip),
    ("pipx", PackageManagerKind::Pipx),
    ("uv", PackageManagerKind::Pip),
    // Go
    ("go", PackageManagerKind::Go),
    // Homebrew managed (macOS / Linux brew)
    ("brew", PackageManagerKind::Brew),
    ("gcc", PackageManagerKind::Brew),
    // Arch / AUR
    ("pacman", PackageManagerKind::Pacman),
    ("yay", PackageManagerKind::Yay),
    ("paru", PackageManagerKind::Paru),
    // Debian / Ubuntu
    ("apt", PackageManagerKind::Apt),
    ("apt-get", PackageManagerKind::Apt),
    // Fedora / RHEL
    ("dnf", PackageManagerKind::Dnf),
    ("yum", PackageManagerKind::Yum),
    // Flatpak / Snap
    ("flatpak", PackageManagerKind::Flatpak),
    ("snap", PackageManagerKind::Snap),
];

/// Version probe strategies: try each flag in order, stop at first success.
static VERSION_FLAGS: &[&str] = &["--version", "-V", "-v", "version", "--ver"];

/// Maximum number of tools discovered per PATH entry (prevents runaway scans
/// on unusual systems with enormous directories).
const MAX_TOOLS_PER_DIR: usize = 500;

/// Discover installed tools by scanning the provided PATH entries.
///
/// For each executable found, the function:
/// 1. Checks whether it is a regular executable file.
/// 2. Attempts to retrieve a version string.
/// 3. Associates it with a package manager when possible.
///
/// Results are deduplicated by binary name (first occurrence wins, preserving
/// PATH priority ordering).
pub async fn discover_tools(path_entries: &[PathBuf]) -> Vec<InstalledTool> {
    // Build a fast lookup table for PM hints.
    let pm_hints: HashMap<&str, PackageManagerKind> = TOOL_PM_HINTS.iter().cloned().collect();

    // Process each PATH entry in parallel.
    let mut tasks = Vec::new();
    for dir in path_entries {
        let dir = dir.clone();
        let pm_hints = pm_hints.clone();
        tasks.push(tokio::spawn(async move {
            scan_directory(&dir, &pm_hints).await
        }));
    }

    // Collect results, deduplicate by tool name (PATH order = priority).
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut tools: Vec<InstalledTool> = Vec::new();

    for task in tasks {
        match task.await {
            Ok(dir_tools) => {
                for tool in dir_tools {
                    if seen_names.insert(tool.name.clone()) {
                        tools.push(tool);
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Tool discovery task panicked");
            }
        }
    }

    debug!(total = tools.len(), "Tool discovery complete");
    tools
}

/// Scan a single directory and return all discovered tools.
async fn scan_directory(
    dir: &Path,
    pm_hints: &HashMap<&str, PackageManagerKind>,
) -> Vec<InstalledTool> {
    let mut tools = Vec::new();

    let mut entries = match fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return tools, // Directory not accessible — skip silently.
    };

    let mut count = 0usize;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if count >= MAX_TOOLS_PER_DIR {
            warn!(
                dir = %dir.display(),
                "Reached per-directory tool limit ({MAX_TOOLS_PER_DIR}), stopping scan"
            );
            break;
        }

        let path = entry.path();

        // Only regular executable files (skip directories, symlinks to dirs, etc.)
        if !is_executable_file(&path).await {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        trace!(name = %name, path = %path.display(), "Found executable");

        let version = probe_version(&path).await;
        let package_manager = pm_hints.get(name.as_str()).cloned();

        tools.push(InstalledTool {
            name,
            path,
            version,
            package_manager,
            description: None,
        });

        count += 1;
    }

    tools
}

/// Return `true` if the path is a regular file (or symlink to one) that is
/// executable by the current user.
async fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path).await else {
        return false;
    };

    if !meta.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Check if any execute bit is set (owner / group / other).
        meta.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        // On Windows, assume all files in PATH are executable.
        true
    }
}

/// Attempt to retrieve a version string for the given executable.
///
/// Tries each flag in [`VERSION_FLAGS`] and returns the first non-empty
/// first line of output, or `None` if all attempts fail.
async fn probe_version(path: &Path) -> Option<String> {
    for flag in VERSION_FLAGS {
        if let Some(version) = try_version_flag(path, flag).await {
            return Some(version);
        }
    }
    None
}

/// Run `<path> <flag>` with a short timeout and return the cleaned first line.
async fn try_version_flag(path: &Path, flag: &str) -> Option<String> {
    use tokio::time::{timeout, Duration};

    let future = Command::new(path)
        .arg(flag)
        .output();

    // 2-second timeout per probe to avoid hanging on interactive prompts.
    let out = timeout(Duration::from_secs(2), future).await.ok()?.ok()?;

    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }

    // Many tools print to stderr (e.g. curl). Prefer stdout.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = if stdout.trim().is_empty() { stderr } else { stdout };

    let first_line = combined
        .lines()
        .find(|l| !l.trim().is_empty())?
        .trim()
        .to_string();

    // Sanity check: the output must look like a version string (contains a
    // digit somewhere). This filters out binary garbage and pure error text.
    if first_line.chars().any(|c| c.is_ascii_digit()) {
        Some(first_line)
    } else {
        None
    }
}
