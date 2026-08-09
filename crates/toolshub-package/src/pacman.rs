//! Pacman / Yay / Paru backend for Arch Linux.
//!
//! Detection order: **yay** → **paru** → **pacman**.  If none are found on
//! `PATH`, [`PacmanManager::new`] returns `None`.

use async_trait::async_trait;
use tokio::process::Command;
use tracing::{debug, instrument, warn};

use toolshub_core::types::PackageManagerKind;
use toolshub_core::{Error, Result};

use crate::manager::{InstallResult, InstalledPackage, PackageInfo, PackageManager};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Try to locate `binary` via `which` crate.
fn find_binary(name: &str) -> Option<String> {
    which::which(name).ok().map(|p| p.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────

/// Arch Linux package manager backend.
///
/// Wraps `yay`, `paru`, or `pacman` — whichever is found first.
pub struct PacmanManager {
    /// Path (or bare name) of the binary to invoke.
    binary: String,
    /// The [`PackageManagerKind`] that matches `binary`.
    kind: PackageManagerKind,
}

impl PacmanManager {
    /// Detect and construct a [`PacmanManager`].
    ///
    /// Returns `None` when none of yay / paru / pacman can be located.
    pub fn new() -> Option<Self> {
        let candidates: &[(&str, PackageManagerKind)] = &[
            ("yay", PackageManagerKind::Yay),
            ("paru", PackageManagerKind::Paru),
            ("pacman", PackageManagerKind::Pacman),
        ];

        for (name, kind) in candidates {
            if let Some(path) = find_binary(name) {
                debug!("Selected Arch package manager: {path}");
                return Some(Self {
                    binary: path,
                    kind: kind.clone(),
                });
            }
        }
        None
    }

    // ── Private helpers ────────────────────────────────────────────────────

    /// Run a command, returning `(stdout, stderr, exit_success)`.
    async fn run_cmd(&self, args: &[&str]) -> Result<(String, String, bool)> {
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .await
            .map_err(|e| Error::PackageManager(format!("Failed to spawn {}: {e}", self.binary)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stdout, stderr, output.status.success()))
    }

    /// Run a command with `sudo`, returning `(stdout, stderr, exit_success)`.
    async fn run_sudo(&self, args: &[&str]) -> Result<(String, String, bool)> {
        let mut full_args = vec![self.binary.as_str()];
        full_args.extend_from_slice(args);

        let output = Command::new("sudo")
            .args(&full_args)
            .output()
            .await
            .map_err(|e| Error::PackageManager(format!("Failed to spawn sudo: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stdout, stderr, output.status.success()))
    }

    /// Parse `pacman -Ss` output into [`PackageInfo`] entries.
    ///
    /// Each record spans (at least) two lines:
    /// ```text
    /// repo/name version [flags]
    ///     Short description
    /// ```
    fn parse_search_output(&self, raw: &str) -> Vec<PackageInfo> {
        let mut results = Vec::new();
        let mut lines = raw.lines().peekable();

        while let Some(header) = lines.next() {
            // Header line: "repo/name version …"
            let Some(slash_pos) = header.find('/') else {
                continue;
            };
            let rest = &header[slash_pos + 1..];
            let mut parts = rest.splitn(2, ' ');
            let name = parts.next().unwrap_or("").trim().to_string();
            let version = parts.next().map(|v| v.split_whitespace().next().unwrap_or("").to_string());

            // Description line (indented)
            let description = lines
                .peek()
                .filter(|l| l.starts_with("    ") || l.starts_with('\t'))
                .map(|l| l.trim().to_string());
            if description.is_some() {
                lines.next();
            }

            if name.is_empty() {
                continue;
            }

            results.push(PackageInfo {
                name,
                version,
                description,
                homepage: None,
                installed: false,
                manager: self.kind.clone(),
            });
        }

        results
    }

    /// Parse `pacman -Si` output into a [`PackageInfo`].
    fn parse_info_output(&self, raw: &str, package: &str) -> Option<PackageInfo> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut description: Option<String> = None;
        let mut homepage: Option<String> = None;

        for line in raw.lines() {
            if let Some(val) = field_value(line, "Name") {
                name = Some(val.to_string());
            } else if let Some(val) = field_value(line, "Version") {
                version = Some(val.to_string());
            } else if let Some(val) = field_value(line, "Description") {
                description = Some(val.to_string());
            } else if let Some(val) = field_value(line, "URL") {
                homepage = Some(val.to_string());
            }
        }

        // If the output was empty or unparseable, return None.
        if name.is_none() && raw.trim().is_empty() {
            return None;
        }

        Some(PackageInfo {
            name: name.unwrap_or_else(|| package.to_string()),
            version,
            description,
            homepage,
            installed: false,
            manager: self.kind.clone(),
        })
    }
}

// ── PackageManager impl ───────────────────────────────────────────────────────

#[async_trait]
impl PackageManager for PacmanManager {
    fn kind(&self) -> PackageManagerKind {
        self.kind.clone()
    }

    fn name(&self) -> &str {
        &self.binary
    }

    fn is_available(&self) -> bool {
        find_binary(&self.binary).is_some()
    }

    #[instrument(skip(self), fields(manager = %self.binary))]
    async fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let (stdout, stderr, ok) = self.run_cmd(&["-Ss", query]).await?;
        if !ok && stdout.is_empty() {
            warn!("pacman -Ss returned non-zero: {stderr}");
        }
        Ok(self.parse_search_output(&stdout))
    }

    #[instrument(skip(self), fields(manager = %self.binary))]
    async fn install(&self, package: &str) -> Result<InstallResult> {
        let (stdout, stderr, success) = self.run_sudo(&["-S", "--noconfirm", package]).await?;
        let combined = format!("{stdout}{stderr}");

        // Attempt to extract installed version from output.
        let version_installed = stdout
            .lines()
            .find_map(|l| {
                // "installing package-name (x.y.z-1)..."
                let l = l.trim();
                if l.starts_with("installing ") || l.contains("upgraded") {
                    let open = l.rfind('(')?;
                    let close = l.rfind(')')?;
                    Some(l[open + 1..close].to_string())
                } else {
                    None
                }
            });

        if !success {
            return Err(Error::PackageManager(format!(
                "pacman install failed for `{package}`: {stderr}"
            )));
        }

        Ok(InstallResult {
            success,
            package: package.to_string(),
            version_installed,
            output: combined,
        })
    }

    #[instrument(skip(self), fields(manager = %self.binary))]
    async fn remove(&self, package: &str) -> Result<()> {
        let (_stdout, stderr, success) = self.run_sudo(&["-R", "--noconfirm", package]).await?;
        if !success {
            return Err(Error::PackageManager(format!(
                "pacman remove failed for `{package}`: {stderr}"
            )));
        }
        Ok(())
    }

    #[instrument(skip(self), fields(manager = %self.binary))]
    async fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        let (stdout, stderr, ok) = self.run_cmd(&["-Q"]).await?;
        if !ok {
            return Err(Error::PackageManager(format!("pacman -Q failed: {stderr}")));
        }

        let packages = stdout
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?.trim().to_string();
                let version = parts.next()?.trim().to_string();
                if name.is_empty() || version.is_empty() {
                    return None;
                }
                Some(InstalledPackage {
                    name,
                    version,
                    manager: self.kind.clone(),
                    install_date: None,
                })
            })
            .collect();

        Ok(packages)
    }

    #[instrument(skip(self), fields(manager = %self.binary))]
    async fn info(&self, package: &str) -> Result<Option<PackageInfo>> {
        let (stdout, _stderr, _ok) = self.run_cmd(&["-Si", package]).await?;
        Ok(self.parse_info_output(&stdout, package))
    }

    fn install_command(&self, package: &str) -> String {
        format!("sudo {} -S {}", self.binary, package)
    }

    fn remove_command(&self, package: &str) -> String {
        format!("sudo {} -R {}", self.binary, package)
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Extract the value from a `Key           : value` line, if the key matches.
fn field_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let colon = line.find(':')?;
    let k = line[..colon].trim();
    if k.eq_ignore_ascii_case(key) {
        Some(line[colon + 1..].trim())
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> PacmanManager {
        PacmanManager {
            binary: "pacman".into(),
            kind: PackageManagerKind::Pacman,
        }
    }

    #[test]
    fn parse_search_output_basic() {
        let raw = "core/zsh 5.9-4\n    A very advanced and programmable command interpreter (shell) for UNIX\nextra/zsh-completions 0.35.0-2\n    Additional completion definitions for Zsh\n";
        let mgr = make_manager();
        let results = mgr.parse_search_output(raw);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "zsh");
        assert_eq!(results[0].version.as_deref(), Some("5.9-4"));
        assert_eq!(
            results[0].description.as_deref(),
            Some("A very advanced and programmable command interpreter (shell) for UNIX")
        );
    }

    #[test]
    fn parse_info_output_basic() {
        let raw = "Name           : ripgrep\nVersion        : 14.1.0-1\nDescription    : A search tool that combines the usability of ag with the raw speed of grep\nURL            : https://github.com/BurntSushi/ripgrep\n";
        let mgr = make_manager();
        let info = mgr.parse_info_output(raw, "ripgrep").unwrap();
        assert_eq!(info.name, "ripgrep");
        assert_eq!(info.version.as_deref(), Some("14.1.0-1"));
    }

    #[test]
    fn install_command_format() {
        let mgr = make_manager();
        assert_eq!(mgr.install_command("htop"), "sudo pacman -S htop");
    }
}
