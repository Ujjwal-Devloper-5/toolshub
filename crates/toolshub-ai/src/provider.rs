//! Core AI provider trait and shared request/response types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use toolshub_core::Result;

// ── Enums & structs ───────────────────────────────────────────────────────────

/// The role of a participant in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in a conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Parameters for a single text-completion request.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// Full conversation history, including the latest user message.
    pub messages: Vec<Message>,
    /// Hard cap on the number of tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
    /// Whether to stream the response token-by-token.
    pub stream: bool,
}

impl Default for CompletionRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            max_tokens: 2048,
            temperature: 0.7,
            stream: false,
        }
    }
}

/// The result of a completed text-generation request.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The generated text content.
    pub content: String,
    /// The model identifier returned by the provider.
    pub model: String,
    /// Number of tokens in the prompt (0 if unavailable).
    pub prompt_tokens: u32,
    /// Number of tokens generated (0 if unavailable).
    pub completion_tokens: u32,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// A pluggable AI backend capable of generating text completions.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Human-readable provider name (e.g. `"openai"`, `"anthropic"`, `"ollama"`).
    fn name(&self) -> &str;

    /// The model currently in use.
    fn model(&self) -> &str;

    /// Whether this provider supports streaming responses.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Submit a completion request and wait for the full response.
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;

    /// Stream a completion request, printing tokens to stdout as they arrive.
    ///
    /// The default implementation falls back to `complete()` so providers
    /// that don't implement streaming still work correctly.
    ///
    /// Providers that override this (like `OpenAiProvider`) get live token
    /// output — each token is printed immediately as it arrives, making the
    /// experience feel instant rather than waiting for the full response.
    async fn stream_to_stdout(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        // Default: fall back to blocking complete() — no streaming side-effects.
        self.complete(req).await
    }

    /// Return `true` if the provider can currently be reached.
    async fn is_available(&self) -> bool;
}
