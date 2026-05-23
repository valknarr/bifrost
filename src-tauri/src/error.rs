use serde::{Serialize, Serializer};

/// Unified error type for Bifrost. Serialises as a plain string for the
/// frontend so `invoke()` errors are easy to display.
#[derive(Debug, thiserror::Error)]
pub enum BifrostError {
    #[error("Sandboxie is not installed or could not be found")]
    SandboxieMissing,

    #[error("Sandboxie command failed: {0}")]
    SandboxieCommand(String),

    #[error("Rider not found: {0}")]
    RiderNotFound(String),

    #[error("Rider already exists: {0}")]
    RiderExists(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl Serialize for BifrostError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BifrostError>;
