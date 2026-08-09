//! `toolshub memory <action>` — inspect and manage the AI memory store.

use colored::Colorize;
use std::sync::Arc;
use tracing::instrument;

use toolshub_memory::store::MemoryStore;
use crate::MemoryAction;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub memory <action>`.
#[instrument(skip(memory))]
pub async fn run(
    action: MemoryAction,
    memory: Option<Arc<MemoryStore>>,
) -> anyhow::Result<()> {
    let store = memory.ok_or_else(|| {
        anyhow::anyhow!(
            "Memory is disabled or unavailable.\n\
             Enable it with: toolshub config set memory.enabled true"
        )
    })?;

    match action {
        MemoryAction::Show => show_facts(&store).await?,
        MemoryAction::Clear => clear_all(&store).await?,
        MemoryAction::Summaries => show_summaries(&store).await?,
        MemoryAction::Cache => show_cache(&store).await?,
        MemoryAction::History => show_history(&store).await?,
    }

    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn show_facts(store: &MemoryStore) -> anyhow::Result<()> {
    let facts = store.list_all_facts().await?;
    let stats = store.stats().await?;

    println!();
    println!("  {} {}", "◆".cyan().bold(), "Memory Store".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();
    println!("  {:<24} {}", "Facts stored:".cyan(), stats.facts);
    println!("  {:<24} {}", "Cached docs:".cyan(), stats.doc_cache);
    println!("  {:<24} {}", "Tool usage records:".cyan(), stats.tool_usage);
    println!("  {:<24} {}", "Conversation summaries:".cyan(), stats.conversation_summaries);
    println!();

    if facts.is_empty() {
        println!("  {} No facts stored yet.", "○".yellow());
        println!();
        println!(
            "  {} Facts are stored automatically as you use ToolsHub.",
            "info".dimmed()
        );
        return Ok(());
    }

    println!("  {} {}", "FACTS".bold().yellow(), format!("({} total)", facts.len()).dimmed());
    println!("  {}", "─".repeat(60).dimmed());

    let cat_w = facts.iter().map(|f| f.category.len()).max().unwrap_or(8).clamp(8, 16);
    let key_w = facts.iter().map(|f| f.key.len()).max().unwrap_or(12).clamp(12, 24);

    println!(
        "  {:<cw$}  {:<kw$}  {}",
        "CATEGORY".bold(),
        "KEY".bold(),
        "VALUE".bold(),
        cw = cat_w,
        kw = key_w,
    );
    println!("  {}", "─".repeat(cat_w + key_w + 40).dimmed());

    for f in &facts {
        println!(
            "  {:<cw$}  {:<kw$}  {}",
            f.category.cyan(),
            f.key.green(),
            f.value.trim(),
            cw = cat_w,
            kw = key_w,
        );
    }
    println!();
    Ok(())
}

async fn clear_all(store: &MemoryStore) -> anyhow::Result<()> {
    use dialoguer::Confirm;

    println!();
    println!(
        "  {} This will permanently delete ALL memory data:",
        "⚠".red().bold()
    );
    println!("    • All stored facts");
    println!("    • All cached documentation");
    println!("    • All tool usage history");
    println!("    • All conversation summaries");
    println!();

    let ok = Confirm::new()
        .with_prompt("  Are you sure?")
        .default(false)
        .interact()
        .unwrap_or(false);

    if !ok {
        println!("  {}", "Cancelled.".yellow());
        return Ok(());
    }

    store.clear_all().await?;
    println!("  {} Memory cleared.", "✓".green().bold());
    Ok(())
}

async fn show_summaries(store: &MemoryStore) -> anyhow::Result<()> {
    let summaries = store.list_conversation_summaries(20).await?;

    println!();
    println!("  {} {}", "◆".cyan().bold(), "Conversation Summaries".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    if summaries.is_empty() {
        println!("  {} No conversation summaries stored yet.", "○".yellow());
        return Ok(());
    }

    for (i, s) in summaries.iter().enumerate() {
        println!(
            "  {}. {} {}",
            (i + 1).to_string().cyan(),
            s.created_at.dimmed(),
            format!("[{}]", s.topics.join(", ")).dimmed()
        );
        // Word-wrap summary.
        for line in wrap_text(&s.summary, 68).lines() {
            println!("     {}", line);
        }
        println!();
    }
    Ok(())
}

async fn show_cache(store: &MemoryStore) -> anyhow::Result<()> {
    let entries = store.list_cached_docs().await?;

    println!();
    println!("  {} {}", "◆".cyan().bold(), "Documentation Cache".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    if entries.is_empty() {
        println!("  {} No cached documentation yet.", "○".yellow());
        println!();
        println!(
            "  {} Run {} to populate the cache.",
            "tip".dimmed(),
            "toolshub explain <tool>".green()
        );
        return Ok(());
    }

    let tool_w = entries.iter().map(|e| e.tool_name.len()).max().unwrap_or(10).clamp(10, 24);

    println!(
        "  {:<tw$}  {:<16}  {:<20}  {}",
        "TOOL".bold(),
        "SOURCE".bold(),
        "CACHED AT".bold(),
        "EXPIRES AT".bold(),
        tw = tool_w,
    );
    println!("  {}", "─".repeat(tool_w + 60).dimmed());

    for e in &entries {
        println!(
            "  {:<tw$}  {:<16}  {:<20}  {}",
            e.tool_name.green(),
            e.source.cyan(),
            &e.cached_at[..19],
            &e.expires_at[..19],
            tw = tool_w,
        );
    }
    println!();
    Ok(())
}

async fn show_history(store: &MemoryStore) -> anyhow::Result<()> {
    let usages = store.list_tool_usage(50).await?;

    println!();
    println!("  {} {}", "◆".cyan().bold(), "Tool Usage History".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();

    if usages.is_empty() {
        println!("  {} No tool usage recorded yet.", "○".yellow());
        return Ok(());
    }

    println!(
        "  {:<22}  {:<8}  {}",
        "TIME".bold(),
        "EXIT".bold(),
        "COMMAND".bold(),
    );
    println!("  {}", "─".repeat(80).dimmed());

    for u in &usages {
        let time = &u.used_at[..19];
        let exit = if u.exit_code == 0 {
            "0".green().to_string()
        } else {
            u.exit_code.to_string().red().to_string()
        };
        let cmd = if u.command.len() > 54 {
            format!("{}…", &u.command[..53])
        } else {
            u.command.clone()
        };
        println!(
            "  {:<22}  {:<8}  {}",
            time.dimmed(),
            exit,
            cmd,
        );
    }
    println!();
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn wrap_text(text: &str, width: usize) -> String {
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
