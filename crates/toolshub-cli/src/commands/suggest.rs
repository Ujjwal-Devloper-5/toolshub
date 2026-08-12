//! `toolshub suggest <goal>` — AI-powered command planner.
//!
//! Given a natural language goal (e.g. "find all large files taking up disk space"),
//! the AI returns a numbered, step-by-step command sequence. Each step is
//! safety-checked and rendered with a danger badge before presenting it to the user.
//!
//! The AI's response is streamed token-by-token so the user sees the plan
//! being generated in real time.

use std::sync::Arc;

use colored::Colorize;
use tracing::instrument;
use uuid::Uuid;

use toolshub_ai::{router::AiRouter, CompletionRequest, Message, Role};
use toolshub_core::config::Config;
use toolshub_core::types::DangerLevel;
use toolshub_memory::store::MemoryStore;
use toolshub_safety::SafetyChecker;

use chrono::Utc;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub suggest <goal>`.
#[instrument(skip(memory, ai, config))]
pub async fn run(
    goal: &str,
    memory: Option<Arc<MemoryStore>>,
    ai: Option<Arc<AiRouter>>,
    config: &Config,
) -> anyhow::Result<()> {
    if goal.trim().is_empty() {
        anyhow::bail!("Please describe your goal. Example: toolshub suggest 'find large files and delete them safely'");
    }

    println!();
    println!(
        "  {} {}",
        "◆ Suggest:".bold().magenta(),
        goal.white().bold()
    );
    println!("  {}", "─".repeat(60).dimmed());

    let Some(ref ai) = ai else {
        println!();
        println!("  {} AI is not configured. Run:", "⚠".yellow());
        println!("  {}", "toolshub config set ai.provider openrouter".green());
        println!();
        return Ok(());
    };

    // Build system prompt enriched with environment context
    let env_ctx = build_env_context(&memory).await;

    let system_prompt = format!(
        "You are ToolsHub, an expert CLI command planner. \
         Given a goal, you produce a numbered step-by-step command sequence to achieve it. \
         Each step must be on its own line in this exact format:\n\
         STEP N: <description>\n\
         CMD: <actual_shell_command>\n\
         RISK: <safe|low|medium|high|critical>\n\
         EXPLANATION: <why this step and what it does>\n\n\
         Rules:\n\
         - Provide at least 3 and at most 8 steps.\n\
         - Commands must be real, copy-pasteable shell commands.\n\
         - RISK must be one of: safe, low, medium, high, critical.\n\
         - Tailor commands to the user's environment.\n\
         - Include a WARNING at the end if any step is irreversible.\n\n\
         User environment:\n{env_ctx}"
    );

    let user_msg = format!("Goal: {goal}");

    println!();
    println!(
        "  {} Planning command sequence…",
        "◈".magenta().bold()
    );
    println!();

    let req = CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: system_prompt },
            Message { role: Role::User,   content: user_msg },
        ],
        max_tokens: config.ai.max_tokens,
        temperature: 0.3,
        stream: true,
    };

    // Stream response
    let resp = ai.complete_streaming(req).await?;

    // Parse the structured response and re-render with safety badges
    println!();
    println!("  {}", "─".repeat(60).dimmed());
    render_plan(&resp.content, config);
    println!("  {}", "─".repeat(60).dimmed());

    // Record in memory
    if let Some(ref store) = memory {
        let usage = toolshub_memory::ToolUsage {
            id: Uuid::new_v4().to_string(),
            tool_name: "suggest".into(),
            command: format!("toolshub suggest {goal}"),
            directory: cwd(),
            exit_code: 0,
            used_at: Utc::now().to_rfc3339(),
        };
        let _ = store.record_tool_usage(&usage).await;
    }

    Ok(())
}

// ── Plan renderer ─────────────────────────────────────────────────────────────

fn render_plan(raw: &str, config: &Config) {
    let threshold = parse_danger_level(&config.safety.danger_level_threshold);
    let checker = SafetyChecker::with_threshold(threshold);

    let mut step_n = 0usize;
    let mut current_desc = String::new();
    let mut current_cmd = String::new();
    let mut current_risk = String::new();
    let mut current_expl = String::new();

    let flush_step = |n: usize, desc: &str, cmd: &str, risk: &str, expl: &str, checker: &SafetyChecker| {
        if cmd.is_empty() { return; }

        // Safety-check the command
        let report = checker.analyse(cmd);
        let danger_badge = match risk.to_lowercase().as_str() {
            "safe"                    => "  ✓ SAFE    ".green().to_string(),
            "low"                     => "  ⚡ LOW     ".yellow().to_string(),
            "medium"                  => "  ⚠ MEDIUM  ".yellow().bold().to_string(),
            "high" | "critical"       => "  ✗ HIGH    ".red().bold().to_string(),
            _ if report.is_clean()    => "  ✓ SAFE    ".green().to_string(),
            _                         => "  ⚠ WARNING ".yellow().to_string(),
        };

        println!();
        println!(
            "  {}  {}",
            format!("Step {n}").bold().magenta(),
            desc.white().bold()
        );
        println!("{}{}", danger_badge, cmd.green().bold());
        if !expl.is_empty() {
            println!("  {}", expl.dimmed());
        }

        // Extra safety warnings from the checker
        if !report.concerns.is_empty() {
            for concern in &report.concerns {
                println!(
                    "  {} {} — {}",
                    concern.level.badge(),
                    concern.label.yellow(),
                    concern.consequence.dimmed()
                );
            }
        }
    };

    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("STEP ") {
            // Flush previous step
            if !current_cmd.is_empty() {
                flush_step(step_n, &current_desc, &current_cmd, &current_risk, &current_expl, &checker);
            }
            step_n += 1;
            current_desc = line
                .split_once(':')
                .map(|x| x.1)
                .unwrap_or(line)
                .trim()
                .to_string();
            current_cmd.clear();
            current_risk.clear();
            current_expl.clear();
        } else if let Some(cmd) = line.strip_prefix("CMD:") {
            current_cmd = cmd.trim().to_string();
        } else if let Some(risk) = line.strip_prefix("RISK:") {
            current_risk = risk.trim().to_string();
        } else if let Some(expl) = line.strip_prefix("EXPLANATION:") {
            current_expl = expl.trim().to_string();
        } else if line.starts_with("WARNING:") {
            // After flushing the last step, show global warning
            flush_step(step_n, &current_desc, &current_cmd, &current_risk, &current_expl, &checker);
            current_cmd.clear();
            println!();
            println!(
                "  {} {}",
                "⚠  WARNING:".red().bold(),
                line.trim_start_matches("WARNING:").trim().yellow()
            );
        }
    }

    // Flush the final step
    if !current_cmd.is_empty() {
        flush_step(step_n, &current_desc, &current_cmd, &current_risk, &current_expl, &checker);
    }

    println!();
    println!(
        "  {} Run each step with {} for live safety checking",
        "tip:".dimmed(),
        "toolshub run <command>".green()
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn build_env_context(memory: &Option<Arc<MemoryStore>>) -> String {
    let mut ctx = String::new();
    if let Some(ref store) = memory {
        for cat in &["os", "shell", "project"] {
            if let Ok(facts) = store.get_facts_by_category(cat).await {
                for f in &facts {
                    ctx.push_str(&format!("  [{cat}] {}: {}\n", f.key, f.value));
                }
            }
        }
    }
    if ctx.is_empty() {
        ctx.push_str("  No environment context stored yet.\n");
    }
    ctx
}

fn parse_danger_level(s: &str) -> DangerLevel {
    match s.to_lowercase().as_str() {
        "low"      => DangerLevel::Low,
        "medium"   => DangerLevel::Medium,
        "high"     => DangerLevel::High,
        "critical" => DangerLevel::Critical,
        _          => DangerLevel::High,
    }
}

fn cwd() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string())
}
