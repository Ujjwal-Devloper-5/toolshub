//! Command dispatcher — routes CLI arguments to the appropriate handler.

use crate::{Cli, Commands};
use toolshub_core::config::Config;
use std::sync::Arc;
use tracing::debug;

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
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: could not load config ({}), using defaults.", e);
        Config::default()
    });

    // Initialise memory store
    let memory = if config.memory.enabled {
        let db_path = config.memory.resolved_db_path();
        match toolshub_memory::store::MemoryStore::new(&db_path).await {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                eprintln!("Warning: memory store unavailable ({}), continuing without memory.", e);
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
        Commands::Explain { tool, brief } => {
            explain::run(&tool, brief, memory, ai, &config).await?;
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
        Commands::Run { command } => {
            run_cmd::run(&command, &config).await?;
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
