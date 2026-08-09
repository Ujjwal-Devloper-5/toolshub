//! `toolshub install / remove` — unified package management.
//!
//! Detects the best available package manager, explains what it will do,
//! prompts for confirmation (unless `--yes`), then executes the operation.

use colored::Colorize;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, instrument};

use toolshub_core::config::Config;
use toolshub_package::{best_system_manager, get_manager};

// ── Public entry points ───────────────────────────────────────────────────────

/// Run `toolshub install <packages…>`.
#[instrument(skip(config))]
pub async fn run_install(
    packages: &[String],
    manager_hint: Option<&str>,
    yes: bool,
    config: &Config,
) -> anyhow::Result<()> {
    let mgr = resolve_manager(manager_hint, config).await?;

    println!();
    println!(
        "  {} {} package(s) via {}",
        "→".cyan().bold(),
        packages.len(),
        mgr.name().green().bold()
    );

    for pkg in packages {
        let cmd_preview = mgr.install_command(pkg);
        println!("    {} {}", "$".dimmed(), cmd_preview.dimmed());
    }
    println!();

    if !yes && config.package.confirm_before_install {
        let ok = Confirm::new()
            .with_prompt("  Proceed with installation?")
            .default(true)
            .interact()
            .unwrap_or(false);
        if !ok {
            println!("  {}", "Cancelled.".yellow());
            return Ok(());
        }
    }

    for pkg in packages {
        install_one(pkg, &*mgr, config).await?;
    }

    Ok(())
}

/// Run `toolshub remove <packages…>`.
#[instrument(skip(config))]
pub async fn run_remove(
    packages: &[String],
    manager_hint: Option<&str>,
    yes: bool,
    config: &Config,
) -> anyhow::Result<()> {
    let mgr = resolve_manager(manager_hint, config).await?;

    println!();
    println!(
        "  {} {} package(s) via {}",
        "→".red().bold(),
        packages.len(),
        mgr.name().green().bold()
    );

    for pkg in packages {
        let cmd_preview = mgr.remove_command(pkg);
        println!("    {} {}", "$".dimmed(), cmd_preview.dimmed());
    }
    println!();

    if !yes && config.package.confirm_before_install {
        let ok = Confirm::new()
            .with_prompt("  Proceed with removal?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if !ok {
            println!("  {}", "Cancelled.".yellow());
            return Ok(());
        }
    }

    for pkg in packages {
        remove_one(pkg, &*mgr).await?;
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn resolve_manager(
    hint: Option<&str>,
    config: &Config,
) -> anyhow::Result<Box<dyn toolshub_package::PackageManager>> {
    if let Some(name) = hint {
        // Parse the hint into a PackageManagerKind.
        use std::str::FromStr;
        use toolshub_core::types::PackageManagerKind;
        let kind = PackageManagerKind::from_str(name)
            .map_err(|_| anyhow::anyhow!("Unknown package manager: '{name}'"))?;
        return get_manager(kind)
            .ok_or_else(|| anyhow::anyhow!("Package manager '{name}' not found on this system"));
    }

    // Try preferred managers from config first.
    for preferred in &config.package.preferred_managers {
        use std::str::FromStr;
        use toolshub_core::types::PackageManagerKind;
        if let Ok(kind) = PackageManagerKind::from_str(preferred) {
            if let Some(mgr) = get_manager(kind) {
                return Ok(mgr);
            }
        }
    }

    // Fall back to auto-detection.
    best_system_manager()
        .await
        .ok_or_else(|| anyhow::anyhow!(
            "No supported package manager detected on this system.\n\
             Supported: pacman, apt, brew. Use `--manager` to specify one manually."
        ))
}

async fn install_one(
    pkg: &str,
    mgr: &dyn toolshub_package::PackageManager,
    _config: &Config,
) -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(format!("Installing {pkg}…"));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = mgr.install(pkg).await;

    spinner.finish_and_clear();

    match result {
        Ok(res) => {
            if res.success {
                println!(
                    "  {} {} installed successfully",
                    "✓".green().bold(),
                    pkg.bold()
                );
                if !res.output.trim().is_empty() {
                    debug!("install output: {}", res.output);
                }
            } else {
                eprintln!(
                    "  {} Failed to install {}",
                    "✗".red().bold(),
                    pkg.bold()
                );
                if !res.output.trim().is_empty() {
                    eprintln!("{}", res.output.dimmed());
                }
                anyhow::bail!("Installation of '{}' failed", pkg);
            }
        }
        Err(e) => {
            anyhow::bail!("Error installing '{}': {}", pkg, e);
        }
    }
    Ok(())
}

async fn remove_one(
    pkg: &str,
    mgr: &dyn toolshub_package::PackageManager,
) -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.red} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message(format!("Removing {pkg}…"));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = mgr.remove(pkg).await;

    spinner.finish_and_clear();

    match result {
        Ok(()) => {
            println!(
                "  {} {} removed successfully",
                "✓".green().bold(),
                pkg.bold()
            );
        }
        Err(e) => {
            anyhow::bail!("Error removing '{}': {}", pkg, e);
        }
    }
    Ok(())
}
