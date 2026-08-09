//! Homebrew backend for macOS (and Linux).
//!
//! [`BrewManager::new`] returns `None` when `brew` is not on `PATH`.
//! All operations use `tokio::process::Command` and capture stdout/stderr.

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;
use tracing::{debug, instrument, warn};

use toolshub_core::types::PackageManagerKind;
use toolshub_core::{Error, Result};

use crate::manager::{InstallResult, InstalledPackage, PackageInfo, PackageManager};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_brew() -> Option<String> {
    which::which("brew").ok().map(|p| p.to_string_lossy().into_owned())
}

async fn run_brew(args: &[&str]) -> Result<(String, String, bool)> {
    let brew = find_brew().ok_or_else(|| Error::CommandNotFound("brew".into()))?;
    let output = Command::new(&brew)
        .args(args)
        .output()
        .await
        .map_err(|e| Error::PackageManager(format!("Failed to spawn brew: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok((stdout, stderr, output.status.success()))
}

// ── JSON schema for `brew info --json=v2` ────────────────────────────────────

#[derive(Deserialize)]
struct BrewInfoResponse {
    formulae: Vec<BrewFormula>,
    casks: Vec<BrewCask>,
}

#[derive(Deserialize)]
struct BrewFormula {
    name: String,
    desc: Option<String>,
    homepage: Option<String>,
    versions: BrewVersions,
    installed: Vec<BrewInstalled>,
}

#[derive(Deserialize)]
struct BrewVersions {
    stable: Option<String>,
}

#[derive(Deserialize)]
struct BrewInstalled {
    version: String,
}

#[derive(Deserialize)]
struct BrewCask {
    token: String,
    desc: Option<String>,
    homepage: Option<String>,
    version: Option<String>,
    installed: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// Homebrew package manager backend.
pub struct BrewManager;

impl BrewManager {
    /// Construct a [`BrewManager`] if `brew` is found on `PATH`.
    pub fn new() -> Option<Self> {
        if find_brew().is_some() {
            debug!("Homebrew detected");
            Some(Self)
        } else {
            None
        }
    }

    /// Parse `brew list --versions` output.
    ///
    /// Each line: `name version [version …]`
    fn parse_list_versions(raw: &str) -> Vec<InstalledPackage> {
        raw.lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?.trim().to_string();
                let version = parts
                    .next()
                    .and_then(|v| v.split_whitespace().next())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    return None;
                }
                Some(InstalledPackage {
                    name,
                    version,
                    manager: PackageManagerKind::Brew,
                    install_date: None,
                })
            })
            .collect()
    }

    /// Parse `brew search` output (plain-text, one name per line).
    fn parse_search_output(raw: &str) -> Vec<PackageInfo> {
        raw.lines()
            .filter_map(|line| {
                let name = line.trim().to_string();
                // Skip section headers like "==> Formulae"
                if name.starts_with("==>") || name.is_empty() {
                    return None;
                }
                Some(PackageInfo {
                    name,
                    version: None,
                    description: None,
                    homepage: None,
                    installed: false,
                    manager: PackageManagerKind::Brew,
                })
            })
            .collect()
    }

    /// Convert a [`BrewFormula`] from `brew info --json=v2` into [`PackageInfo`].
    fn formula_to_info(f: BrewFormula) -> PackageInfo {
        let installed = !f.installed.is_empty();
        let version = f
            .installed
            .first()
            .map(|i| i.version.clone())
            .or(f.versions.stable);
        PackageInfo {
            name: f.name,
            version,
            description: f.desc,
            homepage: f.homepage,
            installed,
            manager: PackageManagerKind::Brew,
        }
    }

    /// Convert a [`BrewCask`] from `brew info --json=v2` into [`PackageInfo`].
    fn cask_to_info(c: BrewCask) -> PackageInfo {
        let installed = c.installed.is_some();
        PackageInfo {
            name: c.token,
            version: c.version,
            description: c.desc,
            homepage: c.homepage,
            installed,
            manager: PackageManagerKind::Brew,
        }
    }
}

// ── PackageManager impl ───────────────────────────────────────────────────────

#[async_trait]
impl PackageManager for BrewManager {
    fn kind(&self) -> PackageManagerKind {
        PackageManagerKind::Brew
    }

    fn name(&self) -> &str {
        "brew"
    }

    fn is_available(&self) -> bool {
        find_brew().is_some()
    }

    #[instrument(skip(self))]
    async fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let (stdout, stderr, ok) = run_brew(&["search", query]).await?;
        if !ok {
            warn!("brew search returned non-zero: {stderr}");
        }
        Ok(Self::parse_search_output(&stdout))
    }

    #[instrument(skip(self))]
    async fn install(&self, package: &str) -> Result<InstallResult> {
        let (stdout, stderr, success) = run_brew(&["install", package]).await?;
        let combined = format!("{stdout}{stderr}");

        if !success {
            return Err(Error::PackageManager(format!(
                "brew install failed for `{package}`: {stderr}"
            )));
        }

        // Extract version from lines like "🍺  /usr/local/Cellar/ripgrep/14.1.0: ..."
        let version_installed = stdout.lines().find_map(|l| {
            let l = l.trim();
            // The Cellar path contains the version as the last path component before ':'
            if l.contains("/Cellar/") && l.contains(':') {
                let path_part = l.split(':').next()?;
                path_part.split('/').last().map(|s| s.to_string())
            } else {
                None
            }
        });

        Ok(InstallResult {
            success,
            package: package.to_string(),
            version_installed,
            output: combined,
        })
    }

    #[instrument(skip(self))]
    async fn remove(&self, package: &str) -> Result<()> {
        let (_stdout, stderr, success) = run_brew(&["uninstall", package]).await?;
        if !success {
            return Err(Error::PackageManager(format!(
                "brew uninstall failed for `{package}`: {stderr}"
            )));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        let (stdout, stderr, ok) = run_brew(&["list", "--versions"]).await?;
        if !ok {
            return Err(Error::PackageManager(format!(
                "brew list --versions failed: {stderr}"
            )));
        }
        Ok(Self::parse_list_versions(&stdout))
    }

    #[instrument(skip(self))]
    async fn info(&self, package: &str) -> Result<Option<PackageInfo>> {
        let (stdout, _stderr, ok) = run_brew(&["info", "--json=v2", package]).await?;
        if !ok || stdout.trim().is_empty() {
            return Ok(None);
        }

        let resp: BrewInfoResponse = serde_json::from_str(&stdout).map_err(|e| {
            Error::PackageManager(format!("brew info JSON parse error: {e}"))
        })?;

        // Prefer formula over cask
        if let Some(formula) = resp.formulae.into_iter().next() {
            return Ok(Some(Self::formula_to_info(formula)));
        }
        if let Some(cask) = resp.casks.into_iter().next() {
            return Ok(Some(Self::cask_to_info(cask)));
        }

        Ok(None)
    }

    fn install_command(&self, package: &str) -> String {
        format!("brew install {}", package)
    }

    fn remove_command(&self, package: &str) -> String {
        format!("brew uninstall {}", package)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_versions_basic() {
        let raw = "ripgrep 14.1.0\ncurl 8.6.0\ngh 2.43.1 2.44.0\n";
        let installed = BrewManager::parse_list_versions(raw);
        assert_eq!(installed.len(), 3);
        assert_eq!(installed[0].name, "ripgrep");
        assert_eq!(installed[0].version, "14.1.0");
        // When multiple versions are listed, take the first
        assert_eq!(installed[2].version, "2.43.1");
    }

    #[test]
    fn parse_search_output_skips_headers() {
        let raw = "==> Formulae\nripgrep\ncurl\n==> Casks\nchrome\n";
        let results = BrewManager::parse_search_output(raw);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].name, "ripgrep");
    }

    #[test]
    fn install_command_format() {
        let mgr = BrewManager;
        assert_eq!(mgr.install_command("jq"), "brew install jq");
    }
}
