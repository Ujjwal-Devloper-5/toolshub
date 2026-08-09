//! `toolshub doctor` — diagnose the ToolsHub installation.

use colored::Colorize;
use std::sync::Arc;
use tracing::instrument;

use toolshub_ai::router::AiRouter;
use toolshub_core::config::Config;
use toolshub_env::EnvironmentSnapshot;
use toolshub_memory::store::MemoryStore;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub doctor`.
#[instrument(skip(config, memory, ai))]
pub async fn run(
    config: &Config,
    memory: Option<Arc<MemoryStore>>,
    ai: Option<Arc<AiRouter>>,
) -> anyhow::Result<()> {
    println!();
    println!("  {} {}", "◆".cyan().bold(), "ToolsHub Doctor".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();
    println!("  Checking your ToolsHub installation…");
    println!();

    let mut pass = 0u32;
    let mut warn = 0u32;
    let mut fail = 0u32;

    // ── Configuration ────────────────────────────────────────────────────
    section("Configuration");

    let config_path = Config::config_path();
    if config_path.exists() {
        check_pass(&mut pass, "Config file", &config_path.to_string_lossy());
    } else {
        check_warn(&mut warn, "Config file", "Not found — defaults will be used");
    }

    let data_dir = Config::data_dir();
    if data_dir.exists() {
        check_pass(&mut pass, "Data directory", &data_dir.to_string_lossy());
    } else {
        check_warn(&mut warn, "Data directory", &format!("Not created yet: {}", data_dir.to_string_lossy()));
    }

    // ── Memory store ─────────────────────────────────────────────────────
    section("Memory");

    if config.memory.enabled {
        match &memory {
            Some(store) => {
                match store.stats().await {
                    Ok(stats) => {
                        check_pass(
                            &mut pass,
                            "SQLite memory store",
                            &format!(
                                "{} facts, {} cached docs, {} usages",
                                stats.facts, stats.doc_cache, stats.tool_usage
                            ),
                        );
                    }
                    Err(e) => {
                        check_fail(&mut fail, "SQLite memory store", &e.to_string());
                    }
                }
            }
            None => {
                check_fail(&mut fail, "SQLite memory store", "Failed to initialise");
            }
        }
    } else {
        check_warn(&mut warn, "Memory", "Disabled in config (memory.enabled = false)");
    }

    // ── AI backend ───────────────────────────────────────────────────────
    section("AI Backend");

    kv("Provider", &config.ai.provider);
    kv("Model", &config.ai.model);
    kv("Endpoint", &config.ai.endpoint);

    match &ai {
        Some(router) => {
            let available = router.is_available().await;
            if available {
                check_pass(
                    &mut pass,
                    "AI connection",
                    &format!("{} / {}", router.provider_name(), router.model()),
                );
            } else {
                check_warn(
                    &mut warn,
                    "AI connection",
                    &format!(
                        "Provider '{}' at '{}' is not responding",
                        config.ai.provider, config.ai.endpoint
                    ),
                );
            }
        }
        None => {
            check_warn(
                &mut warn,
                "AI backend",
                "Not configured or API key missing — AI features will be unavailable",
            );
        }
    }

    // ── Environment ───────────────────────────────────────────────────────
    section("Environment");

    match EnvironmentSnapshot::capture().await {
        Ok(snapshot) => {
            check_pass(&mut pass, "OS", &snapshot.os.to_string());
            check_pass(&mut pass, "Shell", &snapshot.shell.to_string());
            check_pass(
                &mut pass,
                "PATH entries",
                &format!("{} directories", snapshot.path_entries.len()),
            );
            check_pass(
                &mut pass,
                "Installed tools",
                &format!("{} executables found", snapshot.installed_tools.len()),
            );
            if snapshot.package_managers.is_empty() {
                check_warn(
                    &mut warn,
                    "Package managers",
                    "None detected — install pacman / apt / brew",
                );
            } else {
                let names: Vec<String> = snapshot
                    .package_managers
                    .iter()
                    .map(|pm| pm.kind.to_string())
                    .collect();
                check_pass(&mut pass, "Package managers", &names.join(", "));
            }
        }
        Err(e) => {
            check_fail(&mut fail, "Environment detection", &e.to_string());
        }
    }

    // ── Key tool availability ─────────────────────────────────────────────
    section("Key Tools");

    for tool in &["git", "curl", "tar", "gzip", "ssh"] {
        if which::which(tool).is_ok() {
            check_pass(&mut pass, tool, "found in PATH");
        } else {
            check_warn(&mut warn, tool, "not found in PATH");
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────
    println!();
    println!("  {}", "─".repeat(60).dimmed());
    println!(
        "  {} {}   {} {}   {} {}",
        "✓".green().bold(), format!("{pass} passed").green(),
        "⚠".yellow().bold(), format!("{warn} warnings").yellow(),
        "✗".red().bold(), format!("{fail} failed").red(),
    );
    println!();

    if fail > 0 {
        println!(
            "  {} Some checks failed. Run with {} for more detail.",
            "→".red(),
            "--verbose".green()
        );
    } else if warn > 0 {
        println!(
            "  {} ToolsHub is functional but some optional features may be limited.",
            "→".yellow()
        );
    } else {
        println!("  {} ToolsHub is healthy and fully functional!", "✓".green().bold());
    }
    println!();

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("  {}", title.bold().yellow());
    println!("  {}", "─".repeat(title.len()).dimmed());
}

fn kv(key: &str, value: &str) {
    println!("    {:<22} {}", format!("{key}:").cyan(), value.dimmed());
}

fn check_pass(count: &mut u32, label: &str, detail: &str) {
    *count += 1;
    println!(
        "    {} {:<28} {}",
        "✓".green().bold(),
        label.green(),
        detail.dimmed()
    );
}

fn check_warn(count: &mut u32, label: &str, detail: &str) {
    *count += 1;
    println!(
        "    {} {:<28} {}",
        "⚠".yellow().bold(),
        label.yellow(),
        detail.yellow()
    );
}

fn check_fail(count: &mut u32, label: &str, detail: &str) {
    *count += 1;
    println!(
        "    {} {:<28} {}",
        "✗".red().bold(),
        label.red(),
        detail.red()
    );
}
