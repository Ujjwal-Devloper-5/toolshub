//! `toolshub run <command>` — safety-check a command before executing it.
//!
//! Scans the command for dangerous patterns (rm -rf, dd, mkfs, etc.),
//! explains the risk, prompts for confirmation, then runs it with
//! real-time output forwarding.

use colored::Colorize;
use dialoguer::Confirm;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::instrument;

use toolshub_core::config::Config;
use toolshub_core::types::DangerLevel;
use toolshub_safety::{SafetyChecker, SafetyConcern};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub run <command…>`.
#[instrument(skip(config))]
pub async fn run(command: &[String], config: &Config) -> anyhow::Result<()> {
    if command.is_empty() {
        anyhow::bail!("No command specified");
    }

    let full_cmd = command.join(" ");

    println!();
    println!("  {} {}", "◆ Command:".bold().cyan(), full_cmd.bold());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    // Safety check delegated to toolshub-safety engine.
    if config.safety.check_commands {
        let threshold = parse_danger_level(&config.safety.danger_level_threshold);
        let checker = SafetyChecker::with_threshold(threshold);
        let report = checker.analyse(&full_cmd);

        if !report.is_clean() {
            print_safety_warnings(&report.concerns);

            if report.should_block {
                let ok = Confirm::new()
                    .with_prompt("  ⚠  This command may be dangerous. Run anyway?")
                    .default(false)
                    .interact()
                    .unwrap_or(false);

                if !ok {
                    println!("  {}", "Command cancelled.".yellow());
                    return Ok(());
                }
            }
        }
    }

    // Execute the command.
    println!("  {} Running: {}", "▶".green().bold(), full_cmd.green());
    println!("  {}", "─".repeat(60).dimmed());

    let status = execute_command(command).await?;

    println!("  {}", "─".repeat(60).dimmed());
    if status {
        println!("  {} Command completed successfully.", "✓".green().bold());
    } else {
        println!("  {} Command exited with a non-zero status.", "✗".red().bold());
        std::process::exit(1);
    }

    Ok(())
}

fn print_safety_warnings(concerns: &[SafetyConcern]) {
    println!("  {} Safety Analysis", "🔍".yellow());
    println!("  {}", "─".repeat(50).dimmed());
    for c in concerns {
        let badge = c.level.badge();
        println!("  {} {}", badge, c.label.bold());
        println!("    {} {}", "⚠".yellow(), c.consequence.yellow());
        if let Some(alt) = &c.alternative {
            println!("    {} Alternative: {}", "💡".cyan(), alt.cyan());
        }
        println!();
    }
}

fn parse_danger_level(s: &str) -> DangerLevel {
    match s.to_lowercase().as_str() {
        "low" => DangerLevel::Low,
        "medium" => DangerLevel::Medium,
        "high" => DangerLevel::High,
        "critical" => DangerLevel::Critical,
        _ => DangerLevel::High,
    }
}

// ── Execution ─────────────────────────────────────────────────────────────────

async fn execute_command(args: &[String]) -> anyhow::Result<bool> {
    let (prog, rest) = args.split_first().unwrap();

    let mut child = Command::new(prog)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn '{}': {}", prog, e))?;

    // Stream stdout.
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("  {}", line);
        }
    }

    // Stream stderr.
    if let Some(stderr) = child.stderr.take() {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("  {}", line.yellow());
        }
    }

    let status = child.wait().await
        .map_err(|e| anyhow::anyhow!("Failed to wait for command: {e}"))?;

    Ok(status.success())
}
