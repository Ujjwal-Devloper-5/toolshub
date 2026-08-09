//! Operating system detection module.
//!
//! Reads OS metadata from platform-specific sources and returns a typed
//! [`OperatingSystem`] value.

use toolshub_core::types::{LinuxDistro, OperatingSystem};
use toolshub_core::Result;
#[allow(unused_imports)]
use tracing::{debug, warn};

/// Detect the operating system and return structured metadata.
pub async fn detect_os() -> Result<OperatingSystem> {
    #[cfg(target_os = "linux")]
    {
        detect_linux().await
    }

    #[cfg(target_os = "macos")]
    {
        detect_macos().await
    }

    #[cfg(target_os = "windows")]
    {
        detect_windows().await
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(OperatingSystem::Unknown)
    }
}

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn detect_linux() -> Result<OperatingSystem> {
    use tokio::fs;

    let contents = fs::read_to_string("/etc/os-release").await.unwrap_or_default();
    let id = parse_os_release_field(&contents, "ID")
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase();
    let version = parse_os_release_field(&contents, "VERSION_ID").unwrap_or_default();
    let pretty = parse_os_release_field(&contents, "PRETTY_NAME");

    debug!(id = %id, version = %version, pretty = ?pretty, "Detected Linux distribution");

    let distro = map_linux_distro(&id);

    Ok(OperatingSystem::Linux { distro, version })
}

/// Parse a key=value field from `/etc/os-release` content.
fn parse_os_release_field(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(rest) = rest.strip_prefix('=') {
                // Strip surrounding quotes if present.
                let value = rest.trim_matches('"').trim_matches('\'').to_string();
                return Some(value);
            }
        }
    }
    None
}

/// Map an `/etc/os-release` ID string to a [`LinuxDistro`].
fn map_linux_distro(id: &str) -> LinuxDistro {
    match id {
        "arch" | "archlinux" => LinuxDistro::Arch,
        "manjaro" => LinuxDistro::Manjaro,
        "endeavouros" => LinuxDistro::EndeavourOS,
        "ubuntu" => LinuxDistro::Ubuntu,
        "debian" => LinuxDistro::Debian,
        "linuxmint" => LinuxDistro::Mint,
        "pop" => LinuxDistro::PopOS,
        "elementary" => LinuxDistro::Elementary,
        "fedora" => LinuxDistro::Fedora,
        "rhel" | "redhat" => LinuxDistro::RedHat,
        "centos" | "centos-stream" => LinuxDistro::CentOS,
        "rocky" => LinuxDistro::Rocky,
        "almalinux" => LinuxDistro::AlmaLinux,
        "opensuse-leap" | "opensuse-tumbleweed" | "sles" => LinuxDistro::OpenSUSE,
        "alpine" => LinuxDistro::Alpine,
        "gentoo" => LinuxDistro::Gentoo,
        "void" => LinuxDistro::Void,
        "nixos" => LinuxDistro::NixOS,
        "raspbian" => LinuxDistro::Raspbian,
        _ => LinuxDistro::Other(id.to_string()),
    }
}

// ── macOS ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn detect_macos() -> Result<OperatingSystem> {
    let output = tokio::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .await;

    let version = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        Ok(out) => {
            warn!(
                stderr = %String::from_utf8_lossy(&out.stderr),
                "sw_vers returned non-zero exit code"
            );
            String::from("unknown")
        }
        Err(e) => {
            warn!(error = %e, "Failed to execute sw_vers");
            String::from("unknown")
        }
    };

    debug!(version = %version, "Detected macOS");
    Ok(OperatingSystem::MacOS { version })
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
async fn detect_windows() -> Result<OperatingSystem> {
    // Attempt to read the Windows version from the registry via the
    // `ver` command (available in all modern Windows versions).
    let output = tokio::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .await;

    let version = match output {
        Ok(out) if out.status.success() => {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // `ver` returns something like "Microsoft Windows [Version 10.0.22621.3447]"
            extract_windows_version(&raw).unwrap_or(raw)
        }
        _ => {
            // Fall back to WINDIR or OS env vars
            std::env::var("OS").unwrap_or_else(|_| "Windows".to_string())
        }
    };

    debug!(version = %version, "Detected Windows");
    Ok(OperatingSystem::Windows { version })
}

#[cfg(target_os = "windows")]
fn extract_windows_version(ver_output: &str) -> Option<String> {
    // Pattern: "Version X.Y.ZZZZZ"
    let start = ver_output.find("Version ")?;
    let rest = &ver_output[start + 8..];
    let end = rest.find(']').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}
