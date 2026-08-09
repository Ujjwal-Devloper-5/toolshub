//! ToolsHub CLI — The Intelligent CLI Operating Companion
//!
//! Entry point for the `toolshub` binary. Parses arguments via clap,
//! initialises all subsystems, and dispatches to command handlers.

use clap::{Parser, Subcommand};
use std::process;
use tracing::error;

mod commands;

// ── CLI Definition ────────────────────────────────────────────────────────────

/// ToolsHub — The Intelligent CLI Operating Companion
///
/// Learn, discover, install, and understand command-line tools with
/// AI-powered assistance and deep system awareness.
#[derive(Debug, Parser)]
#[command(
    name        = "toolshub",
    version     = env!("CARGO_PKG_VERSION"),
    author      = "ToolsHub Contributors",
    about       = "The Intelligent CLI Operating Companion",
    long_about  = None,
    arg_required_else_help = true,
    propagate_version = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose/debug output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable all AI features (work offline)
    #[arg(long, global = true)]
    pub no_ai: bool,

    /// Disable coloured terminal output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Output results as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Use a specific configuration profile
    #[arg(long, value_name = "PROFILE", global = true)]
    pub profile: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Explain a command or tool in comprehensive detail
    #[command(alias = "ex")]
    Explain {
        /// The tool or command to explain
        tool: String,
        /// Show only a brief summary
        #[arg(short, long)]
        brief: bool,
    },

    /// Ask the AI assistant a question
    #[command(alias = "q")]
    Ask {
        /// Your question (supports natural language)
        #[arg(trailing_var_arg = true)]
        question: Vec<String>,
    },

    /// Install a package using the best available package manager
    #[command(alias = "i", alias = "add")]
    Install {
        /// Package name(s) to install
        #[arg(required = true)]
        packages: Vec<String>,
        /// Force a specific package manager (e.g. brew, apt, cargo)
        #[arg(short = 'm', long)]
        manager: Option<String>,
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Remove an installed package
    #[command(alias = "rm", alias = "uninstall")]
    Remove {
        /// Package name(s) to remove
        #[arg(required = true)]
        packages: Vec<String>,
        /// Force a specific package manager
        #[arg(short = 'm', long)]
        manager: Option<String>,
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Search packages across all available package managers
    #[command(alias = "find")]
    Search {
        /// Search query
        query: String,
        /// Limit to a specific package manager
        #[arg(short = 'm', long)]
        manager: Option<String>,
    },

    /// List all installed tools discovered in PATH
    Tools {
        /// Filter tools by name (substring match)
        #[arg(short, long)]
        filter: Option<String>,
        /// Show tools as a compact list (no details)
        #[arg(short, long)]
        compact: bool,
    },

    /// Show and manage systemd services (Linux only)
    Services {
        /// Filter services by name
        filter: Option<String>,
        /// Show only failed services
        #[arg(long)]
        failed: bool,
    },

    /// Show context for the current directory
    Context {
        /// Directory to inspect (default: current)
        #[arg(default_value = ".")]
        path: String,
    },

    /// Display the detected system environment
    Env {
        /// Show only package managers
        #[arg(long)]
        packages: bool,
        /// Show only hardware info
        #[arg(long)]
        hardware: bool,
    },

    /// Run a command with safety checking
    Run {
        /// Command to run (will be safety-checked first)
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Manage ToolsHub configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Inspect and manage AI memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Diagnose your ToolsHub installation
    Doctor,

    /// Launch the interactive TUI dashboard
    #[command(alias = "ui", alias = "tui")]
    Dashboard,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Open config file in $EDITOR
    Edit,
    /// Set a configuration value
    Set {
        key: String,
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
    /// Show the config file path
    Path,
}

#[derive(Debug, Subcommand)]
pub enum MemoryAction {
    /// Show all stored facts
    Show,
    /// Clear all memory (irreversible)
    Clear,
    /// Show conversation summaries
    Summaries,
    /// Show cached documentation
    Cache,
    /// Show tool usage history
    History,
}

// ── Entry Point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialise tracing
    let level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    // Disable colour if requested or if not a TTY
    if cli.no_color || !console::Term::stdout().is_term() {
        colored::control::set_override(false);
    }

    // Dispatch command
    let result = commands::dispatch(cli).await;

    if let Err(err) = result {
        error!("{}", err);
        eprintln!("{}: {}", colored::Colorize::red("error"), err);
        process::exit(1);
    }
}
