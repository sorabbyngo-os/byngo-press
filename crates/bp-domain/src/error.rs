use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("encode error for type {kind}: {reason}")]
    EncodeError { kind: String, reason: String },
    #[error("decode error for type {kind}: {reason}")]
    DecodeError { kind: String, reason: String },
    #[error("invalid input length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
