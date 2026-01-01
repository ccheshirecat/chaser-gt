//! Error types for the chaser-gt library.

use thiserror::Error;

/// Main error type for the chaser-gt library.
#[derive(Error, Debug)]
pub enum GeekedError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] rquest::Error),

    /// Captcha verification failed
    #[error("Captcha verification failed: {message}")]
    VerificationFailed { message: String },

    /// Unsupported captcha type
    #[error("Unsupported captcha type: {0}")]
    UnsupportedType(String),

    /// Deobfuscation failed
    #[error("Deobfuscation failed: {0}")]
    Deobfuscation(String),

    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Image processing error
    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Regex error
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    /// Invalid response from server
    #[error("Invalid server response: {0}")]
    InvalidResponse(String),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),
}

/// Result type alias for chaser-gt operations.
pub type Result<T> = std::result::Result<T, GeekedError>;
