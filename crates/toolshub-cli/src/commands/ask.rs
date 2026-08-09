//! `toolshub ask <question>` — conversational AI assistant.
//!
//! Routes a free-form question to the configured AI provider, enriching
//! the system prompt with memory context and environment information.

use std::sync::Arc;

use colored::Colorize;
use tracing::{debug, instrument};
use uuid::Uuid;

use toolshub_ai::{router::AiRouter, CompletionRequest, Message, Role};
use toolshub_core::config::Config;
use toolshub_memory::{store::MemoryStore, ToolUsage};

use chrono::Utc;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the `ask` command.
#[instrument(skip(memory, ai, config))]
pub async fn run(
    question: &str,
    memory: Option<Arc<MemoryStore>>,
    ai: Option<Arc<AiRouter>>,
    config: &Config,
) -> anyhow::Result<()> {
    if question.trim().is_empty() {
        anyhow::bail!("Please provide a question. Example: toolshub ask 'how do I list open ports?'");
    }

    print_header(question);

    let answer = if let Some(ref router) = ai {
        query_ai(question, &memory, router, config).await?
    } else {
        no_ai_response(question)
    };

    println!();
    render_answer(&answer);
    println!();

    // Record usage in memory.
    if let Some(ref store) = memory {
        let usage = ToolUsage {
            id: Uuid::new_v4().to_string(),
            tool_name: "ask".to_string(),
            command: format!("toolshub ask {question}"),
            directory: cwd(),
            exit_code: 0,
            used_at: Utc::now().to_rfc3339(),
        };
        if let Err(e) = store.record_tool_usage(&usage).await {
            debug!("Failed to record tool usage: {e}");
        }
    }

    Ok(())
}

// ── AI query ──────────────────────────────────────────────────────────────────

async fn query_ai(
    question: &str,
    memory: &Option<Arc<MemoryStore>>,
    ai: &Arc<AiRouter>,
    config: &Config,
) -> anyhow::Result<String> {
    // Build context from memory facts.
    let mut context = String::new();
    if let Some(ref store) = memory {
        for cat in &["os", "shell", "preference", "project"] {
            if let Ok(facts) = store.get_facts_by_category(cat).await {
                for f in &facts {
                    context.push_str(&format!("  [{cat}] {}: {}\n", f.key, f.value));
                }
            }
        }
    }

    let system_prompt = format!(
        "You are ToolsHub, an expert CLI assistant. You help developers understand \
         and use command-line tools, package managers, and system administration \
         concepts. You give accurate, practical answers with concrete examples. \
         When showing commands, wrap them in backtick fences. Be concise but thorough.\
         \n\nUser system context:\n{context}"
    );

    println!("  {}", "Thinking…".dimmed().italic());

    let req = CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: system_prompt },
            Message { role: Role::User, content: question.to_string() },
        ],
        max_tokens: config.ai.max_tokens,
        temperature: config.ai.temperature,
        stream: false,
    };

    let resp = ai.complete(req).await?;
    Ok(resp.content)
}

fn no_ai_response(question: &str) -> String {
    format!(
        "No AI provider is configured. To enable AI assistance, run:\n\n  \
         toolshub config set ai.provider ollama\n  \
         toolshub config set ai.model llama3.2\n\n\
         Or for OpenAI:\n  \
         toolshub config set ai.provider openai\n  \
         toolshub config set ai.api_key YOUR_KEY\n\n\
         Your question was: {question}"
    )
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn print_header(question: &str) {
    let q = if question.len() > 60 {
        format!("{}…", &question[..57])
    } else {
        question.to_string()
    };
    println!();
    println!("  {} {}", "◆".cyan().bold(), "ToolsHub AI".bold().cyan());
    println!("  {} {}", "?".yellow().bold(), q.white().bold());
    println!("  {}", "─".repeat(60).dimmed());
}

fn render_answer(text: &str) {
    // Simple markdown-aware renderer: detect code blocks.
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            if in_code {
                println!("  {}", "┌────────────────────────────────────────────┐".dimmed());
            } else {
                println!("  {}", "└────────────────────────────────────────────┘".dimmed());
            }
            continue;
        }
        if in_code {
            println!("  {} {}", "│".dimmed(), line.green());
        } else if line.starts_with("# ") {
            println!("  {}", line[2..].bold().yellow());
        } else if line.starts_with("## ") {
            println!("  {}", line[3..].bold().cyan());
        } else if line.starts_with("- ") || line.starts_with("* ") {
            println!("  {} {}", "•".cyan(), &line[2..]);
        } else if line.starts_with("**") && line.ends_with("**") {
            println!("  {}", line.trim_matches('*').bold());
        } else {
            println!("  {line}");
        }
    }
}

fn cwd() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string())
}
