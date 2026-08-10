//! Provider routing: selects the right [`AiProvider`] backend from configuration.
//!
//! The router tries the primary provider first, then falls back through the
//! configured list.  Each attempt is bounded by a 15-second timeout so a
//! slow or hung provider doesn't stall the entire request.

use std::time::Duration;

use tokio::time::timeout;
use tracing::{debug, instrument, warn};

use toolshub_core::{config::AiConfig, Error, Result};

use crate::anthropic::AnthropicProvider;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiProvider;
use crate::provider::{AiProvider, CompletionRequest, CompletionResponse};

/// Timeout applied to every individual provider attempt.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);

// ── AiRouter ──────────────────────────────────────────────────────────────────

/// Routes AI completion requests to the configured backend provider.
///
/// Constructed via [`AiRouter::from_config`]; thereafter acts as a thin
/// pass-through to the selected [`AiProvider`] implementation.
pub struct AiRouter {
    providers: Vec<Box<dyn AiProvider>>,
}

impl AiRouter {
    /// Build an `AiRouter` from the application [`AiConfig`].
    ///
    /// Supported `config.provider` values: `"openai"`, `"anthropic"`, `"ollama"`, `"openrouter"`.
    #[instrument(skip(config), fields(provider = %config.provider, model = %config.model))]
    pub async fn from_config(config: &AiConfig) -> Result<Self> {
        let mut providers = Vec::new();

        // 1. Primary provider
        providers.push(Self::build_provider(&config.provider, &config.model, config)?);

        // 2. Fallback providers
        for fallback in &config.fallback_models {
            // If the fallback specifies a provider (e.g. "openrouter::google/gemma..."), parse it
            let (prov, modl) = if let Some((p, m)) = fallback.split_once("::") {
                (p, m)
            } else {
                (config.provider.as_str(), fallback.as_str())
            };

            debug!("Registering fallback provider: {} with model {}", prov, modl);
            match Self::build_provider(prov, modl, config) {
                Ok(p) => providers.push(p),
                Err(e) => warn!("Failed to initialise fallback {fallback}: {e}"),
            }
        }

        if providers.is_empty() {
            return Err(Error::config("No valid AI providers could be initialised."));
        }

        Ok(Self { providers })
    }

    fn build_provider(provider: &str, model: &str, config: &AiConfig) -> Result<Box<dyn AiProvider>> {
        match provider.to_lowercase().as_str() {
            "openai" => {
                let api_key = config
                    .api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        Error::config("AiConfig: `api_key` is required for the openai provider")
                    })?;
                Ok(Box::new(OpenAiProvider::new(api_key, model, Some(config.endpoint.clone()))))
            }
            "openrouter" => {
                let api_key = config
                    .api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        Error::config("AiConfig: `api_key` is required for the openrouter provider")
                    })?;

                // OpenRouter uses the OpenAI compatibility layer.
                let endpoint = if config.endpoint.is_empty() || config.endpoint == "http://localhost:11434" {
                    "https://openrouter.ai/api".to_string()
                } else {
                    config.endpoint.clone()
                };

                Ok(Box::new(OpenAiProvider::new(api_key, model, Some(endpoint))))
            }
            "anthropic" => {
                let api_key = config
                    .api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        Error::config("AiConfig: `api_key` is required for the anthropic provider")
                    })?;
                Ok(Box::new(AnthropicProvider::new(
                    api_key,
                    model,
                    Some(config.endpoint.clone()),
                )))
            }
            "ollama" => {
                Ok(Box::new(OllamaProvider::new(model, Some(config.endpoint.clone()))))
            }
            other => Err(Error::config(format!(
                "AiConfig: unknown provider `{other}`. Supported values: openai, openrouter, anthropic, ollama"
            ))),
        }
    }

    /// Forward a completion request, trying fallback providers if the primary fails.
    ///
    /// Each provider attempt is bounded by [`PROVIDER_TIMEOUT`] so a hung
    /// provider never blocks indefinitely.
    #[instrument(skip(self, req))]
    pub async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let mut last_error = None;

        for (idx, provider) in self.providers.iter().enumerate() {
            // Availability check — also timeout-bounded.
            let available = timeout(PROVIDER_TIMEOUT, provider.is_available())
                .await
                .unwrap_or(false);

            if !available {
                debug!(
                    "Provider {} (index {}) unavailable or timed out, skipping…",
                    provider.name(),
                    idx
                );
                continue;
            }

            // Attempt completion with a hard timeout.
            let result = timeout(PROVIDER_TIMEOUT, provider.complete(req.clone())).await;

            match result {
                Ok(Ok(response)) => {
                    if idx > 0 {
                        tracing::info!(
                            "Fallback successful using {} ({})",
                            provider.name(),
                            provider.model()
                        );
                    }
                    return Ok(response);
                }
                Ok(Err(e)) => {
                    warn!(
                        "Provider {} ({}) failed: {}",
                        provider.name(),
                        provider.model(),
                        e
                    );
                    last_error = Some(e);
                }
                Err(_elapsed) => {
                    warn!(
                        "Provider {} ({}) timed out after {}s",
                        provider.name(),
                        provider.model(),
                        PROVIDER_TIMEOUT.as_secs()
                    );
                    last_error = Some(Error::ai_provider(format!(
                        "Provider {} timed out after {}s",
                        provider.name(),
                        PROVIDER_TIMEOUT.as_secs()
                    )));
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| Error::ai_provider("All configured providers failed or were unavailable.")))
    }

    /// Stream a completion from the first available provider that supports it.
    ///
    /// Tokens are printed to stdout as they arrive (via each provider's
    /// `stream_to_stdout()` implementation). Falls back through the provider
    /// list if the first available provider fails.
    pub async fn complete_streaming(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let mut last_error = None;

        for (idx, provider) in self.providers.iter().enumerate() {
            let available = timeout(PROVIDER_TIMEOUT, provider.is_available())
                .await
                .unwrap_or(false);

            if !available {
                continue;
            }

            // Use streaming if supported, fall back to regular complete().
            let fut = if provider.supports_streaming() {
                provider.stream_to_stdout(req.clone())
            } else {
                provider.complete(req.clone())
            };

            match timeout(PROVIDER_TIMEOUT, fut).await {
                Ok(Ok(r)) => {
                    if idx > 0 {
                        tracing::info!(
                            "Streaming fallback successful using {} ({})",
                            provider.name(),
                            provider.model()
                        );
                    }
                    return Ok(r);
                }
                Ok(Err(e)) => {
                    warn!("Provider {} ({}) stream failed: {}", provider.name(), provider.model(), e);
                    last_error = Some(e);
                }
                Err(_) => {
                    warn!("Provider {} ({}) stream timed out after {}s", provider.name(), provider.model(), PROVIDER_TIMEOUT.as_secs());
                    last_error = Some(Error::ai_provider(format!(
                        "Provider {} stream timed out",
                        provider.name()
                    )));
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| Error::ai_provider("All configured providers failed or were unavailable.")))
    }

    /// Return the name of the active primary provider (e.g. `"openai"`).
    pub fn provider_name(&self) -> &str {
        self.providers.first().map(|p| p.name()).unwrap_or("unknown")
    }

    /// Return the model the primary provider is configured to use.
    pub fn model(&self) -> &str {
        self.providers.first().map(|p| p.model()).unwrap_or("unknown")
    }

    /// Delegate to the providers' availability checks (true if at least one is available).
    pub async fn is_available(&self) -> bool {
        for provider in &self.providers {
            let available = timeout(PROVIDER_TIMEOUT, provider.is_available())
                .await
                .unwrap_or(false);
            if available {
                return true;
            }
        }
        false
    }
}
