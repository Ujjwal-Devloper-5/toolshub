//! Ollama local model provider.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use toolshub_core::{Error, Result};

use crate::provider::{AiProvider, CompletionRequest, CompletionResponse, Message, Role};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    num_predict: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: Option<String>,
    message: OllamaResponseMessage,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

// ── Provider ──────────────────────────────────────────────────────────────────

const OLLAMA_DEFAULT_ENDPOINT: &str = "http://localhost:11434";

/// AI provider that delegates to a locally-running Ollama instance.
pub struct OllamaProvider {
    client: reqwest::Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    /// Construct a new provider.
    ///
    /// `endpoint` defaults to `http://localhost:11434` when `None`.
    pub fn new(model: impl Into<String>, endpoint: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.unwrap_or_else(|| OLLAMA_DEFAULT_ENDPOINT.into()),
            model: model.into(),
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
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    #[instrument(skip(self, req), fields(provider = "ollama", model = %self.model))]
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/api/chat", self.endpoint);

        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m: &Message| OllamaMessage {
                role: Self::map_role(&m.role),
                content: &m.content,
            })
            .collect();

        let body = OllamaRequest {
            model: &self.model,
            messages,
            stream: false,
            options: OllamaOptions {
                num_predict: req.max_tokens,
                temperature: req.temperature,
            },
        };

        debug!("POST {url}");

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::http(format!("ollama request failed: {e}")))?;

        let status = resp.status();

        if !status.is_success() {
            let raw = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".into());
            return Err(Error::ai_provider(format!(
                "Ollama HTTP {status}: {raw}"
            )));
        }

        let ollama_resp: OllamaResponse = resp
            .json()
            .await
            .map_err(|e| Error::ai_provider(format!("Ollama: failed to parse response: {e}")))?;

        Ok(CompletionResponse {
            content: ollama_resp.message.content,
            model: ollama_resp.model.unwrap_or_else(|| self.model.clone()),
            prompt_tokens: ollama_resp.prompt_eval_count,
            completion_tokens: ollama_resp.eval_count,
        })
    }

    /// Returns `true` when the Ollama HTTP server is reachable (GET `/api/tags`).
    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.endpoint);
        match self.client.get(&url).send().await {
            Ok(r) => {
                let ok = r.status().is_success();
                if !ok {
                    warn!(status = %r.status(), "Ollama availability check: unexpected status");
                }
                ok
            }
            Err(e) => {
                warn!("Ollama availability check error: {e}");
                false
            }
        }
    }
}
