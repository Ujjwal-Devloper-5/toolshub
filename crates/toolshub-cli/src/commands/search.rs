//! `toolshub search <query>` — search packages across all available managers.

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::instrument;

use toolshub_core::config::Config;
use toolshub_package::{available_managers, get_manager, PackageInfo};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub search <query>`.
#[instrument(skip(_config))]
pub async fn run(
    query: &str,
    manager_hint: Option<&str>,
    _config: &Config,
) -> anyhow::Result<()> {
    println!();
    println!(
        "  {} Searching for {} {}",
        "◆".cyan().bold(),
        format!("\"{}\"", query).yellow().bold(),
        if let Some(m) = manager_hint {
            format!("in {}", m.green().bold())
        } else {
            "across all package managers".dimmed().to_string()
        }
    );
    println!();

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Searching…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let mut all_results: Vec<(String, Vec<PackageInfo>)> = Vec::new();

    if let Some(name) = manager_hint {
        use std::str::FromStr;
        use toolshub_core::types::PackageManagerKind;
        let kind = PackageManagerKind::from_str(name)
            .map_err(|_| anyhow::anyhow!("Unknown package manager: '{name}'"))?;
        if let Some(mgr) = get_manager(kind) {
            let results = mgr.search(query).await.unwrap_or_default();
            all_results.push((mgr.name().to_string(), results));
        } else {
            anyhow::bail!("Package manager '{name}' not found on this system");
        }
    } else {
        let managers = available_managers().await;
        for mgr in &managers {
            spinner.set_message(format!("Searching {} …", mgr.name()));
            let results = mgr.search(query).await.unwrap_or_default();
            if !results.is_empty() {
                all_results.push((mgr.name().to_string(), results));
            }
        }
    }

    spinner.finish_and_clear();

    if all_results.is_empty() {
        println!(
            "  {} No packages found matching {}",
            "○".yellow(),
            format!("\"{}\"", query).yellow()
        );
        println!();
        println!(
            "  {} Try a different query or check available package managers with {}",
            "hint".dimmed(),
            "toolshub env --packages".green()
        );
        return Ok(());
    }

    let total: usize = all_results.iter().map(|(_, r)| r.len()).sum();
    println!(
        "  {} Found {} result(s)\n",
        "✓".green().bold(),
        total.to_string().bold()
    );

    for (manager_name, results) in &all_results {
        println!(
            "  {} {}",
            "▶".cyan().bold(),
            manager_name.bold().cyan()
        );
        println!("  {}", "─".repeat(60).dimmed());

        // Column widths
        let name_w = results.iter().map(|r| r.name.len()).max().unwrap_or(10).clamp(10, 30);
        let ver_w = 12usize;
        let desc_w = 56usize.saturating_sub(name_w + ver_w + 4);

        // Header
        println!(
            "  {:<nw$}  {:<vw$}  {}",
            "NAME".bold(),
            "VERSION".bold(),
            "DESCRIPTION".bold(),
            nw = name_w,
            vw = ver_w,
        );
        println!("  {}", "─".repeat(name_w + ver_w + desc_w + 4).dimmed());

        for pkg in results.iter().take(20) {
            let name = truncate(&pkg.name, name_w);
            let ver = truncate(pkg.version.as_deref().unwrap_or("—"), ver_w);
            let desc = truncate(pkg.description.as_deref().unwrap_or(""), desc_w);

            println!(
                "  {:<nw$}  {:<vw$}  {}",
                name.green(),
                ver.yellow(),
                desc.dimmed(),
                nw = name_w,
                vw = ver_w,
            );
        }

        if results.len() > 20 {
            println!(
                "  {} … and {} more results (use `--manager {}` for a focused search)",
                "".dimmed(),
                results.len() - 20,
                manager_name
            );
        }

        println!();
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
