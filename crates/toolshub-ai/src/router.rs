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
    provider: Box<dyn AiProvider>,
}

impl AiRouter {
    /// Build an `AiRouter` from the application [`AiConfig`].
    ///
    /// Supported `config.provider` values: `"openai"`, `"anthropic"`, `"ollama"`.
    #[instrument(skip(config), fields(provider = %config.provider, model = %config.model))]
    pub async fn from_config(config: &AiConfig) -> Result<Self> {
        let provider: Box<dyn AiProvider> = match config.provider.to_lowercase().as_str() {
            "openai" => {
                let api_key = config
                    .api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        Error::config("AiConfig: `api_key` is required for the openai provider")
                    })?;

                Box::new(OpenAiProvider::new(api_key, &config.model, Some(config.endpoint.clone())))
            }

            "anthropic" => {
                let api_key = config
                    .api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| {
                        Error::config(
                            "AiConfig: `api_key` is required for the anthropic provider",
                        )
                    })?;

                debug!("Initialising Anthropic provider, model={}", config.model);
                Box::new(AnthropicProvider::new(
                    api_key,
                    &config.model,
                    Some(config.endpoint.clone()),
                ))
            }

            "ollama" => {
                Box::new(OllamaProvider::new(&config.model, Some(config.endpoint.clone())))
            }

            other => {
                return Err(Error::config(format!(
                    "AiConfig: unknown provider `{other}`. \
                     Supported values: openai, anthropic, ollama"
                )));
            }
        };

        Ok(Self { provider })
    }

    /// Forward a completion request to the underlying provider.
    #[instrument(skip(self, req), fields(provider = %self.provider.name()))]
    pub async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        self.provider.complete(req).await
    }

    /// Return the name of the active provider (e.g. `"openai"`).
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Return the model the active provider is configured to use.
    pub fn model(&self) -> &str {
        self.provider.model()
    }

    /// Delegate to the provider's availability check.
    pub async fn is_available(&self) -> bool {
        self.provider.is_available().await
    }
}
