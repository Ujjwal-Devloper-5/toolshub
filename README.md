<div align="center">

```
████████╗ ██████╗  ██████╗ ██╗     ███████╗██╗  ██╗██╗   ██╗██████╗
   ██╔══╝██╔═══██╗██╔═══██╗██║     ██╔════╝██║  ██║██║   ██║██╔══██╗
   ██║   ██║   ██║██║   ██║██║     ███████╗███████║██║   ██║██████╔╝
   ██║   ██║   ██║██║   ██║██║     ╚════██║██╔══██║██║   ██║██╔══██╗
   ██║   ╚██████╔╝╚██████╔╝███████╗███████║██║  ██║╚██████╔╝██████╔╝
   ╚═╝    ╚═════╝  ╚═════╝ ╚══════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝
```

**The Intelligent CLI Operating Companion**

[![CI](https://github.com/toolshub/toolshub/actions/workflows/ci.yml/badge.svg)](https://github.com/toolshub/toolshub/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/toolshub-cli.svg)](https://crates.io/crates/toolshub-cli)

</div>

---

ToolsHub is a production-grade, AI-powered CLI assistant built in Rust that understands your tools, operating system, package managers, local repository context, and command safety — all from a single terminal application.

It is **not** a generic web chatbot. It is an intelligent, context-aware operating companion for developers.

---

## ✨ Features at a Glance

* 🧠 **AI-Powered Command Explanation**: Detailed breakdowns of tools (`explain rsync`) combining man pages, TLDR pages, and LLM intelligence.
* 📦 **Universal Package Management**: Auto-detects pacman, apt, brew, dnf, cargo, npm, pip, and 20+ other package managers.
* 🛡️ **Safety Checker Engine**: Analyzes and blocks destructive commands (`rm -rf /`, `mkfs`, raw drive writes, fork bombs).
* 💾 **Persistent Local Memory**: Stores facts, tool usage history, and doc cache in a privacy-first local SQLite database.
* 📂 **Project Context Sensing**: Auto-detects Git repository state, language stacks (Rust, Node, Python, Go, Docker, K8s, Terraform), and tools.
* 🖥️ **Interactive TUI Dashboard**: Full terminal user interface powered by `ratatui` and `crossterm`.

---

## 🚀 Installation

### Quick Install Script
Install the pre-compiled binary via curl:
```bash
curl -fsSL https://toolshub.dev/install.sh | bash
```

### Install with Cargo
Install from Crates.io:
```bash
cargo install toolshub-cli
```

### Build from Source
Clone the workspace repository and build with Cargo:
```bash
git clone https://github.com/toolshub/toolshub.git
cd toolshub
cargo build --release --workspace
./target/release/toolshub --version
```

---

## 💻 Subcommands & Usage Guide

ToolsHub provides a comprehensive set of subcommands for system management, tool discovery, AI querying, and documentation:

### 1. `explain` (alias: `ex`)
Generates deep, cited documentation for any terminal tool or command:
```bash
# Explain a command in comprehensive detail
toolshub explain rsync

# Show a brief summary only
toolshub explain tar --brief
```

### 2. `ask` (alias: `q`)
Ask the AI assistant natural language questions about your environment or commands:
```bash
toolshub ask how do I monitor active TCP connections on Linux?
```

### 3. `install` (alias: `i`, `add`) & `remove` (alias: `rm`, `uninstall`)
Install or remove software packages across any detected system package manager:
```bash
# Auto-detect package manager and install ripgrep
toolshub install ripgrep

# Force a specific package manager (e.g. Homebrew or Cargo)
toolshub install bat -m brew

# Skip confirmation prompts
toolshub install htop -y

# Remove a package
toolshub remove htop
```

### 4. `search` (alias: `find`)
Search packages across all available system package managers:
```bash
toolshub search postgresql
```

### 5. `tools`
Discover and list all executable binaries found in your system `$PATH`:
```bash
# List all executable tools
toolshub tools

# Filter tools by name substring
toolshub tools --filter git

# Show compact listing
toolshub tools --compact
```

### 6. `context`
Inspect directory context, Git status, and project language stacks:
```bash
# Inspect current working directory
toolshub context

# Inspect a specific path
toolshub context ./my-project
```

### 7. `env`
Display detected host operating system, active shell, hardware specs, and package managers:
```bash
# Full environment report
toolshub env

# Show only package managers or hardware
toolshub env --packages
toolshub env --hardware
```

### 8. `run`
Execute shell commands with pre-execution safety verification:
```bash
toolshub run "sudo rm -rf /var/tmp/old-logs"
```

### 9. `services`
Inspect and manage systemd services (Linux systems):
```bash
# List systemd services
toolshub services

# Show only failed services
toolshub services --failed
```

### 10. `config`
Inspect and modify ToolsHub configuration options:
```bash
# Show current active configuration
toolshub config show

# Display configuration file path
toolshub config path

# Edit configuration in $EDITOR
toolshub config edit

# Set a configuration property
toolshub config set ai.provider ollama

# Reset configuration to defaults
toolshub config reset
```

### 11. `memory`
Inspect and manage the local SQLite AI memory database:
```bash
# Show stored facts
toolshub memory show

# Show conversation summaries
toolshub memory summaries

# Show cached documentation entries
toolshub memory cache

# Show command execution history
toolshub memory history

# Clear all stored memory (irreversible)
toolshub memory clear
```

### 12. `doctor`
Run diagnostic checkups on your ToolsHub installation:
```bash
toolshub doctor
```

### 13. `dashboard` (alias: `ui`, `tui`)
Launch the interactive terminal user interface dashboard:
```bash
toolshub dashboard
```

---

## 🏗️ Architecture

ToolsHub is engineered as a modular Cargo workspace comprised of 10 specialized crates:

| Crate | Functionality |
| :--- | :--- |
| **`toolshub-core`** | Fundamental types, error models, and TOML configuration loaders. |
| **`toolshub-env`** | OS distribution, shell, hardware, and binary discovery probes. |
| **`toolshub-memory`** | SQLite persistent memory storage for facts, history, and doc cache. |
| **`toolshub-ai`** | Multi-provider LLM abstraction layer (Ollama, OpenAI, Anthropic, OpenRouter). |
| **`toolshub-package`** | Package manager abstraction and unified command generator for 20+ PMs. |
| **`toolshub-doc`** | Aggregator and renderer for man pages, TLDR entries, and AI docs. |
| **`toolshub-context`** | Directory context, Git repository status, and language stack analyzer. |
| **`toolshub-safety`** | Dangerous command pattern analyzer and risk score evaluator. |
| **`toolshub-tui`** | Terminal UI dashboard components built with `ratatui`. |
| **`toolshub-cli`** | Main CLI application binary, clap parser, and subcommand dispatcher. |

For an in-depth architectural breakdown, data flow diagrams, and module responsibilities, read [ARCHITECTURE.md](ARCHITECTURE.md).

---

## ⚙️ Configuration & Environment Variables

ToolsHub configuration is stored at `~/.config/toolshub/config.toml`. Environment variables can be configured directly or loaded via a `.env` file in your execution directory.

Refer to [.env.example](.env.example) and [.env](.env) for full configuration templates:

```env
# AI Subsystem
TOOLSHUB_AI_PROVIDER=ollama
TOOLSHUB_AI_MODEL=llama3.2
TOOLSHUB_AI_ENDPOINT=http://localhost:11434
TOOLSHUB_AI_API_KEY=
TOOLSHUB_AI_MAX_TOKENS=4096
TOOLSHUB_AI_TEMPERATURE=0.7

# Memory Subsystem
TOOLSHUB_MEMORY_ENABLED=true
TOOLSHUB_DB_PATH=~/.local/share/toolshub/memory.db

# System Logging & Display
RUST_LOG=info
EDITOR=nano
NO_COLOR=0
```

---

## 🤝 Contributing

We welcome community contributions! Please review [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, build and test requirements (`cargo check`, `cargo test`), coding standards, and pull request procedures.

---

## 📜 License

ToolsHub is released under the [Apache-2.0 License](LICENSE).
