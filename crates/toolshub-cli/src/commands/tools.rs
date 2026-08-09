//! `toolshub tools` — discover and list every executable in $PATH.

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::instrument;

use toolshub_env::EnvironmentSnapshot;
use toolshub_core::types::InstalledTool;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub tools`.
#[instrument]
pub async fn run(filter: Option<&str>, compact: bool) -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Scanning PATH for tools…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let snapshot = EnvironmentSnapshot::capture().await?;

    spinner.finish_and_clear();

    let mut tools: Vec<&InstalledTool> = snapshot.installed_tools.iter().collect();

    // Apply filter.
    if let Some(f) = filter {
        let fl = f.to_lowercase();
        tools.retain(|t| t.name.to_lowercase().contains(&fl));
    }

    // Sort alphabetically.
    tools.sort_by(|a, b| a.name.cmp(&b.name));

    println!();
    println!(
        "  {} {} tool(s) found in PATH{}",
        "◆".cyan().bold(),
        tools.len().to_string().bold(),
        filter.map(|f| format!(" (filter: \"{}\")", f)).unwrap_or_default().dimmed().to_string()
    );
    println!();

    if tools.is_empty() {
        println!("  No tools match the current filter.");
        return Ok(());
    }

    if compact {
        render_compact(&tools);
    } else {
        render_table(&tools);
    }

    println!();
    println!(
        "  {} Use {} to learn about any tool",
        "tip".dimmed(),
        "toolshub explain <tool>".green()
    );
    println!();

    Ok(())
}

// ── Renderers ─────────────────────────────────────────────────────────────────

fn render_compact(tools: &[&InstalledTool]) {
    // Print as a multi-column list (4 columns).
    let col_w = 20usize;
    let cols = 4usize;
    let mut col = 0;
    for tool in tools {
        let name = if tool.name.len() > col_w - 2 {
            format!("{}…", &tool.name[..col_w - 3])
        } else {
            tool.name.clone()
        };
        print!("  {:<col_w$}", name.green());
        col += 1;
        if col >= cols {
            println!();
            col = 0;
        }
    }
    if col > 0 {
        println!();
    }
}

fn render_table(tools: &[&InstalledTool]) {
    let name_w = tools.iter().map(|t| t.name.len()).max().unwrap_or(10).min(28).max(10);
    let ver_w = 14usize;
    let pm_w = 10usize;
    let path_w = 44usize;

    // Header
    println!(
        "  {:<nw$}  {:<vw$}  {:<pw$}  {}",
        "NAME".bold(),
        "VERSION".bold(),
        "MANAGER".bold(),
        "PATH".bold(),
        nw = name_w,
        vw = ver_w,
        pw = pm_w,
    );
    println!(
        "  {}",
        "─".repeat(name_w + ver_w + pm_w + path_w + 6).dimmed()
    );

    for tool in tools {
        let name = truncate(&tool.name, name_w);
        let ver = tool.version.as_deref().unwrap_or("—");
        let ver = truncate(ver, ver_w);
        let pm = tool
            .package_manager
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "—".to_string());
        let pm = truncate(&pm, pm_w);
        let path = tool.path.to_string_lossy();
        let path = truncate(&path, path_w);

        println!(
            "  {:<nw$}  {:<vw$}  {:<pw$}  {}",
            name.green(),
            ver.yellow(),
            pm.cyan(),
            path.dimmed(),
            nw = name_w,
            vw = ver_w,
            pw = pm_w,
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
