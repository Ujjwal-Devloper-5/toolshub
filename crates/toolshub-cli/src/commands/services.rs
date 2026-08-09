//! `toolshub services` — list and inspect systemd services (Linux only).

use colored::Colorize;
use std::process::Stdio;
use tokio::process::Command;
use tracing::instrument;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub services`.
#[instrument]
pub async fn run(filter: Option<&str>, failed_only: bool) -> anyhow::Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (filter, failed_only);
        println!();
        println!("  {} Service management is only available on Linux.", "⚠".yellow().bold());
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    run_linux(filter, failed_only).await
}

#[cfg(target_os = "linux")]
async fn run_linux(filter: Option<&str>, failed_only: bool) -> anyhow::Result<()> {
    // Check systemctl is available.
    if which::which("systemctl").is_err() {
        println!();
        println!("  {} systemctl not found — is systemd running?", "⚠".yellow().bold());
        return Ok(());
    }

    let services = list_services().await?;

    let mut filtered: Vec<&ServiceEntry> = services.iter().collect();

    if failed_only {
        filtered.retain(|s| s.active == "failed");
    }

    if let Some(f) = filter {
        let fl = f.to_lowercase();
        filtered.retain(|s| s.name.to_lowercase().contains(&fl));
    }

    filtered.sort_by(|a, b| a.name.cmp(&b.name));

    println!();
    println!(
        "  {} {} service(s){}{}",
        "◆".cyan().bold(),
        filtered.len().to_string().bold(),
        if failed_only { " (failed only)" } else { "" }.red().to_string(),
        filter
            .map(|f| format!(" (filter: \"{}\")", f))
            .unwrap_or_default()
            .dimmed()
            .to_string()
    );
    println!();

    if filtered.is_empty() {
        println!("  No services match the current filter.");
        return Ok(());
    }

    let name_w = filtered.iter().map(|s| s.name.len()).max().unwrap_or(20).min(40).max(20);
    let load_w = 10usize;
    let active_w = 10usize;
    let sub_w = 12usize;

    // Header
    println!(
        "  {:<nw$}  {:<lw$}  {:<aw$}  {:<sw$}  {}",
        "SERVICE".bold(),
        "LOAD".bold(),
        "ACTIVE".bold(),
        "SUB".bold(),
        "DESCRIPTION".bold(),
        nw = name_w,
        lw = load_w,
        aw = active_w,
        sw = sub_w,
    );
    println!("  {}", "─".repeat(name_w + load_w + active_w + sub_w + 50).dimmed());

    for svc in &filtered {
        let name = truncate(&svc.name, name_w);
        let load = truncate(&svc.load, load_w);
        let active = color_active(&svc.active, active_w);
        let sub = truncate(&svc.sub, sub_w);
        let desc = truncate(&svc.description, 40);

        println!(
            "  {:<nw$}  {:<lw$}  {}  {:<sw$}  {}",
            name.cyan(),
            load.dimmed(),
            active,
            sub.dimmed(),
            desc,
            nw = name_w,
            lw = load_w,
            sw = sub_w,
        );
    }

    println!();
    println!(
        "  {} Use {} to view logs, {} to restart",
        "tip".dimmed(),
        "journalctl -u <service>".green(),
        "systemctl restart <service>".green()
    );
    println!();

    Ok(())
}

// ── systemctl parsing ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct ServiceEntry {
    name: String,
    load: String,
    active: String,
    sub: String,
    description: String,
}

#[cfg(target_os = "linux")]
async fn list_services() -> anyhow::Result<Vec<ServiceEntry>> {
    let output = Command::new("systemctl")
        .args(["list-units", "--type=service", "--all", "--no-pager", "--plain"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run systemctl: {e}"))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in text.lines() {
        // Each line format:
        //   UNIT                   LOAD   ACTIVE SUB     DESCRIPTION
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("UNIT")
            || line.starts_with("LOAD")
            || line.starts_with("To show")
            || line.starts_with("Legend:")
        {
            continue;
        }

        let parts: Vec<&str> = line.splitn(5, char::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() >= 4 {
            entries.push(ServiceEntry {
                name: parts[0].trim_end_matches(".service").to_string(),
                load: parts.get(1).unwrap_or(&"—").to_string(),
                active: parts.get(2).unwrap_or(&"—").to_string(),
                sub: parts.get(3).unwrap_or(&"—").to_string(),
                description: parts.get(4).unwrap_or(&"").to_string(),
            });
        }
    }

    Ok(entries)
}

fn color_active(active: &str, width: usize) -> String {
    let s = truncate(active, width);
    match active {
        "active" => format!("{:<w$}", s.green().bold(), w = width),
        "failed" => format!("{:<w$}", s.red().bold(), w = width),
        "inactive" => format!("{:<w$}", s.dimmed(), w = width),
        "activating" => format!("{:<w$}", s.yellow(), w = width),
        _ => format!("{:<w$}", s.white(), w = width),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
