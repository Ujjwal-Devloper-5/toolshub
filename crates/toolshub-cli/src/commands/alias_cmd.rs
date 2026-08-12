//! `toolshub alias <description>` — AI-powered smart alias generator.
//!
//! The AI generates a shell alias for the described use case, shows it
//! to the user for confirmation, and optionally saves it to their shell
//! config file (`.bashrc`, `.zshrc`, or `config.fish`).


use std::sync::Arc;

use colored::Colorize;
use dialoguer::Confirm;
use tracing::instrument;

use toolshub_ai::{router::AiRouter, CompletionRequest, Message, Role};
use toolshub_core::config::Config;
use toolshub_memory::store::MemoryStore;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub alias <description>`.
#[instrument(skip(memory, ai, _config))]
pub async fn run(
    description: &str,
    save: bool,
    memory: Option<Arc<MemoryStore>>,
    ai: Option<Arc<AiRouter>>,
    _config: &Config,
) -> anyhow::Result<()> {
    if description.trim().is_empty() {
        anyhow::bail!(
            "Please describe the alias. Example: toolshub alias 'git status but pretty'"
        );
    }

    println!();
    println!(
        "  {} {}",
        "◆ Alias:".bold().green(),
        description.white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    let Some(ref ai) = ai else {
        println!();
        println!("  {} AI is not configured.", "⚠".yellow());
        return Ok(());
    };

    // Detect current shell from environment / memory
    let shell = detect_shell(&memory).await;
    let rc_file = shell_rc_file(&shell);

    let system_prompt = format!(
        "You are a shell alias expert. Given a description of what the user wants to do, \
         generate ONE smart shell alias for {shell}. \
         Respond ONLY in this exact format — nothing else:\n\
         ALIAS_NAME: <short_memorable_name>\n\
         ALIAS_CMD: <full_alias_definition_including_alias_keyword>\n\
         EXPLANATION: <one sentence explaining what it does and why this name>\n\n\
         Rules:\n\
         - The alias name must be short (2-8 chars), memorable, and not conflict with common system commands.\n\
         - The command must be valid {shell} syntax.\n\
         - Include the 'alias name=' prefix in ALIAS_CMD.\n\
         - Do NOT include backticks, markdown, or code fences in your response.\n\
         - If the description implies a function is better than an alias (e.g. needs arguments), \
           use a shell function instead and prefix ALIAS_CMD with 'function'."
    );

    let user_msg = format!("Create an alias for: {description}");

    println!();
    println!("  {} Generating alias…", "◈".green().bold());
    println!();

    let req = CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: system_prompt },
            Message { role: Role::User,   content: user_msg },
        ],
        max_tokens: 512,
        temperature: 0.4,
        stream: false,
    };

    let resp = ai.complete(req).await?;

    // Parse the structured response
    let (alias_name, alias_cmd, explanation) = parse_alias_response(&resp.content);

    if alias_cmd.is_empty() {
        println!("  {} Failed to parse AI response. Raw output:", "⚠".yellow());
        println!();
        for line in resp.content.lines() {
            println!("  {line}");
        }
        return Ok(());
    }

    // Display the generated alias
    println!("  {}", "┌─ Generated Alias ──────────────────────────────────┐".cyan());
    println!("  {} {}", "│".cyan(), alias_cmd.green().bold());
    println!("  {}", "└────────────────────────────────────────────────────┘".cyan());
    println!();
    if !explanation.is_empty() {
        println!("  {} {explanation}", "ℹ".cyan());
    }
    println!();

    if save || ask_to_save(&alias_name, &rc_file)? {
        append_alias_to_rc(&alias_cmd, &rc_file, description)?;
        println!(
            "  {} Alias saved to {}",
            "✓".green().bold(),
            rc_file.cyan().bold()
        );
        println!();
        println!("  Activate it now with:");
        println!("  {}", format!("source {rc_file}").green());
        println!("  or start a new terminal session.");
    } else {
        println!("  {} Not saved. To add manually, append to {}:", "ℹ".cyan(), rc_file.cyan());
        println!("  {}", alias_cmd.dimmed());
    }

    println!();
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_alias_response(raw: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut cmd  = String::new();
    let mut expl = String::new();

    for line in raw.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("ALIAS_NAME:") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("ALIAS_CMD:") {
            cmd = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("EXPLANATION:") {
            expl = v.trim().to_string();
        }
    }
    (name, cmd, expl)
}

async fn detect_shell(memory: &Option<Arc<MemoryStore>>) -> String {
    // Try memory first
    if let Some(ref store) = memory {
        if let Ok(facts) = store.get_facts_by_category("shell").await {
            for f in &facts {
                if f.key == "name" {
                    return f.value.to_lowercase();
                }
            }
        }
    }
    // Fall back to $SHELL env var
    std::env::var("SHELL")
        .unwrap_or_default()
        .split('/')
        .next_back()
        .unwrap_or("bash")
        .to_lowercase()
}

fn shell_rc_file(shell: &str) -> String {
    match shell {
        "zsh"  => shellexpand::tilde("~/.zshrc").to_string(),
        "fish" => shellexpand::tilde("~/.config/fish/config.fish").to_string(),
        _      => shellexpand::tilde("~/.bashrc").to_string(),
    }
}

fn ask_to_save(name: &str, rc_file: &str) -> anyhow::Result<bool> {
    println!();
    let ok = Confirm::new()
        .with_prompt(format!(
            "  Save alias '{}' to {}?",
            name.green(),
            rc_file.cyan()
        ))
        .default(true)
        .interact()
        .unwrap_or(false);
    Ok(ok)
}

fn append_alias_to_rc(alias_cmd: &str, rc_file: &str, description: &str) -> anyhow::Result<()> {
    let existing = std::fs::read_to_string(rc_file).unwrap_or_default();
    let mut content = existing;
    content.push_str(&format!(
        "\n# ToolsHub alias — {description}\n{alias_cmd}\n"
    ));
    std::fs::write(rc_file, &content)?;
    Ok(())
}
