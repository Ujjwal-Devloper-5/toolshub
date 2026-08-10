//! `toolshub context <path>` — detect and display project context.

use colored::Colorize;
use std::path::{Path, PathBuf};
use tracing::instrument;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `toolshub context`.
#[instrument]
pub async fn run(path: &str) -> anyhow::Result<()> {
    let root = PathBuf::from(path).canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    let ctx = detect_context(&root).await;

    println!();
    println!("  {} {}", "◆".cyan().bold(), "Project Context".bold().cyan());
    println!("  {}", "─".repeat(60).dimmed());
    println!();
    kv("Directory", &root.to_string_lossy());

    if ctx.project_types.is_empty() {
        println!("  {} No recognised project type detected.", "○".yellow());
    } else {
        kv("Project type(s)", &ctx.project_types.join(", "));
    }

    if let Some(ref git) = ctx.git_branch {
        kv("Git branch", git);
    }
    if let Some(ref remote) = ctx.git_remote {
        kv("Git remote", remote);
    }

    if !ctx.notable_files.is_empty() {
        println!();
        section("KEY FILES");
        for f in &ctx.notable_files {
            println!("    {} {}", "•".cyan(), f.dimmed());
        }
    }

    if !ctx.suggested_tools.is_empty() {
        println!();
        section("SUGGESTED TOOLS");
        for t in &ctx.suggested_tools {
            println!(
                "    {} {} — {}",
                "→".green(),
                t.0.bold().green(),
                t.1.dimmed()
            );
        }
        println!();
        println!(
            "  {} Use {} to learn about any of these tools",
            "tip".dimmed(),
            "toolshub explain <tool>".green()
        );
    }

    println!();
    Ok(())
}

// ── Context detection ─────────────────────────────────────────────────────────

struct ProjectContext {
    project_types: Vec<String>,
    git_branch: Option<String>,
    git_remote: Option<String>,
    notable_files: Vec<String>,
    /// (tool_name, reason)
    suggested_tools: Vec<(String, String)>,
}

async fn detect_context(root: &Path) -> ProjectContext {
    let mut project_types = Vec::new();
    let mut notable_files = Vec::new();
    let mut suggested_tools: Vec<(String, String)> = Vec::new();

    // --- Rust ---
    if root.join("Cargo.toml").exists() {
        project_types.push("Rust".into());
        notable_files.push("Cargo.toml".into());
        if root.join("Cargo.lock").exists() {
            notable_files.push("Cargo.lock".into());
        }
        suggested_tools.push(("cargo".into(), "build, test, run Rust code".into()));
        suggested_tools.push(("clippy".into(), "Rust linter".into()));
    }

    // --- Node / JS ---
    if root.join("package.json").exists() {
        let pm = if root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else if root.join("bun.lockb").exists() {
            "bun"
        } else {
            "npm"
        };
        project_types.push(format!("Node.js ({pm})"));
        notable_files.push("package.json".into());
        suggested_tools.push((pm.into(), "package manager for this project".into()));
        if root.join("next.config.js").exists() || root.join("next.config.ts").exists() {
            project_types.push("Next.js".into());
        }
    }

    // --- Python ---
    if root.join("pyproject.toml").exists()
        || root.join("setup.py").exists()
        || root.join("requirements.txt").exists()
    {
        project_types.push("Python".into());
        if root.join("pyproject.toml").exists() {
            notable_files.push("pyproject.toml".into());
        }
        if root.join("requirements.txt").exists() {
            notable_files.push("requirements.txt".into());
        }
        suggested_tools.push(("pip".into(), "install Python dependencies".into()));
        suggested_tools.push(("pytest".into(), "run tests".into()));
    }

    // --- Go ---
    if root.join("go.mod").exists() {
        project_types.push("Go".into());
        notable_files.push("go.mod".into());
        suggested_tools.push(("go".into(), "build, test, run Go code".into()));
    }

    // --- Docker ---
    if root.join("Dockerfile").exists() || root.join("docker-compose.yml").exists()
        || root.join("docker-compose.yaml").exists()
    {
        project_types.push("Docker".into());
        if root.join("Dockerfile").exists() {
            notable_files.push("Dockerfile".into());
        }
        if root.join("docker-compose.yml").exists() {
            notable_files.push("docker-compose.yml".into());
        } else if root.join("docker-compose.yaml").exists() {
            notable_files.push("docker-compose.yaml".into());
        }
        suggested_tools.push(("docker".into(), "build and run containers".into()));
        suggested_tools.push(("docker-compose".into(), "manage multi-container apps".into()));
    }

    // --- Kubernetes / Helm ---
    if root.join("Chart.yaml").exists() {
        project_types.push("Helm chart".into());
        notable_files.push("Chart.yaml".into());
        suggested_tools.push(("helm".into(), "Kubernetes package manager".into()));
        suggested_tools.push(("kubectl".into(), "Kubernetes CLI".into()));
    }

    // --- Terraform ---
    if has_extension(root, "tf") {
        project_types.push("Terraform".into());
        suggested_tools.push(("terraform".into(), "infrastructure provisioning".into()));
    }

    // --- Nix ---
    if root.join("flake.nix").exists() || root.join("default.nix").exists() {
        project_types.push("Nix".into());
        if root.join("flake.nix").exists() {
            notable_files.push("flake.nix".into());
        }
        suggested_tools.push(("nix".into(), "reproducible builds".into()));
    }

    // --- Git ---
    let (git_branch, git_remote) = detect_git(root).await;

    // Deduplicate suggested tools.
    suggested_tools.dedup_by(|a, b| a.0 == b.0);

    ProjectContext {
        project_types,
        git_branch,
        git_remote,
        notable_files,
        suggested_tools,
    }
}

async fn detect_git(root: &Path) -> (Option<String>, Option<String>) {
    if !root.join(".git").exists() {
        return (None, None);
    }

    // Read branch from HEAD.
    // Use spawn_blocking to avoid stalling the Tokio executor on slow
    // filesystems (NFS, encrypted volumes, etc.).
    let head_path = root.join(".git").join("HEAD");
    let git_config_path = root.join(".git").join("config");

    let (branch, remote) = tokio::task::spawn_blocking(move || {
        let branch = std::fs::read_to_string(&head_path)
            .ok()
            .and_then(|s| {
                s.trim()
                    .strip_prefix("ref: refs/heads/")
                    .map(|b| b.to_string())
            });

        // Read origin URL from config.
        let remote = std::fs::read_to_string(&git_config_path)
            .ok()
            .and_then(|s| {
                let mut in_origin = false;
                for line in s.lines() {
                    let t = line.trim();
                    if t == "[remote \"origin\"]" {
                        in_origin = true;
                        continue;
                    }
                    if in_origin && t.starts_with('[') {
                        break;
                    }
                    if in_origin && t.starts_with("url = ") {
                        return Some(t.trim_start_matches("url = ").to_string());
                    }
                }
                None
            });

        (branch, remote)
    })
    .await
    .unwrap_or((None, None));

    (branch, remote)
}

fn has_extension(dir: &Path, ext: &str) -> bool {
    // NOTE: This is intentionally synchronous and called only from
    // detect_context which itself is wrapped in spawn_blocking at call sites.
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == ext)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("  {}", title.bold().yellow());
    println!("  {}", "─".repeat(title.len()).dimmed());
}

fn kv(key: &str, value: &str) {
    println!("    {:<18} {}", format!("{key}:").cyan(), value);
}
