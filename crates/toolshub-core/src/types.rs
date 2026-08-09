//! Core domain types for ToolsHub.
//!
//! This module defines the fundamental data structures used across the entire
//! ToolsHub ecosystem: operating system detection, shell identification,
//! package manager enumeration, danger level classification, and hardware
//! information.

use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

// ─── Operating System ────────────────────────────────────────────────────────

/// The host operating system as detected at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "os", rename_all = "snake_case")]
pub enum OperatingSystem {
    /// A Linux distribution.
    Linux {
        /// The specific Linux distribution.
        distro: LinuxDistro,
        /// Kernel / release version string.
        version: String,
    },
    /// Apple macOS.
    MacOS {
        /// macOS version string (e.g. `"14.4.1"`).
        version: String,
    },
    /// Microsoft Windows.
    Windows {
        /// Windows version string (e.g. `"11 23H2"`).
        version: String,
    },
    /// Could not determine the operating system.
    Unknown,
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperatingSystem::Linux { distro, version } => write!(f, "Linux ({distro} {version})"),
            OperatingSystem::MacOS { version } => write!(f, "macOS {version}"),
            OperatingSystem::Windows { version } => write!(f, "Windows {version}"),
            OperatingSystem::Unknown => write!(f, "Unknown OS"),
        }
    }
}

// ─── Linux Distro ─────────────────────────────────────────────────────────────

/// A well-known Linux distribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "title_case")]
pub enum LinuxDistro {
    /// Arch Linux.
    Arch,
    /// Manjaro Linux.
    Manjaro,
    /// EndeavourOS.
    EndeavourOS,
    /// Ubuntu.
    Ubuntu,
    /// Debian.
    Debian,
    /// Linux Mint.
    Mint,
    /// Pop!_OS.
    PopOS,
    /// elementary OS.
    Elementary,
    /// Fedora.
    Fedora,
    /// Red Hat Enterprise Linux.
    RedHat,
    /// CentOS / CentOS Stream.
    CentOS,
    /// Rocky Linux.
    Rocky,
    /// AlmaLinux.
    AlmaLinux,
    /// openSUSE.
    OpenSUSE,
    /// Alpine Linux.
    Alpine,
    /// Gentoo Linux.
    Gentoo,
    /// Void Linux.
    Void,
    /// NixOS.
    NixOS,
    /// Raspbian.
    Raspbian,
    /// Any distribution not explicitly enumerated.
    Other(String),
}

// ─── Shell ────────────────────────────────────────────────────────────────────

/// The interactive shell that is currently active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "title_case")]
pub enum Shell {
    /// GNU Bash.
    Bash,
    /// Z Shell.
    Zsh,
    /// Friendly Interactive SHell.
    Fish,
    /// Nushell (canonical variant — aliases in detection: `nu`, `nushell`).
    Nu,
    /// POSIX sh.
    Sh,
    /// Debian Almquist Shell.
    Dash,
    /// KornShell.
    Ksh,
    /// TENEX C Shell.
    Tcsh,
    /// Elvish shell.
    Elvish,
    /// Xonsh (Python-powered shell).
    Xonsh,
    /// Ion shell (written in Rust).
    Ion,
    /// Microsoft PowerShell (cross-platform).
    PowerShell,
    /// Windows Command Prompt.
    Cmd,
    /// Unrecognised or unavailable shell.
    Unknown(String),
}

// ─── Package Manager ──────────────────────────────────────────────────────────

/// Every package manager that ToolsHub is aware of.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum PackageManagerKind {
    // ── System (Arch) ──
    /// The official Arch Linux package manager.
    Pacman,
    /// Yet Another Yogurt – an Arch AUR helper.
    Yay,
    /// Paru – a fast AUR helper.
    Paru,

    // ── System (Debian/Ubuntu) ──
    /// Advanced Packaging Tool.
    Apt,

    // ── System (Fedora/RHEL) ──
    /// Dandified YUM.
    Dnf,
    /// Yellowdog Updater Modified.
    Yum,

    // ── System (Alpine) ──
    /// Alpine Package Keeper.
    Apk,

    // ── System (openSUSE) ──
    /// Zypper package manager.
    Zypper,

    // ── System (macOS) ──
    /// Homebrew.
    Brew,

    // ── System (Windows) ──
    /// Windows Package Manager.
    Winget,
    /// Chocolatey.
    Choco,
    /// Scoop.
    Scoop,

    // ── Language ──
    /// Rust / Cargo.
    Cargo,
    /// Go modules tool.
    Go,
    /// Python pip.
    Pip,
    /// pip with isolated environments.
    Pipx,
    /// Node Package Manager.
    Npm,
    /// Performant npm alternative.
    Pnpm,
    /// Yarn package manager.
    Yarn,
    /// Bun JavaScript runtime & package manager.
    Bun,

    // ── Universal ──
    /// Flatpak.
    Flatpak,
    /// Snap.
    Snap,
}

impl PackageManagerKind {
    /// Returns `true` if this is a system-level package manager.
    pub fn is_system(&self) -> bool {
        matches!(
            self,
            PackageManagerKind::Pacman
                | PackageManagerKind::Yay
                | PackageManagerKind::Paru
                | PackageManagerKind::Apt
                | PackageManagerKind::Dnf
                | PackageManagerKind::Yum
                | PackageManagerKind::Apk
                | PackageManagerKind::Zypper
                | PackageManagerKind::Brew
                | PackageManagerKind::Winget
                | PackageManagerKind::Choco
                | PackageManagerKind::Scoop
        )
    }

    /// Returns `true` if this is a language-specific package manager.
    pub fn is_language(&self) -> bool {
        matches!(
            self,
            PackageManagerKind::Cargo
                | PackageManagerKind::Go
                | PackageManagerKind::Pip
                | PackageManagerKind::Pipx
                | PackageManagerKind::Npm
                | PackageManagerKind::Pnpm
                | PackageManagerKind::Yarn
                | PackageManagerKind::Bun
        )
    }

    /// Returns `true` if this is a universal / sandboxed package manager.
    pub fn is_universal(&self) -> bool {
        matches!(
            self,
            PackageManagerKind::Flatpak | PackageManagerKind::Snap
        )
    }

    /// Returns the canonical binary name used to invoke this manager.
    pub fn binary_name(&self) -> &'static str {
        match self {
            PackageManagerKind::Pacman => "pacman",
            PackageManagerKind::Yay => "yay",
            PackageManagerKind::Paru => "paru",
            PackageManagerKind::Apt => "apt",
            PackageManagerKind::Dnf => "dnf",
            PackageManagerKind::Yum => "yum",
            PackageManagerKind::Apk => "apk",
            PackageManagerKind::Zypper => "zypper",
            PackageManagerKind::Brew => "brew",
            PackageManagerKind::Winget => "winget",
            PackageManagerKind::Choco => "choco",
            PackageManagerKind::Scoop => "scoop",
            PackageManagerKind::Cargo => "cargo",
            PackageManagerKind::Go => "go",
            PackageManagerKind::Pip => "pip",
            PackageManagerKind::Pipx => "pipx",
            PackageManagerKind::Npm => "npm",
            PackageManagerKind::Pnpm => "pnpm",
            PackageManagerKind::Yarn => "yarn",
            PackageManagerKind::Bun => "bun",
            PackageManagerKind::Flatpak => "flatpak",
            PackageManagerKind::Snap => "snap",
        }
    }
}

// ─── Danger Level ─────────────────────────────────────────────────────────────

/// Classification of how dangerous an operation is considered to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DangerLevel {
    /// Harmless – read-only or completely reversible.
    Low,
    /// Moderate impact; user should review before proceeding.
    Medium,
    /// Significant risk; requires explicit confirmation.
    High,
    /// Potentially catastrophic; blocked by default.
    Critical,
}

impl DangerLevel {
    /// Returns a numeric score for comparison purposes.
    fn score(self) -> u8 {
        match self {
            DangerLevel::Low => 0,
            DangerLevel::Medium => 1,
            DangerLevel::High => 2,
            DangerLevel::Critical => 3,
        }
    }

    /// Returns a human-readable emoji badge for the level.
    pub fn badge(self) -> &'static str {
        match self {
            DangerLevel::Low => "✅ Low",
            DangerLevel::Medium => "⚠️  Medium",
            DangerLevel::High => "🔴 High",
            DangerLevel::Critical => "💀 Critical",
        }
    }
}

impl fmt::Display for DangerLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.badge())
    }
}

impl PartialOrd for DangerLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DangerLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score().cmp(&other.score())
    }
}

// ─── InstalledTool ────────────────────────────────────────────────────────────

/// A tool that has been found on the host system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledTool {
    /// The canonical name of the tool (e.g. `"ripgrep"`).
    pub name: String,
    /// Version string as reported by the tool, if available.
    pub version: Option<String>,
    /// Absolute path to the tool binary.
    pub path: PathBuf,
    /// The package manager that owns this installation, if known.
    pub package_manager: Option<PackageManagerKind>,
    /// A short human-readable description of the tool.
    pub description: Option<String>,
}

impl InstalledTool {
    /// Creates a minimal [`InstalledTool`] with only a name and path.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            version: None,
            path: path.into(),
            package_manager: None,
            description: None,
        }
    }

    /// Builder-style method to attach a version string.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder-style method to attach the owning package manager.
    pub fn with_package_manager(mut self, pm: PackageManagerKind) -> Self {
        self.package_manager = Some(pm);
        self
    }

    /// Builder-style method to attach a description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

impl fmt::Display for InstalledTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(ref v) = self.version {
            write!(f, " v{v}")?;
        }
        write!(f, " ({})", self.path.display())
    }
}

// ─── DetectedPackageManager ───────────────────────────────────────────────────

/// A package manager that has been detected on the host system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPackageManager {
    /// The kind of package manager.
    pub kind: PackageManagerKind,
    /// Path to the package manager binary.
    pub path: PathBuf,
    /// Version string, if it could be obtained.
    pub version: Option<String>,
}

impl DetectedPackageManager {
    /// Creates a new [`DetectedPackageManager`] with a kind and path.
    pub fn new(kind: PackageManagerKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
            version: None,
        }
    }

    /// Builder-style method to attach a version string.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

impl fmt::Display for DetectedPackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(ref v) = self.version {
            write!(f, " v{v}")?;
        }
        write!(f, " ({})", self.path.display())
    }
}

// ─── HardwareInfo ─────────────────────────────────────────────────────────────

/// Basic hardware information about the host machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// The CPU brand / model string (e.g. `"AMD Ryzen 9 5950X"`).
    pub cpu_brand: String,
    /// Number of logical CPU cores.
    pub cpu_cores: u32,
    /// Total installed RAM in megabytes.
    pub total_ram_mb: u64,
    /// GPU information string, if available (e.g. `"NVIDIA GeForce RTX 4090"`).
    pub gpu_info: Option<String>,
    /// CPU architecture (e.g. `"x86_64"`, `"aarch64"`).
    pub architecture: String,
}

impl HardwareInfo {
    /// Returns total RAM formatted in a human-friendly way (MB or GB).
    pub fn ram_display(&self) -> String {
        if self.total_ram_mb >= 1024 {
            format!("{:.1} GB", self.total_ram_mb as f64 / 1024.0)
        } else {
            format!("{} MB", self.total_ram_mb)
        }
    }
}

impl fmt::Display for HardwareInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CPU: {} ({} cores) | RAM: {} | Arch: {}",
            self.cpu_brand,
            self.cpu_cores,
            self.ram_display(),
            self.architecture,
        )?;
        if let Some(ref gpu) = self.gpu_info {
            write!(f, " | GPU: {gpu}")?;
        }
        Ok(())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_level_ordering() {
        assert!(DangerLevel::Low < DangerLevel::Medium);
        assert!(DangerLevel::Medium < DangerLevel::High);
        assert!(DangerLevel::High < DangerLevel::Critical);
        assert!(DangerLevel::Critical == DangerLevel::Critical);
    }

    #[test]
    fn package_manager_classification() {
        assert!(PackageManagerKind::Pacman.is_system());
        assert!(PackageManagerKind::Cargo.is_language());
        assert!(PackageManagerKind::Flatpak.is_universal());
        assert!(!PackageManagerKind::Npm.is_system());
    }

    #[test]
    fn installed_tool_builder() {
        let tool = InstalledTool::new("ripgrep", "/usr/bin/rg")
            .with_version("14.1.0")
            .with_package_manager(PackageManagerKind::Pacman)
            .with_description("A line-oriented search tool");

        assert_eq!(tool.name, "ripgrep");
        assert_eq!(tool.version.as_deref(), Some("14.1.0"));
        assert_eq!(tool.package_manager, Some(PackageManagerKind::Pacman));
    }

    #[test]
    fn hardware_info_ram_display() {
        let hw = HardwareInfo {
            cpu_brand: "Test CPU".into(),
            cpu_cores: 8,
            total_ram_mb: 16384,
            gpu_info: None,
            architecture: "x86_64".into(),
        };
        assert_eq!(hw.ram_display(), "16.0 GB");

        let hw_small = HardwareInfo {
            cpu_brand: "Test CPU".into(),
            cpu_cores: 2,
            total_ram_mb: 512,
            gpu_info: None,
            architecture: "x86_64".into(),
        };
        assert_eq!(hw_small.ram_display(), "512 MB");
    }

    #[test]
    fn os_display() {
        let os = OperatingSystem::Linux {
            distro: LinuxDistro::Arch,
            version: "6.9.3".into(),
        };
        assert_eq!(os.to_string(), "Linux (Arch 6.9.3)");
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let pm = PackageManagerKind::Cargo;
        let json = serde_json::to_string(&pm).unwrap();
        let pm2: PackageManagerKind = serde_json::from_str(&json).unwrap();
        assert_eq!(pm, pm2);
    }
}
