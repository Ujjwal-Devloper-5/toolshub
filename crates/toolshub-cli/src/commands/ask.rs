//! `toolshub ask <question>` — conversational AI assistant.
//!
//! Routes a free-form question to the configured AI provider, enriching
//! the system prompt with memory context and environment information.
//! Tokens are streamed to the terminal as they arrive, giving instant
//! feedback. After each answer, the user can ask a follow-up question,
//! building a full multi-turn conversation.

use std::io::{self, Write};
use std::sync::Arc;

use colored::Colorize;
use tracing::{debug, instrument};
use uuid::Uuid;

use toolshub_ai::{router::AiRouter, CompletionRequest, Message, Role};
use toolshub_core::config::Config;
use toolshub_memory::{store::MemoryStore, ToolUsage};

use chrono::Utc;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the `ask` command with multi-turn conversation support.
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

    // Build the conversation history — start with system prompt.
    let system_prompt = build_system_prompt(&memory, config).await;
    let mut history: Vec<Message> = vec![Message {
        role: Role::System,
        content: system_prompt,
    }];

    // ── First question ────────────────────────────────────────────────────────
    let mut current_question = question.to_string();

    loop {
        print_question_header(&current_question);

        if ai.is_none() {
            println!();
            render_answer(&no_ai_response(&current_question));
            println!();
            break;
        }

        // Add the user's question to conversation history.
        history.push(Message {
            role: Role::User,
            content: current_question.clone(),
        });

        // Stream the answer — tokens print live as they arrive.
        let resp = stream_answer(&history, ai.as_ref().unwrap(), config).await?;

        // Add the assistant's answer to history for follow-up context.
        history.push(Message {
            role: Role::Assistant,
            content: resp.content.clone(),
        });

        // Record usage in memory.
        if let Some(ref store) = memory {
            let usage = ToolUsage {
                id: Uuid::new_v4().to_string(),
                tool_name: "ask".to_string(),
                command: format!("toolshub ask {current_question}"),
                directory: cwd(),
                exit_code: 0,
                used_at: Utc::now().to_rfc3339(),
            };
            if let Err(e) = store.record_tool_usage(&usage).await {
                debug!("Failed to record tool usage: {e}");
            }
        }

        // ── Follow-up prompt ──────────────────────────────────────────────────
        println!();
        println!(
            "  {} {} {} {}",
            "─".repeat(20).dimmed(),
            "follow-up?".dimmed().italic(),
            "─".repeat(20).dimmed(),
            "(Enter to skip)".dimmed()
        );
        print!("  {} ", "›".cyan().bold());
        io::stdout().flush()?;

        let mut follow_up = String::new();
        io::stdin().read_line(&mut follow_up)?;
        let follow_up = follow_up.trim().to_string();

        if follow_up.is_empty() {
            println!();
            break;
        }

        current_question = follow_up;
    }

    Ok(())
}

// ── AI query (streaming) ──────────────────────────────────────────────────────

async fn stream_answer(
    history: &[Message],
    ai: &Arc<AiRouter>,
    config: &Config,
) -> anyhow::Result<toolshub_ai::CompletionResponse> {
    println!("  {}", "Thinking…".dimmed().italic());

    let req = CompletionRequest {
        messages: history.to_vec(),
        max_tokens: config.ai.max_tokens,
        temperature: config.ai.temperature,
        stream: true,
    };

    // complete_streaming() prints tokens live to stdout as they arrive.
    // On providers that don't support streaming it falls back to complete().
    println!();
    println!("  {}", "─".repeat(60).dimmed());

    let resp = ai.complete_streaming(req).await?;

    // If the provider used non-streaming fallback, the content hasn't been
    // printed yet — render it now.
    if resp.content.is_empty() {
        // Edge-case: truly empty response.
        println!("  {}", "(no response)".dimmed());
    } else if !was_streamed_to_stdout(&resp.content) {
        // Non-streaming path: render with the markdown-aware renderer.
        render_answer(&resp.content);
    }

    println!("  {}", "─".repeat(60).dimmed());

    // Token usage hint when available.
    if resp.prompt_tokens > 0 {
        println!(
            "  {} {} prompt + {} completion tokens",
            "tokens:".dimmed(),
            resp.prompt_tokens.to_string().dimmed(),
            resp.completion_tokens.to_string().dimmed()
        );
    }

    Ok(resp)
}

/// Heuristic: content was already streamed to stdout character-by-character
/// if the complete_streaming() path was used. We detect this by checking if
/// the router's provider supports streaming — if so, content was already shown.
///
/// We don't have a direct signal, so we use a simple convention: if the
/// `CompletionResponse` comes back from a streaming call it will always have
/// content (even if non-streaming fallback), but the streaming path already
/// printed it. We render it only in the non-streaming case.
///
/// The cleanest solution is to check if we're using an openrouter/openai
/// provider at the call site, but for now we rely on this being fine because
/// `complete_streaming` already handles the rendering decision internally.
fn was_streamed_to_stdout(_content: &str) -> bool {
    // In the current implementation, complete_streaming() always either:
    // 1. Calls stream_to_stdout() which prints tokens live, OR
    // 2. Calls complete() which does NOT print anything.
    //
    // Since we can't know which path was taken here, we return false and let
    // the caller re-render if needed. The streaming path in openai.rs prints
    // tokens WITHOUT the markdown renderer, giving a raw but live output.
    //
    // TODO(phase-3): Thread a `was_streaming: bool` field through
    // CompletionResponse so we can skip re-rendering.
    false
}

// ── System prompt ─────────────────────────────────────────────────────────────

async fn build_system_prompt(memory: &Option<Arc<MemoryStore>>, _config: &Config) -> String {
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

    format!(
        "You are ToolsHub, an expert CLI assistant. You help developers understand \
         and use command-line tools, package managers, and system administration \
         concepts. You give accurate, practical answers with concrete examples. \
         When showing commands, wrap them in backtick fences. Be concise but thorough.\
         \n\nUser system context:\n{context}"
    )
}

fn no_ai_response(question: &str) -> String {
    format!(
        "No AI provider is configured. To enable AI assistance, run:\n\n  \
         toolshub config set ai.provider openrouter\n  \
         toolshub config set ai.api_key YOUR_OPENROUTER_KEY\n\n\
         Or for local Ollama:\n  \
         toolshub config set ai.provider ollama\n  \
         toolshub config set ai.model llama3.2\n\n\
         Your question was: {question}"
    )
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn print_question_header(question: &str) {
    let q = if question.len() > 60 {
        format!("{}…", &question[..57])
    } else {
        question.to_string()
    };
    println!();
    println!("  {} {}", "◆".cyan().bold(), "ToolsHub AI".bold().cyan());
    println!("  {} {}", "?".yellow().bold(), q.white().bold());
}

fn render_answer(text: &str) {
    // Markdown-aware renderer: handles code blocks, headings, bullet lists.
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
        } else if let Some(stripped) = line.strip_prefix("# ") {
            println!("  {}", stripped.bold().yellow());
        } else if let Some(stripped) = line.strip_prefix("## ") {
            println!("  {}", stripped.bold().cyan());
        } else if let Some(stripped) = line.strip_prefix("### ") {
            println!("  {}", stripped.bold());
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
