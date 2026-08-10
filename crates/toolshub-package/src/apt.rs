//! APT / APT-GET backend for Debian / Ubuntu systems.
//!
//! Detection order: **apt** → **apt-get**.  If neither binary exists on `PATH`,
//! [`AptManager::new`] returns `None`.

use async_trait::async_trait;
use tokio::process::Command;
use tracing::{debug, instrument, warn};

use toolshub_core::types::PackageManagerKind;
use toolshub_core::{Error, Result};

use crate::manager::{InstallResult, InstalledPackage, PackageInfo, PackageManager};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_binary(name: &str) -> Option<String> {
    which::which(name).ok().map(|p| p.to_string_lossy().into_owned())
}

// ─────────────────────────────────────────────────────────────────────────────

/// Debian / Ubuntu package manager backend.
///
/// Uses `apt-cache` for read-only queries and `apt-get` (with `sudo`) for
/// install / remove operations.  `dpkg-query` is used to enumerate installed
/// packages because it is faster and more reliable than parsing `apt list`.
pub struct AptManager {
    /// `apt` or `apt-get`, whichever is available.
    binary: String,
}

impl AptManager {
    /// Detect and construct an [`AptManager`].
    ///
    /// Returns `None` when neither `apt` nor `apt-get` is present.
    pub fn new() -> Option<Self> {
        for candidate in &["apt", "apt-get"] {
            if let Some(path) = find_binary(candidate) {
                debug!("Selected APT binary: {path}");
                return Some(Self { binary: path });
            }
        }
        None
    }

    // ── Private helpers ────────────────────────────────────────────────────

    async fn run_cmd(binary: &str, args: &[&str]) -> Result<(String, String, bool)> {
        let output = Command::new(binary)
            .args(args)
            .output()
            .await
            .map_err(|e| Error::PackageManager(format!("Failed to spawn {binary}: {e}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stdout, stderr, output.status.success()))
    }

    async fn run_sudo_apt(&self, args: &[&str]) -> Result<(String, String, bool)> {
        // Always use apt-get for non-interactive operations.
        let apt_get = find_binary("apt-get").unwrap_or_else(|| self.binary.clone());
        let mut full_args = vec![apt_get.as_str()];
        full_args.extend_from_slice(args);

        let output = Command::new("sudo")
            .args(&full_args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .output()
            .await
            .map_err(|e| Error::PackageManager(format!("Failed to spawn sudo: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stdout, stderr, output.status.success()))
    }

    /// Parse `apt-cache search` output.
    ///
    /// Format: `name - short description`
    fn parse_search_output(&self, raw: &str) -> Vec<PackageInfo> {
        raw.lines()
            .filter_map(|line| {
                let (name, desc) = line.split_once(" - ")?;
                let name = name.trim().to_string();
                if name.is_empty() {
                    return None;
                }
                Some(PackageInfo {
                    name,
                    version: None,
                    description: Some(desc.trim().to_string()),
                    homepage: None,
                    installed: false,
                    manager: PackageManagerKind::Apt,
                })
            })
            .collect()
    }

    /// Parse `apt-cache show` key/value output into a [`PackageInfo`].
    fn parse_show_output(&self, raw: &str, package: &str) -> Option<PackageInfo> {
        if raw.trim().is_empty() {
            return None;
        }

        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut description: Option<String> = None;
        let mut homepage: Option<String> = None;

        for line in raw.lines() {
            if let Some(v) = field_value(line, "Package") {
                name = Some(v.to_string());
            } else if let Some(v) = field_value(line, "Version") {
                version = Some(v.to_string());
            } else if let Some(v) = field_value(line, "Description") {
                description = Some(v.to_string());
            } else if let Some(v) = field_value(line, "Homepage") {
                homepage = Some(v.to_string());
            }
        }

        Some(PackageInfo {
            name: name.unwrap_or_else(|| package.to_string()),
            version,
            description,
            homepage,
            installed: false,
            manager: PackageManagerKind::Apt,
        })
    }
}

// ── PackageManager impl ───────────────────────────────────────────────────────

#[async_trait]
impl PackageManager for AptManager {
    fn kind(&self) -> PackageManagerKind {
        PackageManagerKind::Apt
    }

    fn name(&self) -> &str {
        &self.binary
    }

    fn is_available(&self) -> bool {
        find_binary(&self.binary).is_some()
    }

    #[instrument(skip(self))]
    async fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let (stdout, stderr, ok) = Self::run_cmd("apt-cache", &["search", query]).await?;
        if !ok {
            warn!("apt-cache search returned non-zero: {stderr}");
        }
        Ok(self.parse_search_output(&stdout))
    }

    #[instrument(skip(self))]
    async fn install(&self, package: &str) -> Result<InstallResult> {
        let (stdout, stderr, success) =
            self.run_sudo_apt(&["install", "-y", package]).await?;
        let combined = format!("{stdout}{stderr}");

        if !success {
            return Err(Error::PackageManager(format!(
                "apt-get install failed for `{package}`: {stderr}"
            )));
        }

        // Try to extract the installed version from output lines like:
        // "Unpacking ripgrep (14.1.0-1) ..."
        let version_installed = stdout.lines().find_map(|l| {
            let l = l.trim();
            if l.starts_with("Unpacking ") && l.contains('(') && l.contains(')') {
                let open = l.rfind('(')?;
                let close = l.rfind(')')?;
                Some(l[open + 1..close].to_string())
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
        let (_stdout, stderr, success) =
            self.run_sudo_apt(&["remove", "-y", package]).await?;
        if !success {
            return Err(Error::PackageManager(format!(
                "apt-get remove failed for `{package}`: {stderr}"
            )));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        // dpkg-query is faster and more reliable than `apt list --installed`
        let (stdout, stderr, ok) = Self::run_cmd(
            "dpkg-query",
            &["-W", "-f=${Package}\t${Version}\n"],
        )
        .await?;

        if !ok {
            return Err(Error::PackageManager(format!(
                "dpkg-query failed: {stderr}"
            )));
        }

        let packages = stdout
            .lines()
            .filter_map(|line| {
                // splitn(2, '\t') guarantees at most 2 parts; we validate both exist
                // and are non-empty to guard against malformed dpkg-query output.
                let mut parts = line.splitn(2, '\t');
                let name = parts.next().map(str::trim).filter(|s| !s.is_empty())?.to_string();
                let version = parts.next().map(str::trim).filter(|s| !s.is_empty())?.to_string();
                Some(InstalledPackage {
                    name,
                    version,
                    manager: PackageManagerKind::Apt,
                    install_date: None,
                })
            })
            .collect();

        Ok(packages)
    }

    #[instrument(skip(self))]
    async fn info(&self, package: &str) -> Result<Option<PackageInfo>> {
        let (stdout, _stderr, _ok) =
            Self::run_cmd("apt-cache", &["show", package]).await?;
        Ok(self.parse_show_output(&stdout, package))
    }

    fn install_command(&self, package: &str) -> String {
        format!("sudo apt-get install -y {}", package)
    }

    fn remove_command(&self, package: &str) -> String {
        format!("sudo apt-get remove -y {}", package)
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

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

    fn make_manager() -> AptManager {
        AptManager { binary: "apt".into() }
    }

    #[test]
    fn parse_search_basic() {
        let raw =
            "ripgrep - recursively searches directories for a regex pattern\ncurl - command line tool for transferring data with URL syntax\n";
        let mgr = make_manager();
        let results = mgr.parse_search_output(raw);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "ripgrep");
        assert_eq!(
            results[0].description.as_deref(),
            Some("recursively searches directories for a regex pattern")
        );
    }

    #[test]
    fn parse_show_basic() {
        let raw = "Package: ripgrep\nVersion: 14.1.0-1\nDescription: recursively searches directories for a regex pattern\nHomepage: https://github.com/BurntSushi/ripgrep\n";
        let mgr = make_manager();
        let info = mgr.parse_show_output(raw, "ripgrep").unwrap();
        assert_eq!(info.name, "ripgrep");
        assert_eq!(info.version.as_deref(), Some("14.1.0-1"));
    }

    #[test]
    fn install_command_format() {
        let mgr = make_manager();
        assert_eq!(mgr.install_command("curl"), "sudo apt-get install -y curl");
    }
}
