//! `toolshub config <action>` — view and manage configuration.

use colored::Colorize;
use std::process::Command;
use tracing::instrument;

use toolshub_core::config::Config;
use crate::ConfigAction;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub config <action>`.
#[instrument(skip(config))]
pub async fn run(action: ConfigAction, config: &Config) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => show(config),
        ConfigAction::Edit => edit().await?,
        ConfigAction::Set { key, value } => set_value(&key, &value)?,
        ConfigAction::Reset => reset()?,
        ConfigAction::Path => path(),
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

fn show(config: &Config) {
    println!();
    println!("  {} {}", "◆".cyan().bold(), "ToolsHub Configuration".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    section("[general]");
    kv("theme", &config.general.theme);
    kv("language", &config.general.language);
    kv("telemetry", &config.general.telemetry.to_string());
    println!();

    section("[ai]");
    kv("provider", &config.ai.provider);
    kv("model", &config.ai.model);
    kv(
        "api_key",
        config
            .ai
            .api_key
            .as_deref()
            .map(|k| if k.is_empty() { "not set" } else { "***" })
            .unwrap_or("not set"),
    );
    kv("endpoint", &config.ai.endpoint);
    kv("max_tokens", &config.ai.max_tokens.to_string());
    kv("temperature", &config.ai.temperature.to_string());
    println!();

    section("[memory]");
    kv("enabled", &config.memory.enabled.to_string());
    kv("db_path", &config.memory.db_path.to_string_lossy());
    kv("max_facts", &config.memory.max_facts.to_string());
    kv("doc_cache_ttl_hours", &config.memory.doc_cache_ttl_hours.to_string());
    println!();

    section("[package]");
    kv("preferred_managers", &config.package.preferred_managers.join(", "));
    kv("confirm_before_install", &config.package.confirm_before_install.to_string());
    kv("show_commands_before_run", &config.package.show_commands_before_run.to_string());
    println!();

    section("[safety]");
    kv("check_commands", &config.safety.check_commands.to_string());
    kv("danger_level_threshold", &config.safety.danger_level_threshold);
    println!();

    section("[display]");
    kv("icons", &config.display.icons.to_string());
    kv("color", &config.display.color.to_string());
    kv("syntax_highlight", &config.display.syntax_highlight.to_string());
    kv("pager", &config.display.pager.to_string());
    println!();

    println!(
        "  {} Config file: {}",
        "📄".dimmed(),
        Config::config_path().to_string_lossy().dimmed()
    );
    println!();
}

async fn edit() -> anyhow::Result<()> {
    let path = Config::config_path();

    // Ensure config file exists.
    if !path.exists() {
        Config::default().save()?;
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "nano".to_string());

    println!("  {} Opening config in {}", "→".cyan(), editor.bold());

    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;

    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }

    println!("  {} Configuration saved.", "✓".green().bold());
    Ok(())
}

fn set_value(key: &str, value: &str) -> anyhow::Result<()> {
    // Load current config.
    let mut config = Config::load().unwrap_or_default();

    match key {
        "general.theme" => config.general.theme = value.to_string(),
        "general.language" => config.general.language = value.to_string(),
        "general.telemetry" => config.general.telemetry = parse_bool(value)?,

        "ai.provider" => config.ai.provider = value.to_string(),
        "ai.model" => config.ai.model = value.to_string(),
        "ai.api_key" => config.ai.api_key = if value.is_empty() { None } else { Some(value.to_string()) },
        "ai.endpoint" => config.ai.endpoint = value.to_string(),
        "ai.max_tokens" => {
            config.ai.max_tokens = value.parse::<u32>()
                .map_err(|_| anyhow::anyhow!("ai.max_tokens must be a positive integer"))?;
        }
        "ai.temperature" => {
            config.ai.temperature = value.parse::<f32>()
                .map_err(|_| anyhow::anyhow!("ai.temperature must be a float (0.0–2.0)"))?;
        }

        "memory.enabled" => config.memory.enabled = parse_bool(value)?,
        "memory.max_facts" => {
            config.memory.max_facts = value.parse::<u32>()
                .map_err(|_| anyhow::anyhow!("memory.max_facts must be a positive integer"))?;
        }
        "memory.doc_cache_ttl_hours" => {
            config.memory.doc_cache_ttl_hours = value.parse::<u64>()
                .map_err(|_| anyhow::anyhow!("doc_cache_ttl_hours must be a non-negative integer"))?;
        }

        "package.confirm_before_install" => config.package.confirm_before_install = parse_bool(value)?,
        "package.show_commands_before_run" => config.package.show_commands_before_run = parse_bool(value)?,

        "safety.check_commands" => config.safety.check_commands = parse_bool(value)?,
        "safety.danger_level_threshold" => {
            match value {
                "low" | "medium" | "high" | "critical" => {
                    config.safety.danger_level_threshold = value.to_string();
                }
                _ => anyhow::bail!(
                    "danger_level_threshold must be one of: low, medium, high, critical"
                ),
            }
        }

        "display.icons" => config.display.icons = parse_bool(value)?,
        "display.color" => config.display.color = parse_bool(value)?,
        "display.syntax_highlight" => config.display.syntax_highlight = parse_bool(value)?,
        "display.pager" => config.display.pager = parse_bool(value)?,

        _ => {
            anyhow::bail!(
                "Unknown config key: '{}'\n\
                 Use `toolshub config show` to see all available keys.",
                key
            );
        }
    }

    config.save()?;
    println!("  {} Set {} = {}", "✓".green().bold(), key.cyan(), value.yellow());
    Ok(())
}

fn reset() -> anyhow::Result<()> {
    let path = Config::config_path();
    Config::default().save()?;
    println!(
        "  {} Configuration reset to defaults at {}",
        "✓".green().bold(),
        path.to_string_lossy().dimmed()
    );
    Ok(())
}

fn path() {
    println!("{}", Config::config_path().to_string_lossy());
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_bool(s: &str) -> anyhow::Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => anyhow::bail!("Expected a boolean value (true/false, yes/no, 1/0)"),
    }
}

fn section(title: &str) {
    println!("  {}", title.bold().yellow());
}

fn kv(key: &str, value: &str) {
    println!("    {:<28} {}", format!("{key}:").cyan(), value);
}
