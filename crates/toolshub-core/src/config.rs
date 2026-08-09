//! Application configuration subsystem.
//!
//! Loads from and persists to `~/.config/toolshub/config.toml`.
//! All sections implement `Default` so a fresh installation works
//! out-of-the-box without any user-created file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Error, Result};

// ─── GeneralConfig ────────────────────────────────────────────────────────────

/// General UI and locale settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Colour theme name (e.g. `"dark"`, `"light"`, `"catppuccin"`).
    pub theme: String,
    /// IETF language tag for the interface locale (e.g. `"en"`, `"fr"`).
    pub language: String,
    /// Whether to send anonymous usage telemetry.
    pub telemetry: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "en".to_string(),
            telemetry: false,
        }
    }
}

// ─── AiConfig ─────────────────────────────────────────────────────────────────

/// Settings for the AI/LLM backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// AI provider name (e.g. `"openai"`, `"anthropic"`, `"ollama"`).
    pub provider: String,
    /// Model identifier (e.g. `"gpt-4o"`, `"claude-3-5-sonnet-20241022"`).
    pub model: String,
    /// Ordered list of fallback models to try if the primary fails.
    pub fallback_models: Vec<String>,
    /// API key for the provider. `None` for local / anonymous providers.
    pub api_key: Option<String>,
    /// Custom API endpoint URL. Defaults to the provider's official endpoint.
    pub endpoint: String,
    /// Maximum number of tokens to generate in a single response.
    pub max_tokens: u32,
    /// Sampling temperature in [0.0, 2.0]; lower = more deterministic.
    pub temperature: f32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            fallback_models: Vec::new(),
            api_key: None,
            endpoint: "http://localhost:11434".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

// ─── MemoryConfig ─────────────────────────────────────────────────────────────

/// Long-term memory / knowledge-base settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Whether the memory subsystem is active.
    pub enabled: bool,
    /// Path to the SQLite database used for memory storage.
    pub db_path: PathBuf,
    /// Maximum number of facts to retain in the store.
    pub max_facts: u32,
    /// How long (in hours) cached documentation entries remain valid.
    pub doc_cache_ttl_hours: u64,
}

impl MemoryConfig {
    /// Return the resolved absolute path to the SQLite database file.
    ///
    /// If `db_path` is relative, it is resolved relative to the XDG data
    /// directory. Absolute paths are returned unchanged.
    pub fn resolved_db_path(&self) -> std::path::PathBuf {
        if self.db_path.is_absolute() {
            self.db_path.clone()
        } else {
            Config::data_dir().join(&self.db_path)
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: Config::data_dir().join("memory.db"),
            max_facts: 10_000,
            doc_cache_ttl_hours: 24,
        }
    }
}

// ─── PackageConfig ────────────────────────────────────────────────────────────

/// Preferences for package management operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PackageConfig {
    /// Ordered list of preferred package manager names.
    /// The first available manager in the list is used.
    pub preferred_managers: Vec<String>,
    /// If `true`, prompt the user before installing any package.
    pub confirm_before_install: bool,
    /// If `true`, print the shell command before executing it.
    pub show_commands_before_run: bool,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            preferred_managers: vec![
                "pacman".to_string(),
                "apt".to_string(),
                "dnf".to_string(),
                "brew".to_string(),
            ],
            confirm_before_install: true,
            show_commands_before_run: true,
        }
    }
}

// ─── SafetyConfig ─────────────────────────────────────────────────────────────

/// Settings that control the command-safety checker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SafetyConfig {
    /// Whether to run the safety checker before executing commands.
    pub check_commands: bool,
    /// Minimum danger level that will cause a command to be blocked
    /// (e.g. `"medium"`, `"high"`, `"critical"`).
    pub danger_level_threshold: String,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            check_commands: true,
            danger_level_threshold: "high".to_string(),
        }
    }
}

// ─── DisplayConfig ────────────────────────────────────────────────────────────

/// Terminal display and rendering preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Whether to render Unicode / Nerd-Font icons.
    pub icons: bool,
    /// Whether to use ANSI colour codes.
    pub color: bool,
    /// Whether to syntax-highlight code blocks in output.
    pub syntax_highlight: bool,
    /// Whether to pipe long output through a pager (e.g. `less`).
    pub pager: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            icons: true,
            color: true,
            syntax_highlight: true,
            pager: true,
        }
    }
}

// ─── Root Config ──────────────────────────────────────────────────────────────

/// The root application configuration.
///
/// Loaded from `~/.config/toolshub/config.toml` and composed of several
/// nested sections, each of which falls back to sensible defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General UI and locale settings.
    pub general: GeneralConfig,
    /// AI / LLM backend settings.
    pub ai: AiConfig,
    /// Long-term memory settings.
    pub memory: MemoryConfig,
    /// Package management preferences.
    pub package: PackageConfig,
    /// Safety checker settings.
    pub safety: SafetyConfig,
    /// Terminal display preferences.
    pub display: DisplayConfig,
}

impl Config {
    // ── Path helpers ──────────────────────────────────────────────────────

    /// Returns the path to the toolshub configuration file.
    ///
    /// Defaults to `~/.config/toolshub/config.toml`.
    /// Respects `XDG_CONFIG_HOME` via the `dirs` crate.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                warn!("Could not determine config directory; falling back to ~/.config");
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            })
            .join("toolshub")
            .join("config.toml")
    }

    /// Returns the path to the toolshub data directory.
    ///
    /// Defaults to `~/.local/share/toolshub/`.
    /// Respects `XDG_DATA_HOME` via the `dirs` crate.
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                warn!("Could not determine data directory; falling back to ~/.local/share");
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local")
                    .join("share")
            })
            .join("toolshub")
    }

    // ── Load / Save ───────────────────────────────────────────────────────

    /// Loads the configuration from disk.
    ///
    /// If the configuration file does not exist, a default [`Config`] is
    /// written to disk and returned so that subsequent invocations pick it up.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        debug!(path = %path.display(), "Loading configuration");

        if !path.exists() {
            info!(
                path = %path.display(),
                "No configuration file found; creating default"
            );
            let default_config = Config::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let raw = std::fs::read_to_string(&path).map_err(|e| {
            Error::config(format!("Failed to read {}: {e}", path.display()))
        })?;

        let config: Config = toml::from_str(&raw).map_err(|e| {
            Error::config(format!(
                "Failed to parse TOML from {}: {e}",
                path.display()
            ))
        })?;

        debug!(path = %path.display(), "Configuration loaded successfully");
        Ok(config)
    }

    /// Persists the current configuration to disk.
    ///
    /// Creates the parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML cannot be serialised or the file cannot
    /// be written.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        debug!(path = %path.display(), "Saving configuration");

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::config(format!(
                    "Failed to create config directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let toml_str = toml::to_string_pretty(self).map_err(|e| {
            Error::toml(format!("Failed to serialise configuration: {e}"))
        })?;

        std::fs::write(&path, toml_str.as_bytes()).map_err(|e| {
            Error::config(format!("Failed to write {}: {e}", path.display()))
        })?;

        info!(path = %path.display(), "Configuration saved");
        Ok(())
    }

    /// Loads the config from a specific file path (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is malformed.
    pub fn load_from(path: &Path) -> Result<Self> {
        debug!(path = %path.display(), "Loading configuration from custom path");

        let raw = std::fs::read_to_string(path).map_err(|e| {
            Error::config(format!("Failed to read {}: {e}", path.display()))
        })?;

        let config: Config = toml::from_str(&raw).map_err(|e| {
            Error::config(format!(
                "Failed to parse TOML from {}: {e}",
                path.display()
            ))
        })?;

        Ok(config)
    }

    /// Saves the configuration to a specific file path (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML cannot be serialised or the file cannot
    /// be written.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::config(format!(
                    "Failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let toml_str = toml::to_string_pretty(self).map_err(|e| {
            Error::toml(format!("Failed to serialise configuration: {e}"))
        })?;

        std::fs::write(path, toml_str.as_bytes()).map_err(|e| {
            Error::config(format!("Failed to write {}: {e}", path.display()))
        })?;

        Ok(())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.general.theme, "dark");
        assert_eq!(cfg.general.language, "en");
        assert!(!cfg.general.telemetry);
        assert_eq!(cfg.ai.provider, "ollama");
        assert!(cfg.safety.check_commands);
        assert!(cfg.display.color);
    }

    #[test]
    fn config_roundtrip_toml() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialise");
        let cfg2: Config = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(cfg.general.theme, cfg2.general.theme);
        assert_eq!(cfg.ai.model, cfg2.ai.model);
        assert_eq!(cfg.memory.max_facts, cfg2.memory.max_facts);
    }

    #[test]
    fn config_save_and_load_from() {
        // Use a unique temp path under /tmp — no extra crate dependency needed.
        let unique = uuid::Uuid::new_v4().to_string();
        let dir = std::path::PathBuf::from(format!("/tmp/toolshub-test-{unique}"));
        let path = dir.join("config.toml");

        let original = Config::default();
        original.save_to(&path).expect("save_to");

        assert!(path.exists(), "config file should exist after save_to");

        let loaded = Config::load_from(&path).expect("load_from");
        assert_eq!(original.general.theme, loaded.general.theme);
        assert_eq!(original.ai.max_tokens, loaded.ai.max_tokens);
        assert_eq!(
            original.package.confirm_before_install,
            loaded.package.confirm_before_install
        );

        // Clean up.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_path_is_absolute() {
        let p = Config::config_path();
        assert!(p.is_absolute(), "config path must be absolute");
    }

    #[test]
    fn data_dir_is_absolute() {
        let d = Config::data_dir();
        assert!(d.is_absolute(), "data dir must be absolute");
    }
}
