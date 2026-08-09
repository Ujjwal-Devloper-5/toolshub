//! Provider routing: selects the right [`AiProvider`] backend from configuration.

use tracing::{debug, instrument};

use toolshub_core::{config::AiConfig, Error, Result};

use crate::anthropic::AnthropicProvider;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiProvider;
use crate::provider::{AiProvider, CompletionRequest, CompletionResponse};

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
    /// Supported `config.provider` values: `"openai"`, `"anthropic"`, `"ollama"`.
    #[instrument(skip(config), fields(provider = %config.provider, model = %config.model))]
    pub async fn from_config(config: &AiConfig) -> Result<Self> {
        let mut providers = Vec::new();

        // 1. Primary provider
        providers.push(Self::build_provider(&config.provider, &config.model, config)?);

        // 2. Fallback providers
        for fallback in &config.fallback_models {
            // If the fallback specifies a provider (e.g. "openrouter:google/gemma..."), parse it
            let (prov, modl) = if let Some((p, m)) = fallback.split_once("::") {
                (p, m)
            } else {
                (config.provider.as_str(), fallback.as_str())
            };
            
            // Log fallback registration
            debug!("Registering fallback provider: {} with model {}", prov, modl);
            match Self::build_provider(prov, modl, config) {
                Ok(p) => providers.push(p),
                Err(e) => tracing::warn!("Failed to initialise fallback {fallback}: {e}"),
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
                    "https://openrouter.ai/api/v1".to_string()
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
    #[instrument(skip(self, req))]
    pub async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let mut last_error = None;

        for (idx, provider) in self.providers.iter().enumerate() {
            if !provider.is_available().await {
                tracing::debug!("Provider {} (index {}) is unavailable, skipping...", provider.name(), idx);
                continue;
            }

            match provider.complete(req.clone()).await {
                Ok(response) => {
                    if idx > 0 {
                        tracing::info!("Fallback successful using {} ({})", provider.name(), provider.model());
                    }
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!("Provider {} ({}) failed: {}", provider.name(), provider.model(), e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| Error::ai_provider("All configured providers failed or were unavailable.")))
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
            if provider.is_available().await {
                return true;
            }
        }
        false
    }
}
