//! `toolshub-core` — foundational types, errors, and configuration for ToolsHub.
//!
//! This crate has **no internal workspace dependencies** and is the
//! building block upon which every other ToolsHub crate is built.
//!
//! # Module Layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`error`] | Unified error enum and [`Result`] alias |
//! | [`types`] | Domain types: OS, shell, package managers, etc. |
//! | [`config`] | Application configuration (load / save from TOML) |

pub mod config;
pub mod error;
pub mod types;

pub use error::{Error, Result};
