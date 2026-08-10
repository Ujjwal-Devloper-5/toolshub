//! Command dispatcher — routes CLI arguments to the appropriate handler.

use crate::{Cli, Commands};
use toolshub_core::config::Config;
use std::sync::Arc;
use tracing::debug;
use colored::Colorize;

pub mod ask;
pub mod config;
pub mod context;
pub mod dashboard;
pub mod doctor;
pub mod env_cmd;
pub mod explain;
pub mod install;
pub mod memory_cmd;
pub mod run_cmd;
pub mod search;
pub mod services;
pub mod tools;

/// Top-level dispatcher. Loads config, initialises subsystems, dispatches.
pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    debug!("dispatching command: {:?}", cli.command);

    // Load configuration (creates default if missing)
    let config = match Config::load() {
        Ok(cfg) => {
            // Validate the config and surface friendly warnings.
            if let Err(warnings) = validate_config(&cfg) {
                for w in &warnings {
                    eprintln!("  {} config: {}", "⚠".yellow(), w.yellow());
                }
            }
            cfg
        }
        Err(e) => {
            eprintln!("  {} Could not load config ({}), using defaults.", "⚠".yellow(), e);
            Config::default()
        }
    };

    // Initialise memory store
    let memory = if config.memory.enabled {
        let db_path = config.memory.resolved_db_path();
        match toolshub_memory::store::MemoryStore::new(&db_path).await {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                eprintln!("  {} memory store unavailable ({}), continuing without memory.", "⚠".yellow(), e);
                None
            }
        }
    } else {
        None
    };

    // Initialise AI router (optional)
    let ai = if !cli.no_ai {
        match toolshub_ai::router::AiRouter::from_config(&config.ai).await {
            Ok(router) => Some(Arc::new(router)),
            Err(e) => {
                debug!("AI unavailable: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Dispatch to handler
    match cli.command {
        Commands::Explain { tool, brief, examples_only } => {
            explain::run(&tool, brief, examples_only, memory, ai, &config).await?;
        }
        Commands::Ask { question } => {
            let q = question.join(" ");
            ask::run(&q, memory, ai, &config).await?;
        }
        Commands::Install { packages, manager, yes } => {
            install::run_install(&packages, manager.as_deref(), yes, &config).await?;
        }
        Commands::Remove { packages, manager, yes } => {
            install::run_remove(&packages, manager.as_deref(), yes, &config).await?;
        }
        Commands::Search { query, manager } => {
            search::run(&query, manager.as_deref(), &config).await?;
        }
        Commands::Tools { filter, compact } => {
            tools::run(filter.as_deref(), compact).await?;
        }
        Commands::Services { filter, failed } => {
            services::run(filter.as_deref(), failed).await?;
        }
        Commands::Context { path } => {
            context::run(&path).await?;
        }
        Commands::Env { packages, hardware } => {
            env_cmd::run(packages, hardware).await?;
        }
        Commands::Run { command, dry_run } => {
            run_cmd::run(&command, dry_run, &config).await?;
        }
        Commands::Config { action } => {
            config::run(action, &config).await?;
        }
        Commands::Memory { action } => {
            memory_cmd::run(action, memory).await?;
        }
        Commands::Doctor => {
            doctor::run(&config, memory, ai).await?;
        }
        Commands::Dashboard => {
            dashboard::run().await?;
        }
    }

    Ok(())
}

// ── Config validation ─────────────────────────────────────────────────────────

/// Validate the loaded config and return a list of human-friendly warnings.
///
/// Returns `Ok(())` if config looks correct, or `Err(Vec<String>)` with a
/// list of warnings (non-fatal — the tool continues, but warns the user).
fn validate_config(cfg: &Config) -> Result<(), Vec<String>> {
    let mut warnings = Vec::new();

    // AI provider checks
    let provider = cfg.ai.provider.to_lowercase();
    let supported_providers = ["openai", "openrouter", "anthropic", "ollama"];
    if !supported_providers.contains(&provider.as_str()) {
        warnings.push(format!(
            "ai.provider `{}` is not recognised. Supported: {}",
            cfg.ai.provider,
            supported_providers.join(", ")
        ));
    }

    // API key required for cloud providers
    let needs_key = matches!(provider.as_str(), "openai" | "openrouter" | "anthropic");
    if needs_key {
        let key_missing = cfg.ai.api_key.as_deref().map(|k| k.trim().is_empty()).unwrap_or(true);
        if key_missing {
            warnings.push(format!(
                "ai.api_key is empty but provider `{}` requires one. \
                 Set it with: toolshub config set ai.api_key YOUR_KEY",
                cfg.ai.provider
            ));
        }
    }

    // Temperature range check
    if cfg.ai.temperature < 0.0 || cfg.ai.temperature > 2.0 {
        warnings.push(format!(
            "ai.temperature is {:.2} which is outside the recommended range [0.0, 2.0]",
            cfg.ai.temperature
        ));
    }

    // Max tokens sanity
    if cfg.ai.max_tokens == 0 {
        warnings.push("ai.max_tokens is 0 — responses will be empty. Set it to at least 512.".into());
    } else if cfg.ai.max_tokens > 32_000 {
        warnings.push(format!(
            "ai.max_tokens is {} which is very large and may exceed provider limits",
            cfg.ai.max_tokens
        ));
    }

    if warnings.is_empty() {
        Ok(())
    } else {
        Err(warnings)
    }
}
