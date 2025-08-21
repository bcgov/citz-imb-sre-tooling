//! Error types for the sidecar collector

use std::fmt;

pub type Result<T> = std::result::Result<T, CollectorError>;

#[derive(Debug)]
pub enum CollectorError {
    /// IO operation failed
    Io(std::io::Error),

    /// HTTP request failed
    Http(reqwest::Error),

    /// JSON serialization/deserialization failed
    Json(serde_json::Error),

    /// Configuration error
    Config(String),

    /// Log parsing error
    LogParse(String),

    /// Buffer overflow error
    BufferOverflow,

    /// Transport error
    Transport(String),

    /// Generic error with message
    Other(String),
}

impl fmt::Display for CollectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectorError::Io(err) => write!(f, "IO error: {}", err),
            CollectorError::Http(err) => write!(f, "HTTP error: {}", err),
            CollectorError::Json(err) => write!(f, "JSON error: {}", err),
            CollectorError::Config(msg) => write!(f, "Configuration error: {}", msg),
            CollectorError::LogParse(msg) => write!(f, "Log parsing error: {}", msg),
            CollectorError::BufferOverflow => write!(f, "Buffer overflow"),
            CollectorError::Transport(msg) => write!(f, "Transport error: {}", msg),
            CollectorError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for CollectorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CollectorError::Io(err) => Some(err),
            CollectorError::Http(err) => Some(err),
            CollectorError::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CollectorError {
    fn from(err: std::io::Error) -> Self {
        CollectorError::Io(err)
    }
}

impl From<reqwest::Error> for CollectorError {
    fn from(err: reqwest::Error) -> Self {
        CollectorError::Http(err)
    }
}

impl From<serde_json::Error> for CollectorError {
    fn from(err: serde_json::Error) -> Self {
        CollectorError::Json(err)
    }
}
