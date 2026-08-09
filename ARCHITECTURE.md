# ToolsHub Architecture

ToolsHub is designed as a modular, production-grade CLI operating companion written in Rust. It combines deep system detection, multi-package management, persistent SQLite memory, offline/online AI model abstraction, command safety verification, and interactive terminal interface components across a 10-crate Cargo workspace.

---

## 1. System Context

ToolsHub sits between the developer, the operating system kernel/utilities, package managers, local database storage, and AI inference backends (local Ollama or remote LLM providers).

```
 ┌──────────────────────────────────────────────────────────────────────────────┐
 │                                   USER                                       │
 └──────────────────────────────────────┬───────────────────────────────────────┘
                                        │ Command Execution / CLI Flags
                                        ▼
 ┌──────────────────────────────────────────────────────────────────────────────┐
 │                             TOOLSHUB (toolshub-cli)                          │
 ├───────────────────┬──────────────────┬───────────────────┬───────────────────┤
 │  System Sensing   │ Package Engine   │ AI Engine         │ Security & Memory │
 │  (env / context)  │ (pacman/apt/etc) │ (Ollama/OpenAI)   │ (safety / sqlite) │
 └─────────┬─────────┴────────┬─────────┴─────────┬─────────┴─────────┬─────────┘
           │                  │                   │                   │
           ▼                  ▼                   ▼                   ▼
    ┌─────────────┐   ┌──────────────┐    ┌───────────────┐   ┌──────────────┐
    │ Host OS &   │   │  System PMs  │    │  AI Providers │   │ Local SQLite │
    │ Shell/Proc  │   │  & Binaries  │    │ (Ollama/Cloud)│   │ (~/.local)   │
    └─────────────┘   └──────────────┘    └───────────────┘   └──────────────┘
```

---

## 2. Component Architecture & Crate Roles

The project is structured as a Cargo workspace containing 10 specialized, loosely coupled crates:

```
crates/
├── toolshub-core      ───── Base types, errors, result type, system configuration
├── toolshub-env       ───── OS, shell, hardware, and binary executable discovery
├── toolshub-memory    ───── SQLite database storage, facts, tool history, cache
├── toolshub-ai        ───── Multi-provider LLM client (Ollama, OpenAI, Anthropic)
├── toolshub-package   ───── Multi-package manager abstraction & command generator
├── toolshub-doc       ───── Man page & TLDR documentation parser and renderer
├── toolshub-context   ───── Directory / repository inspection & language stack detection
├── toolshub-safety    ───── Dangerous command analysis & risk assessment engine
├── toolshub-tui       ───── Ratatui interactive dashboard & terminal UI screens
└── toolshub-cli       ───── Binary CLI entrypoint (clap parser, subcommands, dispatch)
```

### Module Breakdown

| Crate | Primary Responsibilities | Key Dependencies |
| :--- | :--- | :--- |
| **`toolshub-core`** | Defines common data types (`OperatingSystem`, `HardwareInfo`, `InstalledTool`, `DangerLevel`), application `Error` enum, and configuration loader/saver (`Config`, `AiConfig`, `MemoryConfig`, `PackageConfig`, `SafetyConfig`, `DisplayConfig`). | `serde`, `toml`, `dirs`, `thiserror`, `uuid` |
| **`toolshub-env`** | Probes the operating system environment asynchronously. Captures OS distribution, active shell (`/proc` or env vars), hardware (CPU cores, RAM size, GPU), package manager availability, and scans `$PATH` directories to build `EnvironmentSnapshot`. | `tokio`, `chrono`, `dirs`, `tracing`, `toolshub-core` |
| **`toolshub-memory`** | Manages persistent local SQLite storage. Handles fact storage, conversation summaries, tool usage frequency history, command log, and documentation cache with TTL expiration. | `sqlx` (sqlite), `tokio`, `chrono`, `toolshub-core` |
| **`toolshub-ai`** | Provides unified async interface for LLM operations. Supports local Ollama API, OpenAI REST API, Anthropic Messages API, and OpenRouter gateway. Handles prompt construction, context streaming, and fallback. | `reqwest`, `serde_json`, `tokio`, `async-trait`, `toolshub-core` |
| **`toolshub-package`** | Detects native package managers across Linux distros, macOS (Homebrew), and Windows (Winget, Scoop, Choco). Generates install/uninstall/search commands for 20+ managers (pacman, apt, dnf, cargo, npm, etc.). | `tokio`, `which`, `toolshub-core` |
| **`toolshub-doc`** | Aggregates and parses documentation from `man` pages, TLDR pages, `--help` command output, and offline AI summaries. Strips ANSI color codes and formats markdown output. | `regex`, `tokio`, `toolshub-core` |
| **`toolshub-context`** | Inspects target directory for Git repository status, active code stack (Rust Cargo, Node.js package.json, Python pyproject.toml/requirements.txt, Go go.mod, Docker, Kubernetes, Terraform), and builds `ProjectContext`. | `tokio`, `serde_json`, `toml`, `toolshub-core` |
| **`toolshub-safety`** | Analyzes input commands against rule heuristics for dangerous operations (destructive `rm -rf`, disk format `mkfs`, raw drive writes `dd`, pipe-to-shell scripts, fork bombs). Assigns `DangerLevel` (Low, Medium, High, Critical). | `regex`, `toolshub-core` |
| **`toolshub-tui`** | Implements ratatui-based terminal dashboard interface for visual environment browsing, command search, memory inspection, and interactive AI chat. | `ratatui`, `crossterm`, `tokio`, `toolshub-core` |
| **`toolshub-cli`** | The main binary crate (`toolshub`). Defines CLI subcommands via `clap` (`ask`, `explain`, `install`, `remove`, `search`, `tools`, `context`, `env`, `run`, `services`, `config`, `memory`, `doctor`, `dashboard`), initializes logging, and executes commands. | `clap`, `tokio`, `tracing-subscriber`, all sub-crates |

---

## 3. Data Flow Pipeline

When a user invokes a command such as `toolshub ask "how do I optimize my Cargo build?"` or `toolshub explain rsync`, ToolsHub executes the following sequence:

```
[1. User Command Input]
          │
          ▼
[2. CLI Flag & Arg Parsing] ─── clap matches subcommand & flags (--no-ai, --profile, --json)
          │
          ▼
[3. Config Stack Resolution] ── TOML file + Environment Variables (.env) merged
          │
          ▼
[4. Environment & Context Sensing] ── (Parallel tokio tasks: OS, Shell, Hardware, Project Context)
          │
          ▼
[5. Safety & Risk Validation] ── Evaluate command against toolshub-safety rules
          │
          ├──> Danger >= Threshold? ──> Block / Prompt Confirmation
          │
          ▼
[6. Memory & Knowledge Retrieval] ── Query SQLite for past interactions, cached docs, facts
          │
          ▼
[7. Provider Request / Synthesis] ── Format prompt with system context & send to AI provider
          │
          ▼
[8. Result Caching & Memory Storage] ─ Save fact / command history to SQLite database
          │
          ▼
[9. Output Rendering] ──────────── Format markdown / terminal widgets / JSON output to stdout
```

---

## 4. Configuration Stack & Precedence

ToolsHub evaluates configuration using a four-tier precedence hierarchy (highest priority wins):

```
       ┌─────────────────────────────────────────────────┐  (HIGHEST)
       │  1. Command-Line Flags                          │
       │     (--no-ai, --no-color, --profile, -v)       │
       ├─────────────────────────────────────────────────┤
       │  2. Environment Variables                       │
       │     (TOOLSHUB_AI_PROVIDER, RUST_LOG, .env)     │
       ├─────────────────────────────────────────────────┤
       │  3. User Configuration File                     │
       │     (~/.config/toolshub/config.toml)           │
       ├─────────────────────────────────────────────────┤
       │  4. Built-in Programmatic Defaults              │
       │     (Ollama local, llama3.2, dark theme)        │
       └─────────────────────────────────────────────────┘  (LOWEST)
```

### Environment Variable Mapping

Environment variables (loaded automatically from `.env` or system shell) map directly to configuration fields:

| Environment Variable | Config Target | Default / Fallback |
| :--- | :--- | :--- |
| `TOOLSHUB_AI_PROVIDER` | `ai.provider` | `"ollama"` |
| `TOOLSHUB_AI_MODEL` | `ai.model` | `"llama3.2"` |
| `TOOLSHUB_AI_ENDPOINT` | `ai.endpoint` | `"http://localhost:11434"` |
| `TOOLSHUB_AI_API_KEY` | `ai.api_key` | `None` |
| `TOOLSHUB_AI_MAX_TOKENS` | `ai.max_tokens` | `4096` |
| `TOOLSHUB_AI_TEMPERATURE` | `ai.temperature` | `0.7` |
| `TOOLSHUB_MEMORY_ENABLED` | `memory.enabled` | `true` |
| `TOOLSHUB_DB_PATH` | `memory.db_path` | `~/.local/share/toolshub/memory.db` |
| `RUST_LOG` | `tracing` log filter | `"info"` |
| `EDITOR` | External editor launcher | `"nano"` |
| `NO_COLOR` | Color override flag | `false` |

---

## 5. Security & Safety Layer

ToolsHub includes a safety subsystem (`toolshub-safety`) designed to protect system integrity before executing or recommending shell commands.

### Danger Level Classification

| Level | Description | Example Commands | Action Taken |
| :--- | :--- | :--- | :--- |
| **Low** | Read-only operations, queries, or safe local commands. | `ls`, `grep`, `cargo check`, `cat` | Allowed immediately. |
| **Medium** | Operations modifying user directory, system state, or installing packages. | `apt install`, `git push`, `rm old_file.txt` | Allowed with standard warning if verbose. |
| **High** | Privileged system modification, package removal, network pipe to shell. | `sudo systemctl restart`, `curl ... \| bash` | Requires explicit user confirmation prompt (`-y` bypass). |
| **Critical** | Potentially unrecoverable disk, filesystem, or kernel operations. | `rm -rf /`, `mkfs.ext4`, `dd if=... of=/dev/sda`, fork bomb `:(){ :|:& };:` | Blocked by default unless forced. |

### Safety Verification Process

1. **Pattern Recognition**: Regex and AST-like tokenizers scan input string for dangerous flags (`-rf /`, `of=/dev/`), destructive tools (`mkfs`, `fdisk`), or unverified remote execution scripts (`curl | sh`).
2. **Threshold Evaluation**: Compares calculated danger score against `safety.danger_level_threshold` (configured in TOML or `.env`).
3. **Execution Interception**: If danger score exceeds threshold, `toolshub-cli` prompts the user with detailed risk explanation before invoking bash/sh command.
