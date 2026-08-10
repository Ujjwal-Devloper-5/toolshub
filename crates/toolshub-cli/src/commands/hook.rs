//! `toolshub hook` — emit a shell integration snippet.
//!
//! The emitted snippet wraps the shell's `preexec` hook (bash/zsh) or
//! a `fish_preexec` function (fish) to run every command through the
//! ToolsHub safety checker before execution.
//!
//! Usage:
//!   # Bash
//!   eval "$(toolshub hook bash)"
//!
//!   # Zsh
//!   eval "$(toolshub hook zsh)"
//!
//!   # Fish
//!   toolshub hook fish | source

use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

pub async fn run(shell: Shell, install: bool) -> anyhow::Result<()> {
    let snippet = match shell {
        Shell::Bash => bash_hook(),
        Shell::Zsh  => zsh_hook(),
        Shell::Fish => fish_hook(),
    };

    if install {
        install_hook(shell, &snippet)?;
    } else {
        print!("{snippet}");
    }

    Ok(())
}

// ── Shell snippets ────────────────────────────────────────────────────────────

fn bash_hook() -> String {
    r#"
# ── ToolsHub shell hook (bash) ─────────────────────────────────────────────
# Add to ~/.bashrc:  eval "$(toolshub hook bash)"
__toolshub_preexec() {
    local cmd="$1"
    # Only check non-trivial commands (skip single words, cd, etc.)
    if [[ "${cmd}" =~ [[:space:]] ]] && [[ "${cmd}" != cd\ * ]]; then
        toolshub run --dry-run ${cmd} 2>/dev/null || true
    fi
}

# Install preexec hook using bash-preexec if available, otherwise DEBUG trap
if declare -f preexec_functions > /dev/null 2>&1; then
    preexec_functions+=(__toolshub_preexec)
else
    __toolshub_last_cmd=""
    __toolshub_debug_trap() {
        if [[ "$BASH_COMMAND" != __toolshub_* && -z "$COMP_LINE" ]]; then
            __toolshub_preexec "$BASH_COMMAND"
        fi
    }
    trap '__toolshub_debug_trap' DEBUG
fi
# ── End ToolsHub hook ───────────────────────────────────────────────────────
"#.to_string()
}

fn zsh_hook() -> String {
    r#"
# ── ToolsHub shell hook (zsh) ──────────────────────────────────────────────
# Add to ~/.zshrc:  eval "$(toolshub hook zsh)"
__toolshub_preexec() {
    local cmd="$1"
    # Only safety-check commands with arguments (skip bare words and cd)
    if [[ "${cmd}" == *\ * ]] && [[ "${cmd}" != cd\ * ]]; then
        toolshub run --dry-run ${cmd} 2>/dev/null || true
    fi
}

# Zsh has a built-in preexec hook array
autoload -Uz add-zsh-hook
add-zsh-hook preexec __toolshub_preexec
# ── End ToolsHub hook ───────────────────────────────────────────────────────
"#.to_string()
}

fn fish_hook() -> String {
    r#"
# ── ToolsHub shell hook (fish) ─────────────────────────────────────────────
# Add to ~/.config/fish/config.fish:  toolshub hook fish | source
function __toolshub_preexec --on-event fish_preexec
    set cmd $argv[1]
    # Only check commands with arguments
    if string match -q '* *' -- $cmd
        and not string match -q 'cd *' -- $cmd
        toolshub run --dry-run $cmd 2>/dev/null; or true
    end
end
# ── End ToolsHub hook ───────────────────────────────────────────────────────
"#.to_string()
}

// ── Auto-install ──────────────────────────────────────────────────────────────

fn install_hook(shell: Shell, snippet: &str) -> anyhow::Result<()> {
    let (rc_file, eval_line) = match shell {
        Shell::Bash => (
            shellexpand::tilde("~/.bashrc").to_string(),
            r#"eval "$(toolshub hook bash)"  # ToolsHub hook"#.to_string(),
        ),
        Shell::Zsh => (
            shellexpand::tilde("~/.zshrc").to_string(),
            r#"eval "$(toolshub hook zsh)"   # ToolsHub hook"#.to_string(),
        ),
        Shell::Fish => (
            shellexpand::tilde("~/.config/fish/config.fish").to_string(),
            "toolshub hook fish | source     # ToolsHub hook".to_string(),
        ),
    };

    // Check if already installed
    let existing = std::fs::read_to_string(&rc_file).unwrap_or_default();
    if existing.contains("toolshub hook") {
        println!();
        println!(
            "  {} ToolsHub hook already installed in {}",
            "✓".green().bold(),
            rc_file.cyan()
        );
        println!("  Restart your shell or run: source {rc_file}");
        println!();
        return Ok(());
    }

    // Append the eval line
    let mut content = existing;
    content.push_str("\n\n");
    content.push_str(&eval_line);
    content.push('\n');
    std::fs::write(&rc_file, &content)?;

    println!();
    println!(
        "  {} Hook installed in {}",
        "✓".green().bold(),
        rc_file.cyan().bold()
    );
    println!();
    println!("  Restart your shell or run:");
    println!("  {}", format!("source {rc_file}").green());
    println!();
    println!("  The hook will now safety-check every command with arguments before execution.");
    println!("  To disable temporarily: unset -f __toolshub_preexec");
    println!();

    // Also print the snippet for reference
    println!("  {}", "─".repeat(60).dimmed());
    println!("  {} What was added:", "ℹ".cyan());
    for line in snippet.lines() {
        println!("  {}", line.dimmed());
    }

    Ok(())
}
