//! `toolshub history` — rich timeline of commands run through ToolsHub.
//!
//! Reads from the SQLite memory store and renders a colour-coded timeline
//! with danger ratings, working directories, and timestamps.

use std::sync::Arc;

use colored::Colorize;
use tracing::instrument;

use toolshub_memory::store::MemoryStore;
use toolshub_safety::SafetyChecker;
use toolshub_core::config::Config;
use toolshub_core::types::DangerLevel;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub history`.
#[instrument(skip(memory, config))]
pub async fn run(
    memory: Option<Arc<MemoryStore>>,
    limit: u32,
    json: bool,
    config: &Config,
) -> anyhow::Result<()> {
    let store = match memory {
        Some(s) => s,
        None => {
            eprintln!(
                "  {} Memory is disabled. Enable it with: toolshub config set memory.enabled true",
                "⚠".yellow()
            );
            return Ok(());
        }
    };

    let entries = store.list_tool_usage(limit as i64).await?;

    if entries.is_empty() {
        println!();
        println!(
            "  {} No history yet. Use ToolsHub commands and they will appear here.",
            "ℹ".cyan()
        );
        println!();
        return Ok(());
    }

    if json {
        let out: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "tool": e.tool_name,
                    "command": e.command,
                    "directory": e.directory,
                    "exit_code": e.exit_code,
                    "used_at": e.used_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!();
    println!(
        "  {} {} — showing last {} entries",
        "◆".cyan().bold(),
        "ToolsHub History".bold().cyan(),
        entries.len().to_string().bold()
    );
    println!("  {}", "─".repeat(70).dimmed());
    println!();

    let threshold = parse_danger_level(&config.safety.danger_level_threshold);
    let checker = SafetyChecker::with_threshold(threshold);

    for (idx, entry) in entries.iter().enumerate() {
        let report = checker.analyse(&entry.command);
        let danger_badge = if report.is_clean() {
            "  ✓".green().to_string()
        } else if report.should_block {
            "  ⚠".red().to_string()
        } else {
            "  !".yellow().to_string()
        };

        // Format timestamp (strip nanoseconds for readability)
        let ts = entry.used_at
            .get(..16)
            .unwrap_or(&entry.used_at)
            .replace('T', " ");

        // Exit code badge
        let exit_badge = if entry.exit_code == 0 {
            format!("{}", "exit:0".green().dimmed())
        } else {
            format!("{}", format!("exit:{}", entry.exit_code).red())
        };

        println!(
            "{} {}  {}  {}",
            danger_badge,
            format!("{:>3}.", entries.len() - idx).dimmed(),
            entry.command.white().bold(),
            exit_badge
        );
        println!(
            "      {}  {}  {}",
            ts.dimmed(),
            entry.directory.dimmed(),
            format!("[{}]", entry.tool_name).dimmed()
        );

        // Show safety concerns inline if any
        if !report.concerns.is_empty() {
            for concern in &report.concerns {
                println!(
                    "      {} {} — {}",
                    concern.level.badge(),
                    concern.label.yellow(),
                    concern.consequence.dimmed()
                );
            }
        }

        if idx < entries.len() - 1 {
            println!("      {}", "╎".dimmed());
        }
    }

    println!();
    println!(
        "  {} ✓=safe  !=warning  ⚠=dangerous",
        "legend:".dimmed()
    );
    println!();

    Ok(())
}

fn parse_danger_level(s: &str) -> DangerLevel {
    match s.to_lowercase().as_str() {
        "low"      => DangerLevel::Low,
        "medium"   => DangerLevel::Medium,
        "high"     => DangerLevel::High,
        "critical" => DangerLevel::Critical,
        _          => DangerLevel::High,
    }
}
