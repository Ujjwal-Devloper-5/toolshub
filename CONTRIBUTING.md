# Contributing to ToolsHub

Thank you for your interest in contributing to ToolsHub! ToolsHub is an open-source, intelligent CLI operating companion built in Rust. We welcome contributions of all kinds: bug reports, documentation improvements, new package manager integrations, AI provider backends, performance optimizations, and new CLI capabilities.

---

## 1. Development Prerequisites

Before building ToolsHub, ensure you have the following installed on your host system:

* **Rust Toolchain**: Rust 1.75 or higher (with `cargo`, `rustc`, and `rustfmt`)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
* **Build Essentials**: C compiler (`gcc` or `clang`) and standard build tools for compiling SQLite dependencies.
* **Git**: Version control system.

---

## 2. Workspace Structure

ToolsHub is organized as a Cargo workspace with 10 specialized crates under `crates/`:

```
toolshub/
├── Cargo.toml                 # Workspace manifest & shared dependencies
├── .env.example               # Template environment configuration
├── ARCHITECTURE.md            # Deep system architecture documentation
├── CONTRIBUTING.md            # Contributor guidelines (this file)
├── README.md                  # Project overview & usage guide
└── crates/
    ├── toolshub-core          # Base types, error models, config management
    ├── toolshub-env           # System sensing & hardware/OS/shell discovery
    ├── toolshub-memory        # SQLite persistent state & cache engine
    ├── toolshub-ai            # Multi-provider LLM abstraction layer
    ├── toolshub-package       # Package manager abstraction (pacman, apt, brew, cargo, etc.)
    ├── toolshub-doc           # Man page & TLDR document renderer
    ├── toolshub-context       # Project directory context & stack analyzer
    ├── toolshub-safety        # Dangerous command verification & risk engine
    ├── toolshub-tui           # Ratatui terminal dashboard interface
    └── toolshub-cli           # Main binary CLI application & clap subcommands
```

When adding new features, place the functionality in the appropriate crate rather than inflating `toolshub-cli`.

---

## 3. Building and Testing

We enforce strict compilation and test pass criteria across the workspace.

### Quick Build Check
Validate that all workspace crates compile cleanly:
```bash
cargo check --workspace
```

### Run Unit and Integration Tests
Execute all workspace tests:
```bash
cargo test --workspace
```

### Run Release Build
Build optimized release binaries:
```bash
cargo build --release --workspace
```

### Linting and Code Formatting
Ensure code formatting and clippy lints pass:
```bash
# Check formatting
cargo fmt --check

# Check lints
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 4. Coding Standards & Guidelines

### Integrity Mandate
* **No Cheating or Hardcoding**: All implementations must be genuine. Do NOT hardcode test results, fake verification outputs, or create stub facade implementations that return pre-calculated responses without real logic.
* **Real State Management**: Features must maintain real data state (e.g. SQLite database, true environment detection, actual LLM requests).

### Error Handling
* **Library Crates**: Use `thiserror` in `toolshub-core` and sub-crates to define domain-specific, strongly typed error enums. Avoid returning untyped strings.
* **CLI Crate**: Use `anyhow::Result` or propagate `toolshub_core::Error` cleanly to top-level `main()` dispatch with helpful error messages.

### Async Concurrency
* Use `tokio` for async runtime execution.
* Ensure I/O operations (file reading, command execution, network requests, SQLite queries) are non-blocking.
* Use `tokio::join!` or `tokio::spawn` when capturing environment probes concurrently.

### Documentation
* All public types, functions, structs, and enums must include doc comments (`///`).
* Each crate root (`src/lib.rs` or `src/main.rs`) must contain a module-level doc comment (`//!`) explaining its purpose and providing usage examples.

---

## 5. Pull Request Workflow

1. **Fork & Branch**: Create a feature branch off `main` with a descriptive name:
   ```bash
   git checkout -b feature/add-nix-package-manager
   # or
   git checkout -b fix/sqlite-memory-migration
   ```

2. **Implement & Test**: Write your code changes and add corresponding unit tests in the affected crate.

3. **Verify Locally**: Run the verification suite before opening a PR:
   ```bash
   cargo check --workspace
   cargo test --workspace
   cargo fmt --check
   ```

4. **Commit Messages**: Follow standard conventional commit guidelines:
   - `feat(package): add support for nix-env package manager`
   - `fix(ai): handle HTTP timeout gracefully in Ollama backend`
   - `docs(readme): update subcommand examples`

5. **Submit PR**: Open a Pull Request against `main`. Ensure all CI checks pass.
