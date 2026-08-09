//! `toolshub explain <tool>` — deep-dive explanation of any CLI tool.
//!
//! Uses the AI router (when available) to generate a comprehensive explanation
//! covering purpose, syntax, flags, examples, pitfalls, and alternatives.
//! Falls back to local man-page / `--help` scraping when AI is unavailable.

use std::sync::Arc;

use chrono::Utc;
use colored::Colorize;
use tracing::{debug, instrument};
use uuid::Uuid;

use toolshub_ai::{CompletionRequest, Message, Role};
use toolshub_ai::router::AiRouter;
use toolshub_core::config::Config;
use toolshub_memory::{store::MemoryStore, ToolUsage};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the `explain` command.
///
/// Fetches a structured explanation for `tool` and renders it to stdout.
/// With `--brief` only the purpose and top-3 examples are shown.
#[instrument(skip(memory, ai, config))]
pub async fn run(
    tool: &str,
    brief: bool,
    memory: Option<Arc<MemoryStore>>,
    ai: Option<Arc<AiRouter>>,
    config: &Config,
) -> anyhow::Result<()> {
    print_header(&format!("toolshub explain  {tool}"));

    let explanation = fetch_explanation(tool, &memory, &ai, config).await?;

    if brief {
        render_brief(&explanation, tool);
    } else {
        render_full(&explanation, tool, config);
    }

    // Record the tool usage in memory.
    if let Some(ref store) = memory {
        let usage = ToolUsage {
            id: Uuid::new_v4().to_string(),
            tool_name: "explain".to_string(),
            command: format!("toolshub explain {tool}"),
            directory: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| ".".to_string()),
            exit_code: 0,
            used_at: Utc::now().to_rfc3339(),
        };
        if let Err(e) = store.record_tool_usage(&usage).await {
            debug!("Failed to record tool usage: {e}");
        }
    }

    Ok(())
}

// ── Explanation data model ────────────────────────────────────────────────────

/// A single annotated usage example.
#[derive(Debug)]
struct Example {
    description: String,
    command: String,
}

/// A flag / option entry for the flags table.
#[derive(Debug)]
struct Flag {
    flag: String,
    description: String,
    example: String,
}

/// An alternative tool entry.
#[derive(Debug)]
struct Alternative {
    name: String,
    reason: String,
}

/// Fully structured explanation for a CLI tool.
#[derive(Debug)]
struct ToolExplanation {
    #[allow(dead_code)]
    tool: String,
    purpose: String,
    history: String,
    syntax: String,
    flags: Vec<Flag>,
    examples: Vec<Example>,
    common_mistakes: Vec<String>,
    performance_tips: Vec<String>,
    security_notes: Vec<String>,
    alternatives: Vec<Alternative>,
    source: ExplainSource,
}

/// Where the explanation originated.
#[derive(Debug)]
enum ExplainSource {
    Ai { provider: String, model: String },
    LocalHelpPage,
    #[allow(dead_code)]
    Fallback,
}

// ── Fetching logic ────────────────────────────────────────────────────────────

async fn fetch_explanation(
    tool: &str,
    memory: &Option<Arc<MemoryStore>>,
    ai: &Option<Arc<AiRouter>>,
    config: &Config,
) -> anyhow::Result<ToolExplanation> {
    if let Some(ref router) = ai {
        return fetch_via_ai(tool, memory, router, config).await;
    }
    fetch_local_fallback(tool).await
}

async fn fetch_via_ai(
    tool: &str,
    memory: &Option<Arc<MemoryStore>>,
    ai: &Arc<AiRouter>,
    config: &Config,
) -> anyhow::Result<ToolExplanation> {
    // Build a context-aware system prompt enriched with known user preferences.
    let mut context_hints = String::new();
    if let Some(ref store) = memory {
        if let Ok(os_facts) = store.get_facts_by_category("os").await {
            for f in &os_facts {
                context_hints.push_str(&format!("  - {}: {}\n", f.key, f.value));
            }
        }
        if let Ok(shell_facts) = store.get_facts_by_category("shell").await {
            for f in &shell_facts {
                context_hints.push_str(&format!("  - {}: {}\n", f.key, f.value));
            }
        }
    }

    let system_prompt = format!(
        "You are ToolsHub, an expert CLI documentation engine. \
         Explain CLI tools in exhaustive, accurate detail. \
         When user context is available, tailor examples to their environment.\n\
         User environment context:\n{context_hints}\n\
         Respond in the following EXACT structured format — no deviations:\n\
         PURPOSE: <one paragraph>\n\
         HISTORY: <one paragraph on origin/background>\n\
         SYNTAX: <usage line(s)>\n\
         FLAGS:\n<flag>|<description>|<example>\n...\n\
         EXAMPLES:\n<description>|<command>\n...\n\
         MISTAKES:\n- <mistake>\n...\n\
         PERFORMANCE:\n- <tip>\n...\n\
         SECURITY:\n- <note>\n...\n\
         ALTERNATIVES:\n<name>|<reason why you might prefer it>\n..."
    );

    let user_msg = format!(
        "Explain the `{tool}` command in exhaustive detail. \
         Provide at least 5 flags, 5 examples, 3 common mistakes, 3 performance tips, \
         2 security notes, and 3 alternatives."
    );

    println!("{}", "  Thinking…".dimmed().italic());

    let req = CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: system_prompt },
            Message { role: Role::User, content: user_msg },
        ],
        max_tokens: config.ai.max_tokens,
        temperature: 0.3,
        stream: false,
    };

    let resp = ai.complete(req).await?;
    let explanation = parse_ai_response(tool, &resp.content, &resp.model);

    // Cache the explanation in memory.
    if let Some(ref store) = memory {
        use toolshub_memory::DocCacheEntry;
        use chrono::Duration;
        let now = Utc::now();
        let expires = now + Duration::hours(config.memory.doc_cache_ttl_hours as i64);
        let entry = DocCacheEntry {
            id: Uuid::new_v4().to_string(),
            tool_name: tool.to_string(),
            content: resp.content.clone(),
            source: format!("ai:{}", resp.model),
            cached_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        };
        if let Err(e) = store.cache_doc(&entry).await {
            debug!("Failed to cache doc: {e}");
        }
    }

    Ok(explanation)
}

/// Parse the structured AI response into a [`ToolExplanation`].
fn parse_ai_response(tool: &str, raw: &str, model: &str) -> ToolExplanation {
    let mut purpose = String::new();
    let mut history = String::new();
    let mut syntax = String::new();
    let mut flags: Vec<Flag> = Vec::new();
    let mut examples: Vec<Example> = Vec::new();
    let mut mistakes: Vec<String> = Vec::new();
    let mut perf: Vec<String> = Vec::new();
    let mut security: Vec<String> = Vec::new();
    let mut alternatives: Vec<Alternative> = Vec::new();

    #[derive(PartialEq, Clone, Copy)]
    enum Section {
        None,
        Purpose,
        History,
        Syntax,
        Flags,
        Examples,
        Mistakes,
        Performance,
        Security,
        Alternatives,
    }

    let mut current = Section::None;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("PURPOSE:") {
            current = Section::Purpose;
            let rest = trimmed.trim_start_matches("PURPOSE:").trim();
            if !rest.is_empty() { purpose = rest.to_string(); }
            continue;
        }
        if trimmed.starts_with("HISTORY:") {
            current = Section::History;
            let rest = trimmed.trim_start_matches("HISTORY:").trim();
            if !rest.is_empty() { history = rest.to_string(); }
            continue;
        }
        if trimmed.starts_with("SYNTAX:") {
            current = Section::Syntax;
            let rest = trimmed.trim_start_matches("SYNTAX:").trim();
            if !rest.is_empty() { syntax = rest.to_string(); }
            continue;
        }
        if trimmed == "FLAGS:" { current = Section::Flags; continue; }
        if trimmed == "EXAMPLES:" { current = Section::Examples; continue; }
        if trimmed == "MISTAKES:" { current = Section::Mistakes; continue; }
        if trimmed == "PERFORMANCE:" { current = Section::Performance; continue; }
        if trimmed == "SECURITY:" { current = Section::Security; continue; }
        if trimmed == "ALTERNATIVES:" { current = Section::Alternatives; continue; }

        if trimmed.is_empty() { continue; }

        match current {
            Section::Purpose => {
                if !purpose.is_empty() { purpose.push(' '); }
                purpose.push_str(trimmed);
            }
            Section::History => {
                if !history.is_empty() { history.push(' '); }
                history.push_str(trimmed);
            }
            Section::Syntax => {
                if !syntax.is_empty() { syntax.push('\n'); }
                syntax.push_str(trimmed);
            }
            Section::Flags => {
                let parts: Vec<&str> = trimmed.splitn(3, '|').collect();
                if parts.len() == 3 {
                    flags.push(Flag {
                        flag: parts[0].trim().to_string(),
                        description: parts[1].trim().to_string(),
                        example: parts[2].trim().to_string(),
                    });
                }
            }
            Section::Examples => {
                let parts: Vec<&str> = trimmed.splitn(2, '|').collect();
                if parts.len() == 2 {
                    examples.push(Example {
                        description: parts[0].trim().to_string(),
                        command: parts[1].trim().to_string(),
                    });
                }
            }
            Section::Mistakes => {
                mistakes.push(trimmed.trim_start_matches("- ").to_string());
            }
            Section::Performance => {
                perf.push(trimmed.trim_start_matches("- ").to_string());
            }
            Section::Security => {
                security.push(trimmed.trim_start_matches("- ").to_string());
            }
            Section::Alternatives => {
                let parts: Vec<&str> = trimmed.splitn(2, '|').collect();
                if parts.len() == 2 {
                    alternatives.push(Alternative {
                        name: parts[0].trim().to_string(),
                        reason: parts[1].trim().to_string(),
                    });
                }
            }
            Section::None => {}
        }
    }

    // Graceful fallback if parsing failed.
    if purpose.is_empty() {
        purpose = raw.lines().next().unwrap_or("No description available.").to_string();
    }

    ToolExplanation {
        tool: tool.to_string(),
        purpose,
        history,
        syntax,
        flags,
        examples,
        common_mistakes: mistakes,
        performance_tips: perf,
        security_notes: security,
        alternatives,
        source: ExplainSource::Ai {
            provider: "ai".to_string(),
            model: model.to_string(),
        },
    }
}

/// Fallback: scrape `--help` and `man` output when AI is unavailable.
async fn fetch_local_fallback(tool: &str) -> anyhow::Result<ToolExplanation> {
    println!("{}", "  No AI provider configured. Falling back to --help output.".yellow());

    let help_output = tokio::process::Command::new(tool)
        .arg("--help")
        .output()
        .await;

    let syntax = match help_output {
        Ok(out) => {
            let text = if out.stdout.is_empty() {
                String::from_utf8_lossy(&out.stderr).into_owned()
            } else {
                String::from_utf8_lossy(&out.stdout).into_owned()
            };
            text.lines().take(30).collect::<Vec<_>>().join("\n")
        }
        Err(_) => format!("Could not execute `{tool} --help`"),
    };

    Ok(ToolExplanation {
        tool: tool.to_string(),
        purpose: format!("CLI tool: {tool}"),
        history: String::new(),
        syntax,
        flags: Vec::new(),
        examples: Vec::new(),
        common_mistakes: Vec::new(),
        performance_tips: Vec::new(),
        security_notes: Vec::new(),
        alternatives: Vec::new(),
        source: ExplainSource::LocalHelpPage,
    })
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_brief(e: &ToolExplanation, tool: &str) {
    println!();
    print_section("PURPOSE");
    println!("{}", textwrap_paragraph(&e.purpose, 80));

    if !e.examples.is_empty() {
        println!();
        print_section("EXAMPLES");
        for (i, ex) in e.examples.iter().take(3).enumerate() {
            println!("  {}. {}", (i + 1).to_string().cyan().bold(), ex.description.white());
            println!("     {}", ex.command.green());
        }
    }

    println!();
    print_source_indicator(&e.source, tool);
}

fn render_full(e: &ToolExplanation, tool: &str, _config: &Config) {
    // PURPOSE (boxed)
    println!();
    print_boxed_section("PURPOSE", &e.purpose);

    // HISTORY & BACKGROUND
    if !e.history.is_empty() {
        println!();
        print_section("HISTORY & BACKGROUND");
        println!("{}", textwrap_paragraph(&e.history, 80));
    }

    // SYNTAX
    if !e.syntax.is_empty() {
        println!();
        print_section("SYNTAX");
        print_code_block(&e.syntax);
    }

    // COMMON FLAGS
    if !e.flags.is_empty() {
        println!();
        print_section("COMMON FLAGS");
        let col_widths = compute_col_widths(&e.flags);
        print_table_header(&["FLAG", "DESCRIPTION", "EXAMPLE"], &col_widths);
        for f in &e.flags {
            print_table_row(&[&f.flag, &f.description, &f.example], &col_widths);
        }
    }

    // EXAMPLES
    if !e.examples.is_empty() {
        println!();
        print_section("EXAMPLES");
        for (i, ex) in e.examples.iter().enumerate() {
            println!("  {}. {}", (i + 1).to_string().cyan().bold(), ex.description.white());
            println!("     {}", format!("$ {}", ex.command).green());
            println!();
        }
    }

    // COMMON MISTAKES
    if !e.common_mistakes.is_empty() {
        println!();
        print_section("COMMON MISTAKES");
        for m in &e.common_mistakes {
            println!("  {} {}", "✗".red(), m);
        }
    }

    // PERFORMANCE TIPS
    if !e.performance_tips.is_empty() {
        println!();
        print_section("PERFORMANCE TIPS");
        for t in &e.performance_tips {
            println!("  {} {}", "⚡".yellow(), t);
        }
    }

    // SECURITY NOTES
    if !e.security_notes.is_empty() {
        println!();
        print_section("SECURITY NOTES");
        for n in &e.security_notes {
            println!("  {} {}", "🔒".cyan(), n);
        }
    }

    // ALTERNATIVES
    if !e.alternatives.is_empty() {
        println!();
        print_section("ALTERNATIVES");
        let alt_widths = [20, 56];
        print_table_header(&["NAME", "REASON TO PREFER"], &alt_widths);
        for a in &e.alternatives {
            print_table_row(&[&a.name, &a.reason], &alt_widths);
        }
    }

    println!();
    print_source_indicator(&e.source, tool);
    println!();
}

// ── UI Helpers ────────────────────────────────────────────────────────────────

fn print_header(title: &str) {
    let line = "─".repeat(title.len() + 4);
    println!();
    println!("  {}", line.dimmed());
    println!("  {} {}", "│".dimmed(), title.bold().cyan());
    println!("  {}", line.dimmed());
}

fn print_section(title: &str) {
    println!("  {}", title.bold().yellow());
    println!("  {}", "─".repeat(title.len()).dimmed());
}

fn print_boxed_section(title: &str, content: &str) {
    let wrapped = textwrap_paragraph(content, 74);
    let max_width = wrapped.lines().map(|l| l.len()).max().unwrap_or(0).max(title.len() + 2);
    let top = format!("╭─ {} {}╮", title.bold().cyan(), "─".repeat(max_width - title.len()));
    let bottom = format!("╰{}╯", "─".repeat(max_width + 3));
    println!("  {}", top.dimmed());
    for line in wrapped.lines() {
        println!("  {}  {}  {}", "│".dimmed(), line, "│".dimmed());
    }
    println!("  {}", bottom.dimmed());
}

fn print_code_block(code: &str) {
    println!("  {}", "┌─────────────────────────────────────────────┐".dimmed());
    for line in code.lines() {
        println!("  {} {}", "│".dimmed(), line.green());
    }
    println!("  {}", "└─────────────────────────────────────────────┘".dimmed());
}

fn print_table_header(headers: &[&str], widths: &[usize]) {
    let row: Vec<String> = headers
        .iter()
        .zip(widths.iter())
        .map(|(h, &w)| format!("{:<width$}", h.bold().to_string(), width = w))
        .collect();
    println!("  {}", row.join("  ").cyan().bold());
    let sep: Vec<String> = widths.iter().map(|&w| "─".repeat(w)).collect();
    println!("  {}", sep.join("  ").dimmed());
}

fn print_table_row(cells: &[&str], widths: &[usize]) {
    let row: Vec<String> = cells
        .iter()
        .zip(widths.iter())
        .map(|(c, &w)| {
            let s = if c.len() > w { format!("{}…", &c[..w.saturating_sub(1)]) } else { c.to_string() };
            format!("{:<width$}", s, width = w)
        })
        .collect();
    println!("  {}", row.join("  "));
}

fn print_source_indicator(source: &ExplainSource, tool: &str) {
    let label = match source {
        ExplainSource::Ai { provider, model } => {
            format!("Source: AI ({provider} / {model})", provider = provider, model = model)
        }
        ExplainSource::LocalHelpPage => format!("Source: `{tool} --help` (no AI configured)"),
        ExplainSource::Fallback => "Source: fallback".to_string(),
    };
    println!("  {}", label.dimmed().italic());
}

fn compute_col_widths(flags: &[Flag]) -> [usize; 3] {
    let w0 = flags.iter().map(|f| f.flag.len()).max().unwrap_or(10).clamp(10, 24);
    let w2 = flags.iter().map(|f| f.example.len()).max().unwrap_or(20).clamp(20, 32);
    let w1 = (80usize).saturating_sub(w0 + w2 + 8).max(20);
    [w0, w1, w2]
}

fn textwrap_paragraph(text: &str, width: usize) -> String {
    let mut result = String::new();
    let mut line_len = 0usize;
    for word in text.split_whitespace() {
        if line_len > 0 && line_len + word.len() + 1 > width {
            result.push('\n');
            line_len = 0;
        }
        if line_len > 0 {
            result.push(' ');
            line_len += 1;
        }
        result.push_str(word);
        line_len += word.len();
    }
    result
}
