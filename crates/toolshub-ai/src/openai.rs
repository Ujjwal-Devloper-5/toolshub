//! OpenAI (and OpenAI-compatible) chat-completion provider.

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
            stream: false, // streaming not handled in complete()
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
