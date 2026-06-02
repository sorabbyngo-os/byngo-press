use thiserror::Error;

#[derive(Debug, Error)]
pub enum DedupError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("hash collision detected for key {0}")]
    HashCollision(String),
    #[error("reference not found: {0}")]
    RefNotFound(String),
}
