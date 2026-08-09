//! `toolshub-context` — project context detection engine.
//!
//! Inspects a directory and produces a [`ProjectContext`] describing
//! the project type, version control state, key files, and recommended tools.
//!
//! This crate is intentionally dependency-light: it uses only the standard
//! library and `toolshub-core`. All I/O is synchronous (filesystem reads)
//! to keep the startup path fast.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use toolshub_core::Result;

// ── Project types ─────────────────────────────────────────────────────────────

/// A detected technology stack or project type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectKind {
    Rust,
    NodeJs { package_manager: String },
    Python,
    Go,
    Docker,
    DockerCompose,
    Kubernetes,
    Helm,
    Terraform,
    Nix,
    Java,
    CSharp,
    Ruby,
    Php,
    Elixir,
    Haskell,
    Zig,
    Generic,
}

impl std::fmt::Display for ProjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectKind::Rust => write!(f, "Rust"),
            ProjectKind::NodeJs { package_manager } => write!(f, "Node.js ({package_manager})"),
            ProjectKind::Python => write!(f, "Python"),
            ProjectKind::Go => write!(f, "Go"),
            ProjectKind::Docker => write!(f, "Docker"),
            ProjectKind::DockerCompose => write!(f, "Docker Compose"),
            ProjectKind::Kubernetes => write!(f, "Kubernetes"),
            ProjectKind::Helm => write!(f, "Helm Chart"),
            ProjectKind::Terraform => write!(f, "Terraform"),
            ProjectKind::Nix => write!(f, "Nix"),
            ProjectKind::Java => write!(f, "Java"),
            ProjectKind::CSharp => write!(f, "C# / .NET"),
            ProjectKind::Ruby => write!(f, "Ruby"),
            ProjectKind::Php => write!(f, "PHP"),
            ProjectKind::Elixir => write!(f, "Elixir"),
            ProjectKind::Haskell => write!(f, "Haskell"),
            ProjectKind::Zig => write!(f, "Zig"),
            ProjectKind::Generic => write!(f, "Generic"),
        }
    }
}

// ── Git state ─────────────────────────────────────────────────────────────────

/// Version control information for the project directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitState {
    /// Whether the directory is inside a git repository.
    pub is_git_repo: bool,
    /// The current branch name (e.g. `"main"`).
    pub branch: Option<String>,
    /// The `origin` remote URL, if configured.
    pub remote_origin: Option<String>,
    /// The root of the git repository.
    pub repo_root: Option<PathBuf>,
}

// ── Suggested tool ────────────────────────────────────────────────────────────

/// A tool suggested for the detected project type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedTool {
    pub name: String,
    pub reason: String,
    pub install_hint: Option<String>,
}

// ── Project context ───────────────────────────────────────────────────────────

/// A comprehensive snapshot of the project context for a given directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Absolute path of the directory that was analysed.
    pub root: PathBuf,

    /// All detected project types (a directory can have multiple).
    pub kinds: Vec<ProjectKind>,

    /// Notable files found in the directory (relative to `root`).
    pub notable_files: Vec<String>,

    /// Version control state.
    pub git: GitState,

    /// Tools recommended for this project type.
    pub suggested_tools: Vec<SuggestedTool>,

    /// A concise one-line summary of the context.
    pub summary: String,
}

impl ProjectContext {
    /// Returns `true` if at least one project type was detected.
    pub fn has_project(&self) -> bool {
        !self.kinds.is_empty() || self.kinds.contains(&ProjectKind::Generic)
    }

    /// Returns a comma-separated list of project types.
    pub fn kinds_display(&self) -> String {
        if self.kinds.is_empty() {
            "Unknown".to_string()
        } else {
            self.kinds.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", ")
        }
    }
}

// ── Context detector ──────────────────────────────────────────────────────────

/// Detects the project context for `root`.
///
/// Inspects the filesystem, reads `.git/HEAD`, and builds a [`ProjectContext`].
/// The detection is fully synchronous and offline.
pub fn detect(root: &Path) -> ProjectContext {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let mut kinds = Vec::new();
    let mut notable_files = Vec::new();
    let mut suggested_tools: Vec<SuggestedTool> = Vec::new();

    // ── Rust ─────────────────────────────────────────────────────────────
    if root.join("Cargo.toml").exists() {
        kinds.push(ProjectKind::Rust);
        notable("Cargo.toml", &mut notable_files);
        if root.join("Cargo.lock").exists() {
            notable("Cargo.lock", &mut notable_files);
        }
        if root.join("src/main.rs").exists() {
            notable("src/main.rs", &mut notable_files);
        } else if root.join("src/lib.rs").exists() {
            notable("src/lib.rs", &mut notable_files);
        }
        suggest("cargo", "build, test, and run Rust code", Some("Via rustup: rustup update"), &mut suggested_tools);
        suggest("clippy", "Rust linter (cargo clippy)", Some("Included with Rust toolchain"), &mut suggested_tools);
        suggest("rustfmt", "Rust code formatter", Some("Included with Rust toolchain"), &mut suggested_tools);
    }

    // ── Node.js ───────────────────────────────────────────────────────────
    if root.join("package.json").exists() {
        notable("package.json", &mut notable_files);
        let pm = detect_node_pm(&root);
        let pm_name = pm.clone();
        kinds.push(ProjectKind::NodeJs { package_manager: pm });
        if root.join("package-lock.json").exists() { notable("package-lock.json", &mut notable_files); }
        if root.join("pnpm-lock.yaml").exists() { notable("pnpm-lock.yaml", &mut notable_files); }
        if root.join("yarn.lock").exists() { notable("yarn.lock", &mut notable_files); }
        suggest(&pm_name, "install and manage Node.js packages", None, &mut suggested_tools);
        if root.join("next.config.js").exists() || root.join("next.config.ts").exists() {
            suggest("next", "Next.js framework CLI", None, &mut suggested_tools);
        }
    }

    // ── Python ────────────────────────────────────────────────────────────
    if root.join("pyproject.toml").exists()
        || root.join("setup.py").exists()
        || root.join("requirements.txt").exists()
        || root.join("Pipfile").exists()
    {
        kinds.push(ProjectKind::Python);
        if root.join("pyproject.toml").exists() { notable("pyproject.toml", &mut notable_files); }
        if root.join("requirements.txt").exists() { notable("requirements.txt", &mut notable_files); }
        if root.join("Pipfile").exists() { notable("Pipfile", &mut notable_files); }
        if root.join(".python-version").exists() { notable(".python-version", &mut notable_files); }
        suggest("python3", "Python interpreter", None, &mut suggested_tools);
        suggest("pip", "install Python packages", None, &mut suggested_tools);
        suggest("pytest", "run tests", Some("pip install pytest"), &mut suggested_tools);
        if root.join("pyproject.toml").exists() {
            suggest("ruff", "fast Python linter", Some("pip install ruff"), &mut suggested_tools);
        }
    }

    // ── Go ────────────────────────────────────────────────────────────────
    if root.join("go.mod").exists() {
        kinds.push(ProjectKind::Go);
        notable("go.mod", &mut notable_files);
        if root.join("go.sum").exists() { notable("go.sum", &mut notable_files); }
        suggest("go", "build, test, and run Go code", None, &mut suggested_tools);
        suggest("golangci-lint", "Go linter suite", Some("brew install golangci-lint"), &mut suggested_tools);
    }

    // ── Java ──────────────────────────────────────────────────────────────
    if root.join("pom.xml").exists() || root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
        kinds.push(ProjectKind::Java);
        if root.join("pom.xml").exists() {
            notable("pom.xml", &mut notable_files);
            suggest("mvn", "Maven build tool", None, &mut suggested_tools);
        } else {
            notable("build.gradle", &mut notable_files);
            suggest("gradle", "Gradle build tool", None, &mut suggested_tools);
        }
        suggest("java", "Java runtime", None, &mut suggested_tools);
    }

    // ── C# / .NET ─────────────────────────────────────────────────────────
    if has_extension(&root, "csproj") || has_extension(&root, "sln") {
        kinds.push(ProjectKind::CSharp);
        suggest("dotnet", ".NET CLI", None, &mut suggested_tools);
    }

    // ── Ruby ──────────────────────────────────────────────────────────────
    if root.join("Gemfile").exists() {
        kinds.push(ProjectKind::Ruby);
        notable("Gemfile", &mut notable_files);
        suggest("bundle", "Bundler — install Ruby gems", None, &mut suggested_tools);
        suggest("ruby", "Ruby interpreter", None, &mut suggested_tools);
    }

    // ── PHP ───────────────────────────────────────────────────────────────
    if root.join("composer.json").exists() {
        kinds.push(ProjectKind::Php);
        notable("composer.json", &mut notable_files);
        suggest("composer", "PHP dependency manager", None, &mut suggested_tools);
        suggest("php", "PHP interpreter", None, &mut suggested_tools);
    }

    // ── Elixir ────────────────────────────────────────────────────────────
    if root.join("mix.exs").exists() {
        kinds.push(ProjectKind::Elixir);
        notable("mix.exs", &mut notable_files);
        suggest("mix", "Elixir build tool", None, &mut suggested_tools);
        suggest("elixir", "Elixir runtime", None, &mut suggested_tools);
    }

    // ── Haskell ───────────────────────────────────────────────────────────
    if root.join("stack.yaml").exists() || has_extension(&root, "cabal") {
        kinds.push(ProjectKind::Haskell);
        suggest("stack", "Haskell build tool", None, &mut suggested_tools);
        suggest("cabal", "Haskell package manager", None, &mut suggested_tools);
    }

    // ── Zig ───────────────────────────────────────────────────────────────
    if root.join("build.zig").exists() {
        kinds.push(ProjectKind::Zig);
        notable("build.zig", &mut notable_files);
        suggest("zig", "Zig build tool and compiler", None, &mut suggested_tools);
    }

    // ── Docker ────────────────────────────────────────────────────────────
    if root.join("Dockerfile").exists() {
        kinds.push(ProjectKind::Docker);
        notable("Dockerfile", &mut notable_files);
        suggest("docker", "build and run containers", None, &mut suggested_tools);
    }
    if root.join("docker-compose.yml").exists() || root.join("docker-compose.yaml").exists() {
        kinds.push(ProjectKind::DockerCompose);
        if root.join("docker-compose.yml").exists() {
            notable("docker-compose.yml", &mut notable_files);
        } else {
            notable("docker-compose.yaml", &mut notable_files);
        }
        suggest("docker", "container runtime", None, &mut suggested_tools);
        suggest("docker compose", "manage multi-container apps", None, &mut suggested_tools);
    }

    // ── Kubernetes ────────────────────────────────────────────────────────
    if root.join("Chart.yaml").exists() {
        kinds.push(ProjectKind::Helm);
        notable("Chart.yaml", &mut notable_files);
        suggest("helm", "Kubernetes package manager", None, &mut suggested_tools);
        suggest("kubectl", "Kubernetes CLI", None, &mut suggested_tools);
    }
    if has_yaml_with_apiversion(&root) && !kinds.contains(&ProjectKind::Helm) {
        kinds.push(ProjectKind::Kubernetes);
        suggest("kubectl", "apply and manage Kubernetes resources", None, &mut suggested_tools);
    }

    // ── Terraform ─────────────────────────────────────────────────────────
    if has_extension(&root, "tf") {
        kinds.push(ProjectKind::Terraform);
        suggest("terraform", "provision infrastructure", None, &mut suggested_tools);
        suggest("tflint", "Terraform linter", Some("brew install tflint"), &mut suggested_tools);
    }

    // ── Nix ───────────────────────────────────────────────────────────────
    if root.join("flake.nix").exists() || root.join("default.nix").exists() {
        kinds.push(ProjectKind::Nix);
        if root.join("flake.nix").exists() { notable("flake.nix", &mut notable_files); }
        if root.join("default.nix").exists() { notable("default.nix", &mut notable_files); }
        if root.join("shell.nix").exists() { notable("shell.nix", &mut notable_files); }
        suggest("nix", "reproducible build system", None, &mut suggested_tools);
    }

    // ── Generic dotfiles ──────────────────────────────────────────────────
    for dotfile in &[".editorconfig", ".env", ".envrc", "Makefile", "justfile"] {
        if root.join(dotfile).exists() {
            notable(dotfile, &mut notable_files);
        }
    }
    if root.join("Makefile").exists() {
        suggest("make", "run project tasks defined in Makefile", None, &mut suggested_tools);
    }
    if root.join("justfile").exists() || root.join("Justfile").exists() {
        suggest("just", "command runner (justfile)", Some("cargo install just"), &mut suggested_tools);
    }

    // ── Git state ─────────────────────────────────────────────────────────
    let git = detect_git_state(&root);

    // Deduplicate suggested tools by name.
    let mut seen_tools = std::collections::HashSet::new();
    suggested_tools.retain(|t| seen_tools.insert(t.name.clone()));

    let summary = build_summary(&kinds, &git);

    ProjectContext {
        root,
        kinds,
        notable_files,
        git,
        suggested_tools,
        summary,
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn notable(name: &str, v: &mut Vec<String>) {
    if !v.contains(&name.to_string()) {
        v.push(name.to_string());
    }
}

fn suggest(name: &str, reason: &str, install_hint: Option<&str>, v: &mut Vec<SuggestedTool>) {
    v.push(SuggestedTool {
        name: name.to_string(),
        reason: reason.to_string(),
        install_hint: install_hint.map(|s| s.to_string()),
    });
}

fn detect_node_pm(root: &Path) -> String {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm".into()
    } else if root.join("yarn.lock").exists() {
        "yarn".into()
    } else if root.join("bun.lockb").exists() {
        "bun".into()
    } else {
        "npm".into()
    }
}

fn has_extension(dir: &Path, ext: &str) -> bool {
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

fn has_yaml_with_apiversion(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
                if ext != "yaml" && ext != "yml" {
                    return false;
                }
                std::fs::read_to_string(&p)
                    .map(|s| s.contains("apiVersion:"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn detect_git_state(root: &Path) -> GitState {
    // Walk up to find the .git directory.
    let mut cur = Some(root.to_path_buf());
    let mut repo_root: Option<PathBuf> = None;
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            repo_root = Some(dir.clone());
            break;
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }

    let Some(ref repo) = repo_root else {
        return GitState {
            is_git_repo: false,
            branch: None,
            remote_origin: None,
            repo_root: None,
        };
    };

    // Read branch from HEAD.
    let branch = std::fs::read_to_string(repo.join(".git").join("HEAD"))
        .ok()
        .and_then(|s| {
            let trimmed = s.trim();
            if let Some(b) = trimmed.strip_prefix("ref: refs/heads/") {
                Some(b.to_string())
            } else if trimmed.len() >= 7 {
                // Detached HEAD — show short hash.
                Some(format!("detached@{}", &trimmed[..7]))
            } else {
                None
            }
        });

    // Read origin URL.
    let remote_origin = std::fs::read_to_string(repo.join(".git").join("config"))
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
                if in_origin {
                    if let Some(url) = t.strip_prefix("url = ") {
                        return Some(url.to_string());
                    }
                }
            }
            None
        });

    GitState {
        is_git_repo: true,
        branch,
        remote_origin,
        repo_root,
    }
}

fn build_summary(kinds: &[ProjectKind], git: &GitState) -> String {
    let mut parts = Vec::new();
    if !kinds.is_empty() {
        let names: Vec<String> = kinds.iter().map(|k| k.to_string()).collect();
        parts.push(names.join(" + "));
    }
    if git.is_git_repo {
        if let Some(ref b) = git.branch {
            parts.push(format!("git:{b}"));
        } else {
            parts.push("git repo".into());
        }
    }
    if parts.is_empty() {
        "No project type detected".into()
    } else {
        parts.join(" | ")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir() -> PathBuf {
        let id = uuid::Uuid::new_v4().to_string();
        let dir = PathBuf::from(format!("/tmp/toolshub-ctx-test-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rust_project_detected() {
        let dir = tmp_dir();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let ctx = detect(&dir);
        assert!(ctx.kinds.contains(&ProjectKind::Rust));
        assert!(ctx.notable_files.contains(&"Cargo.toml".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_dir_has_no_project() {
        let dir = tmp_dir();
        let ctx = detect(&dir);
        assert!(ctx.kinds.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_with_pnpm_detected() {
        let dir = tmp_dir();
        fs::write(dir.join("package.json"), "{}").unwrap();
        fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();
        let ctx = detect(&dir);
        assert!(ctx.kinds.iter().any(|k| matches!(k, ProjectKind::NodeJs { package_manager } if package_manager == "pnpm")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn multi_stack_detected() {
        let dir = tmp_dir();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(dir.join("Dockerfile"), "FROM rust").unwrap();
        let ctx = detect(&dir);
        assert!(ctx.kinds.contains(&ProjectKind::Rust));
        assert!(ctx.kinds.contains(&ProjectKind::Docker));
        let _ = fs::remove_dir_all(&dir);
    }
}
