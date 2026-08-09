# Project: ToolsHub GitHub Actions CI Pipeline

## Architecture
- Target CI system: GitHub Actions
- Trigger branch: `master` (`push` and `pull_request`)
- Rust Toolchain: `stable` via `dtolnay/rust-toolchain@stable` (with clippy component)
- Dependency Caching: `Swatinem/rust-cache@v2`
- Offline SQLx: `SQLX_OFFLINE: "true"` environment variable for all cargo steps
- Codebase scope: Rust Edition 2021, 10 workspace crates (`toolshub-core`, `toolshub-env`, `toolshub-memory`, `toolshub-ai`, `toolshub-package`, `toolshub-doc`, `toolshub-context`, `toolshub-safety`, `toolshub-tui`, `toolshub-cli`)

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | GitHub Actions Workflow | Create `.github/workflows/ci.yml` triggering on push & pull_request for master | M1 | ORIGINAL_REQUEST §1 |
| 2 | Rust Toolchain & Caching | Configure stable Rust toolchain and Swatinem/rust-cache@v2 | M1 | ORIGINAL_REQUEST §2 |
| 3 | SQLx Offline Env | Set `SQLX_OFFLINE=true` in top-level env for cargo steps | M1 | ORIGINAL_REQUEST §3 |
| 4 | Cargo Check & Test | Run `cargo check --workspace` and `cargo test --workspace` | M1 | ORIGINAL_REQUEST §2 |
| 5 | Cargo Clippy Verification | Run `cargo clippy --workspace -- -D warnings` | M1 | ORIGINAL_REQUEST §2 |
| 6 | Clippy Warning Remediation | Fix pre-existing clippy warnings in `toolshub-tui` and `toolshub-core` | M1 | Survey |
| 7 | Git Version Control | Validate YAML syntax, stage `.github/workflows/ci.yml` and commit to git | M1 | ORIGINAL_REQUEST §4 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | CI Workflow & Lint Remediation | Create `.github/workflows/ci.yml`, resolve workspace clippy warnings, verify check/test/clippy with SQLX_OFFLINE=true, stage & commit | none | IN_PROGRESS |

## Code Layout
- `.github/workflows/ci.yml`: GitHub Actions CI pipeline configuration
- `Cargo.toml`: Root cargo workspace definition
- `crates/toolshub-core/src/config.rs`: Core crate config logic (clippy lint remediation)
- `crates/toolshub-tui/src/lib.rs`: TUI crate entry/library logic (clippy lint remediation)
- `.sqlx/`: SQLx prepared query metadata directory
