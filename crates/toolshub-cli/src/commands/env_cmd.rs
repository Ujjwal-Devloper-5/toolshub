//! `toolshub env` — display detected system environment.

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::instrument;

use toolshub_env::EnvironmentSnapshot;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub env`.
#[instrument]
pub async fn run(packages_only: bool, hardware_only: bool) -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Detecting environment…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let snapshot = EnvironmentSnapshot::capture().await?;

    spinner.finish_and_clear();

    println!();
    println!("  {} {}", "◆".cyan().bold(), "System Environment".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    if !packages_only {
        // OS
        section("OPERATING SYSTEM");
        kv("OS", &snapshot.os.to_string());
        kv("Shell", &snapshot.shell.to_string());
        kv("Home", &snapshot.home_dir.to_string_lossy());
        kv("PATH entries", &snapshot.path_entries.len().to_string());
        println!();

        if hardware_only || !packages_only {
            // Hardware
            section("HARDWARE");
            kv("CPU", &snapshot.hardware.cpu_brand);
            kv("Cores", &snapshot.hardware.cpu_cores.to_string());
            kv("RAM", &snapshot.hardware.ram_display());
            kv("Arch", &snapshot.hardware.architecture);
            if let Some(ref gpu) = snapshot.hardware.gpu_info {
                kv("GPU", gpu);
            }
            println!();
        }
    }

    if !hardware_only {
        // Package managers
        section("PACKAGE MANAGERS");
        if snapshot.package_managers.is_empty() {
            println!("    {}", "None detected".dimmed());
        } else {
            for pm in &snapshot.package_managers {
                let ver = pm.version.as_deref().unwrap_or("?");
                println!(
                    "    {} {:<14} {} {}",
                    "✓".green(),
                    pm.kind.to_string().green().bold(),
                    ver.yellow(),
                    pm.path.to_string_lossy().dimmed()
                );
            }
        }
        println!();

        // Tool counts
        section("INSTALLED TOOLS");
        kv("Total", &snapshot.installed_tools.len().to_string());
        println!();
    }

    println!(
        "  {} Detected at {}",
        "clock".dimmed(),
        snapshot.detected_at.format("%Y-%m-%d %H:%M:%S UTC").to_string().dimmed()
    );
    println!();

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("  {}", title.bold().yellow());
    println!("  {}", "─".repeat(title.len()).dimmed());
}

fn kv(key: &str, value: &str) {
    println!("    {:<18} {}", format!("{key}:").cyan(), value);
}
