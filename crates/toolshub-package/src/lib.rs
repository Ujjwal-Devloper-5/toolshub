//! # toolshub-package
//!
//! Unified package manager abstraction for ToolsHub.
//!
//! This crate provides:
//! - The [`PackageManager`] trait, which all backends implement.
//! - Concrete backends: [`pacman`], [`apt`], [`brew`].
//! - Helper functions ([`best_system_manager`], [`available_managers`],
//!   [`get_manager`]) that select the right backend for the current host.

pub mod apt;
pub mod brew;
pub mod manager;
pub mod pacman;

pub use manager::{InstallResult, InstalledPackage, PackageInfo, PackageManager};

use toolshub_core::types::PackageManagerKind;

// ── Public helpers ─────────────────────────────────────────────────────────────

/// Returns the single best system-level package manager for the current host.
///
/// Priority: yay/paru/pacman → apt → brew.
/// Returns `None` if no known system package manager is installed.
pub async fn best_system_manager() -> Option<Box<dyn PackageManager>> {
    // Arch-family
    if let Some(pm) = pacman::PacmanManager::new() {
        return Some(Box::new(pm));
    }
    // Debian/Ubuntu
    if let Some(pm) = apt::AptManager::new() {
        return Some(Box::new(pm));
    }
    // macOS / Linux Homebrew
    if let Some(pm) = brew::BrewManager::new() {
        return Some(Box::new(pm));
    }
    None
}

/// Returns every package manager backend that is currently available on the host.
pub async fn available_managers() -> Vec<Box<dyn PackageManager>> {
    let mut managers: Vec<Box<dyn PackageManager>> = Vec::new();

    if let Some(pm) = pacman::PacmanManager::new() {
        managers.push(Box::new(pm));
    }
    if let Some(pm) = apt::AptManager::new() {
        managers.push(Box::new(pm));
    }
    if let Some(pm) = brew::BrewManager::new() {
        managers.push(Box::new(pm));
    }

    managers
}

/// Return a boxed package manager for the explicit [`PackageManagerKind`].
///
/// Returns `None` when:
/// - the kind is not one of the implemented backends, **or**
/// - the required binary is not present on `PATH`.
pub fn get_manager(kind: PackageManagerKind) -> Option<Box<dyn PackageManager>> {
    match kind {
        PackageManagerKind::Pacman
        | PackageManagerKind::Yay
        | PackageManagerKind::Paru => {
            pacman::PacmanManager::new().map(|pm| Box::new(pm) as Box<dyn PackageManager>)
        }
        PackageManagerKind::Apt => {
            apt::AptManager::new().map(|pm| Box::new(pm) as Box<dyn PackageManager>)
        }
        PackageManagerKind::Brew => {
            brew::BrewManager::new().map(|pm| Box::new(pm) as Box<dyn PackageManager>)
        }
        _ => None,
    }
}
