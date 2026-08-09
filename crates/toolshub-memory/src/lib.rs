//! # toolshub-memory
//!
//! Persistent SQLite-backed memory store for ToolsHub.
//!
//! Provides [`MemoryStore`] for:
//! - Caching AI-generated doc explanations.
//! - Storing structured key/value facts about the user's environment.
//! - Recording tool invocation history.
//! - Keeping compressed AI conversation summaries.

pub mod store;

pub use store::{
    ConversationSummary, DocCacheEntry, Fact, MemoryStats, MemoryStore, ToolUsage,
};
