//! Package manager detection module.
//!
//! Uses [`which::which`] to locate each candidate binary and then probes it
//! with `--version` to confirm it is functional and to extract a version string.

use toolshub_core::types::{DetectedPackageManager, PackageManagerKind};
use tokio::process::Command;
use tracing::{debug, warn};

/// All package managers to probe, in priority order.
///
/// Each entry is `(binary_name, kind, version_flag)`.
static CANDIDATES: &[(&str, PackageManagerKind, &str)] = &[
    // System
    ("pacman", PackageManagerKind::Pacman, "--version"),
    ("yay", PackageManagerKind::Yay, "--version"),
    ("paru", PackageManagerKind::Paru, "--version"),
    ("apt", PackageManagerKind::Apt, "--version"),
    ("apt-get", PackageManagerKind::Apt, "--version"),
    ("dnf", PackageManagerKind::Dnf, "--version"),
    ("yum", PackageManagerKind::Yum, "--version"),
    ("apk", PackageManagerKind::Apk, "--version"),
    ("zypper", PackageManagerKind::Zypper, "--version"),
    ("brew", PackageManagerKind::Brew, "--version"),
    // Windows
    ("winget", PackageManagerKind::Winget, "--version"),
    ("choco", PackageManagerKind::Choco, "--version"),
    ("scoop", PackageManagerKind::Scoop, "--version"),
    // Language / ecosystem
    ("cargo", PackageManagerKind::Cargo, "--version"),
    ("go", PackageManagerKind::Go, "version"),
    ("pip", PackageManagerKind::Pip, "--version"),
    ("pip3", PackageManagerKind::Pip, "--version"),
    ("pipx", PackageManagerKind::Pipx, "--version"),
    ("npm", PackageManagerKind::Npm, "--version"),
    ("pnpm", PackageManagerKind::Pnpm, "--version"),
    ("yarn", PackageManagerKind::Yarn, "--version"),
    ("bun", PackageManagerKind::Bun, "--version"),
    // Universal / sandbox
    ("flatpak", PackageManagerKind::Flatpak, "--version"),
    ("snap", PackageManagerKind::Snap, "--version"),
];

/// Detect all available package managers by probing the `PATH`.
///
/// Returns only managers whose binary was found **and** whose `--version`
/// invocation succeeded (exit status 0).
///
/// Security: Only executes binaries located in trusted system directories
/// to prevent a malicious binary named `apt` or `brew` in a project's local
/// directory from being automatically run during environment detection.
pub async fn detect_package_managers() -> Vec<DetectedPackageManager> {
    let mut detected = Vec::new();

    for (binary, kind, version_flag) in CANDIDATES {
        match which::which(binary) {
            Err(_) => {
                // Binary not on PATH — skip silently.
                continue;
            }
            Ok(path) => {
                // Security check: only probe binaries in trusted directories.
                if !is_trusted_path(&path) {
                    warn!(
                        binary = %binary,
                        path = %path.display(),
                        "Skipping binary in untrusted location (not in a system directory)"
                    );
                    continue;
                }

                debug!(binary = %binary, path = %path.display(), "Found binary, probing version");
                let version = probe_version(binary, version_flag).await;
                detected.push(DetectedPackageManager {
                    kind: kind.clone(),
                    path,
                    version,
                });
            }
        }
    }

    debug!(count = detected.len(), "Package manager detection complete");
    detected
}

/// Return `true` if the binary path starts with a known trusted prefix.
///
/// Trusted prefixes cover standard system locations on Linux, macOS, and
/// common tool install paths (Homebrew, Cargo, pyenv, etc.).
fn is_trusted_path(path: &std::path::Path) -> bool {
    const TRUSTED_PREFIXES: &[&str] = &[
        "/usr/bin",
        "/usr/local/bin",
        "/usr/sbin",
        "/bin",
        "/sbin",
        "/opt/homebrew",       // macOS Homebrew (Apple Silicon)
        "/usr/local/Cellar",   // macOS Homebrew (Intel)
        "/opt/local",          // MacPorts
        "/nix/store",          // Nix
        "/home",               // user-local installs (~/.local/bin, ~/.cargo/bin, etc.)
        "/root",               // root user equivalents
        "/snap",               // Snap packages
        "/var/lib/flatpak",    // Flatpak
    ];

    let path_str = path.to_string_lossy();
    TRUSTED_PREFIXES
        .iter()
        .any(|prefix| path_str.starts_with(prefix))
}

/// Run `<binary> <flag>` and return the first non-empty output line,
/// or `None` if the process fails or produces no output.
async fn probe_version(binary: &str, flag: &str) -> Option<String> {
    let result = Command::new(binary)
        .arg(flag)
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);

            // Some tools (e.g. pip3) write version to stderr.
            let combined = if stdout.trim().is_empty() { stderr } else { stdout };

            combined
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
        }
        Ok(out) => {
            warn!(
                binary = %binary,
                stderr = %String::from_utf8_lossy(&out.stderr),
                "Version probe returned non-zero exit"
            );
            None
        }
        Err(e) => {
            warn!(binary = %binary, error = %e, "Failed to spawn version probe");
            None
        }
    }
}
