//! # toolshub-ai
//!
//! Pluggable AI-provider abstraction for toolshub.  Supports OpenAI-compatible
//! APIs, Anthropic, and Ollama, with an [`AiRouter`] that selects the
//! right backend from application configuration.

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod provider;
pub mod router;

pub use provider::{AiProvider, CompletionRequest, CompletionResponse, Message, Role};
pub use router::AiRouter;
