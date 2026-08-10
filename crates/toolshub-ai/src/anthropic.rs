//! Anthropic Claude provider.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use toolshub_core::{Error, Result};

use crate::provider::{AiProvider, CompletionRequest, CompletionResponse, Role};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    /// System prompt extracted from the messages list, sent as a top-level field.
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<AnthropicMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    model: Option<String>,
    content: Vec<AnthropicContent>,
    usage: Option<AnthropicUsage>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    kind: Option<String>,
    error: Option<AnthropicErrorBody>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorBody {
    message: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

// ── Provider ──────────────────────────────────────────────────────────────────

const ANTHROPIC_DEFAULT_ENDPOINT: &str = "https://api.anthropic.com";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";

/// AI provider that speaks the Anthropic Messages API.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    /// Base URL without trailing slash.
    endpoint: String,
}

impl AnthropicProvider {
    /// Construct a new provider.
    ///
    /// `endpoint` defaults to `https://api.anthropic.com` when `None`.
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        endpoint: Option<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            endpoint: endpoint.unwrap_or_else(|| ANTHROPIC_DEFAULT_ENDPOINT.into()),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    #[instrument(skip(self, req), fields(provider = "anthropic", model = %self.model))]
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/v1/messages", self.endpoint);

        // Anthropic separates the system prompt from the conversation turns.
        let mut system_prompt: Option<&str> = None;
        let mut turns: Vec<AnthropicMessage> = Vec::with_capacity(req.messages.len());

        for msg in &req.messages {
            match msg.role {
                Role::System => {
                    system_prompt = Some(&msg.content);
                }
                Role::User => turns.push(AnthropicMessage {
                    role: "user",
                    content: &msg.content,
                }),
                Role::Assistant => turns.push(AnthropicMessage {
                    role: "assistant",
                    content: &msg.content,
                }),
            }
        }

        let body = AnthropicRequest {
            model: &self.model,
            system: system_prompt,
            messages: turns,
            max_tokens: req.max_tokens,
            // Anthropic caps temperature at 1.0; clamp to avoid a 400 error.
            temperature: req.temperature.clamp(0.0, 1.0),
        };

        debug!("POST {url}");

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::http(format!("anthropic request failed: {e}")))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::ai_provider(
                "Anthropic: invalid or missing API key (HTTP 401)",
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::ai_provider(
                "Anthropic: rate limit exceeded (HTTP 429) – please retry later",
            ));
        }

        // Parse body regardless of status to surface structured error messages.
        let raw = resp
            .text()
            .await
            .map_err(|e| Error::http(format!("anthropic: failed to read body: {e}")))?;

        let anth_resp: AnthropicResponse = serde_json::from_str(&raw).map_err(|e| {
            Error::ai_provider(format!(
                "Anthropic: failed to parse response (HTTP {status}): {e}\nBody: {raw}"
            ))
        })?;

        // Surface API-level errors returned with 2xx or 4xx.
        if let Some(err) = anth_resp.error {
            let kind = err.kind.as_deref().unwrap_or("unknown");
            return Err(Error::ai_provider(format!(
                "Anthropic error [{kind}]: {}",
                err.message
            )));
        }

        if !status.is_success() {
            return Err(Error::ai_provider(format!(
                "Anthropic HTTP {status}: {raw}"
            )));
        }

        // Extract the first text block.
        let content = anth_resp
            .content
            .into_iter()
            .filter(|c| c.kind == "text")
            .find_map(|c| c.text)
            .ok_or_else(|| {
                Error::ai_provider("Anthropic: response contained no text content blocks")
            })?;

        let usage = anth_resp.usage.unwrap_or(AnthropicUsage {
            input_tokens: 0,
            output_tokens: 0,
        });

        Ok(CompletionResponse {
            content,
            model: anth_resp.model.unwrap_or_else(|| self.model.clone()),
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
        })
    }

    async fn is_available(&self) -> bool {
        // Anthropic has no cheap ping endpoint; attempt a minimal messages request.
        let url = format!("{}/v1/messages", self.endpoint);
        match self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "messages": [{"role": "user", "content": "ping"}],
                "max_tokens": 1
            }))
            .send()
            .await
        {
            Ok(r) => {
                // 200 = available, 401 = key problem, 4xx are still "reachable"
                let available = r.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE;
                if !available {
                    warn!(status = %r.status(), "Anthropic availability check: service unavailable");
                }
                available
            }
            Err(e) => {
                warn!("Anthropic availability check error: {e}");
                false
            }
        }
    }
}
