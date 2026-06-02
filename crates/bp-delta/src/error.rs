use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeltaError {
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("patch apply failed at offset {offset}: {reason}")]
    PatchFailed { offset: usize, reason: String },
    #[error("base record not found: {0}")]
    BaseNotFound(usize),
}
