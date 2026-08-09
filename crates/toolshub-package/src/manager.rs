//! Abstract package manager trait and shared data types.
//!
//! Every concrete package manager backend (pacman, apt, brew, …) must
//! implement [`PackageManager`]. The structs in this module carry the
//! structured data those implementations return.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use toolshub_core::types::PackageManagerKind;
use toolshub_core::Result;

// ── Shared data types ──────────────────────────────────────────────────────────

/// Metadata about a package returned from a search or info query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// The canonical package name.
    pub name: String,
    /// Package version, if available.
    pub version: Option<String>,
    /// Short description of the package.
    pub description: Option<String>,
    /// Upstream homepage URL.
    pub homepage: Option<String>,
    /// Whether the package is currently installed.
    pub installed: bool,
    /// Which package manager this record comes from.
    pub manager: PackageManagerKind,
}

/// A package that is confirmed to be installed on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// The package name.
    pub name: String,
    /// Installed version string.
    pub version: String,
    /// Which package manager owns this installation.
    pub manager: PackageManagerKind,
    /// Installation date, if the package manager records it.
    pub install_date: Option<String>,
}

/// The outcome of an install operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    /// Whether the installation succeeded.
    pub success: bool,
    /// The name of the package that was installed.
    pub package: String,
    /// The version that was installed, if determinable.
    pub version_installed: Option<String>,
    /// Combined stdout / stderr output from the package manager.
    pub output: String,
}

// ── Trait ──────────────────────────────────────────────────────────────────────

/// A uniform interface over any package manager backend.
///
/// All methods that invoke external processes are async and run subprocesses
/// via [`tokio::process::Command`].
#[async_trait]
pub trait PackageManager: Send + Sync {
    /// The kind identifier for this manager.
    fn kind(&self) -> PackageManagerKind;

    /// A human-readable name (e.g. `"pacman"`, `"apt"`, `"brew"`).
    fn name(&self) -> &str;

    /// Returns `true` if the manager binary is present on `PATH`.
    fn is_available(&self) -> bool;

    /// Search the package index for packages matching `query`.
    async fn search(&self, query: &str) -> Result<Vec<PackageInfo>>;

    /// Install `package`, capturing all output.
    async fn install(&self, package: &str) -> Result<InstallResult>;

    /// Remove `package` from the system.
    async fn remove(&self, package: &str) -> Result<()>;

    /// List every package currently installed on the system.
    async fn list_installed(&self) -> Result<Vec<InstalledPackage>>;

    /// Fetch detailed metadata for a single `package`.
    ///
    /// Returns `None` if the package is not found in the index.
    async fn info(&self, package: &str) -> Result<Option<PackageInfo>>;

    /// Returns the shell command a human would type to install `package`.
    fn install_command(&self, package: &str) -> String;

    /// Returns the shell command a human would type to remove `package`.
    fn remove_command(&self, package: &str) -> String;
}
