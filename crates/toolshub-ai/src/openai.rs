//! OpenAI (and OpenAI-compatible) chat-completion provider.
//!
//! Supports both the standard blocking `complete()` call and a live
//! token-streaming path via `complete_stream()`, which uses SSE to print
//! tokens to stdout as they arrive.

use std::io::Write;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use toolshub_core::{Error, Result};

use crate::provider::{AiProvider, CompletionRequest, CompletionResponse, Role};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    model: Option<String>,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

// ── SSE / streaming wire types ────────────────────────────────────────────────

/// One chunk from a streaming response (`data: {...}`).
#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    /// Present on the final chunk that carries usage info.
    usage: Option<OpenAiUsage>,
    /// Model name may appear on the first chunk.
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    error: OpenAiErrorBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorBody {
    message: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

// ── Provider ──────────────────────────────────────────────────────────────────

/// AI provider that speaks the OpenAI chat-completion API.
///
/// Compatible with OpenAI itself, Azure OpenAI, OpenRouter, and any other
/// service that implements the `/v1/chat/completions` endpoint.
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    /// Base URL without trailing slash (e.g. `https://api.openai.com`).
    endpoint: String,
}

impl OpenAiProvider {
    /// Construct a new provider.
    ///
    /// `endpoint` defaults to `https://api.openai.com` when `None`.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, endpoint: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            endpoint: endpoint.unwrap_or_else(|| "https://api.openai.com".into()),
        }
    }

    fn map_role(role: &Role) -> &'static str {
        match role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }

    /// Stream a completion request, printing tokens to stdout as they arrive.
    ///
    /// Returns the fully assembled `CompletionResponse` once the stream ends.
    pub async fn complete_stream(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        use futures::StreamExt;

        let url = format!("{}/v1/chat/completions", self.endpoint);

        let messages: Vec<OpenAiMessage> = req
            .messages
            .iter()
            .map(|m| OpenAiMessage {
                role: Self::map_role(&m.role),
                content: &m.content,
            })
            .collect();

        let body = OpenAiRequest {
            model: &self.model,
            messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            stream: true,
        };

        debug!("POST {url} (streaming)");

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::http(format!("openai stream request failed: {e}")))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::ai_provider(
                "OpenAI: invalid or missing API key (HTTP 401)",
            ));
        }

        if !status.is_success() {
            let raw = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".into());

            if let Ok(err) = serde_json::from_str::<OpenAiError>(&raw) {
                let kind = err.error.kind.as_deref().unwrap_or("unknown");
                return Err(Error::ai_provider(format!(
                    "OpenAI error [{kind}]: {}",
                    err.error.message
                )));
            }

            return Err(Error::ai_provider(format!("OpenAI HTTP {status}: {raw}")));
        }

        // Walk the SSE stream, collecting tokens.
        let mut full_content = String::new();
        let mut model_name = self.model.clone();
        let mut prompt_tokens = 0u32;
        let mut completion_tokens = 0u32;

        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result
                .map_err(|e| Error::http(format!("stream read error: {e}")))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process all complete lines in the buffer.
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() || line == "data: [DONE]" {
                    continue;
                }

                let data = line.strip_prefix("data: ").unwrap_or(&line);

                if let Ok(chunk_json) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                    if let Some(m) = chunk_json.model {
                        model_name = m;
                    }
                    if let Some(usage) = chunk_json.usage {
                        prompt_tokens = usage.prompt_tokens;
                        completion_tokens = usage.completion_tokens;
                    }
                    for choice in chunk_json.choices {
                        if choice.finish_reason.is_some() {
                            continue;
                        }
                        if let Some(token) = choice.delta.content {
                            full_content.push_str(&token);
                            // Print each token immediately — this is what makes
                            // the output feel live and responsive.
                            print!("{token}");
                            let _ = std::io::stdout().flush();
                        }
                    }
                }
            }
        }

        // Ensure the terminal moves past the streamed content.
        println!();

        Ok(CompletionResponse {
            content: full_content,
            model: model_name,
            prompt_tokens,
            completion_tokens,
        })
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    /// Stream tokens directly to stdout as they arrive.
    async fn stream_to_stdout(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        self.complete_stream(req).await
    }

    #[instrument(skip(self, req), fields(provider = "openai", model = %self.model))]
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/v1/chat/completions", self.endpoint);

        let messages: Vec<OpenAiMessage> = req
            .messages
            .iter()
            .map(|m| OpenAiMessage {
                role: Self::map_role(&m.role),
                content: &m.content,
            })
            .collect();

        let body = OpenAiRequest {
            model: &self.model,
            messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            stream: false,
        };

        debug!("POST {url}");

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::http(format!("openai request failed: {e}")))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::ai_provider(
                "OpenAI: invalid or missing API key (HTTP 401)",
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::ai_provider(
                "OpenAI: rate limit exceeded (HTTP 429) – please retry later",
            ));
        }

        if !status.is_success() {
            // Try to extract the structured error message from the body.
            let raw = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".into());

            if let Ok(err) = serde_json::from_str::<OpenAiError>(&raw) {
                let kind = err.error.kind.as_deref().unwrap_or("unknown");
                return Err(Error::ai_provider(format!(
                    "OpenAI error [{kind}]: {}",
                    err.error.message
                )));
            }

            return Err(Error::ai_provider(format!(
                "OpenAI HTTP {status}: {raw}"
            )));
        }

        let oai_resp: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| Error::ai_provider(format!("OpenAI: failed to parse response: {e}")))?;

        let choice = oai_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::ai_provider("OpenAI: response contained no choices"))?;

        let usage = oai_resp.usage.unwrap_or(OpenAiUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
        });

        Ok(CompletionResponse {
            content: choice.message.content,
            model: oai_resp.model.unwrap_or_else(|| self.model.clone()),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        })
    }

    async fn is_available(&self) -> bool {
        // A lightweight models-list request confirms connectivity and auth.
        let url = format!("{}/v1/models", self.endpoint);
        match self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
        {
            Ok(r) => {
                let ok = r.status().is_success();
                if !ok {
                    warn!(status = %r.status(), "OpenAI availability check failed");
                }
                ok
            }
            Err(e) => {
                warn!("OpenAI availability check error: {e}");
                false
            }
        }
    }
}
