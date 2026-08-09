use thiserror::Error;

/// The primary error type for all toolshub-core operations.
#[derive(Debug, Error)]
pub enum Error {
    /// A configuration-related error (parsing, missing keys, invalid values).
    #[error("Configuration error: {0}")]
    Config(String),

    /// A database operation error (read/write failures, schema issues).
    #[error("Database error: {0}")]
    Database(String),

    /// An underlying I/O error, forwarded transparently from std::io.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An HTTP request or response error.
    #[error("HTTP error: {0}")]
    Http(String),

    /// A JSON serialisation or deserialisation error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A TOML serialisation or deserialisation error.
    #[error("TOML error: {0}")]
    Toml(String),

    /// An error originating from an AI provider (authentication, quota, etc.).
    #[error("AI provider error: {0}")]
    AiProvider(String),

    /// An error from a system or language package manager.
    #[error("Package manager error: {0}")]
    PackageManager(String),

    /// The requested command could not be found on the system.
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Failure while detecting the runtime environment (OS, shell, etc.).
    #[error("Environment detection error: {0}")]
    EnvDetection(String),

    /// A command was blocked by the safety checker.
    #[error("Safety check failed for command `{command}`: {message}")]
    SafetyCheck {
        /// The command that triggered the safety check.
        command: String,
        /// Human-readable explanation of why the command was blocked.
        message: String,
    },

    /// The requested operation is not supported on the current platform.
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// A catch-all error for situations not covered by specific variants.
    #[error("Error: {0}")]
    Other(String),
}

impl Error {
    /// Convenience constructor for [`Error::Config`].
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Convenience constructor for [`Error::Database`].
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Convenience constructor for [`Error::Http`].
    pub fn http(msg: impl Into<String>) -> Self {
        Self::Http(msg.into())
    }

    /// Convenience constructor for [`Error::Toml`].
    pub fn toml(msg: impl Into<String>) -> Self {
        Self::Toml(msg.into())
    }

    /// Convenience constructor for [`Error::AiProvider`].
    pub fn ai_provider(msg: impl Into<String>) -> Self {
        Self::AiProvider(msg.into())
    }

    /// Convenience constructor for [`Error::PackageManager`].
    pub fn package_manager(msg: impl Into<String>) -> Self {
        Self::PackageManager(msg.into())
    }

    /// Convenience constructor for [`Error::CommandNotFound`].
    pub fn command_not_found(cmd: impl Into<String>) -> Self {
        Self::CommandNotFound(cmd.into())
    }

    /// Convenience constructor for [`Error::EnvDetection`].
    pub fn env_detection(msg: impl Into<String>) -> Self {
        Self::EnvDetection(msg.into())
    }

    /// Convenience constructor for [`Error::SafetyCheck`].
    pub fn safety_check(command: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SafetyCheck {
            command: command.into(),
            message: message.into(),
        }
    }

    /// Convenience constructor for [`Error::NotSupported`].
    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::NotSupported(msg.into())
    }

    /// Convenience constructor for [`Error::Other`].
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Returns `true` if this error is a safety check failure.
    pub fn is_safety_check(&self) -> bool {
        matches!(self, Self::SafetyCheck { .. })
    }

    /// Returns `true` if this error is an I/O error.
    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(_))
    }
}

/// A specialised `Result` type for toolshub-core operations.
pub type Result<T> = std::result::Result<T, Error>;
