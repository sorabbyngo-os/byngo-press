use thiserror::Error;

#[derive(Debug, Error)]
pub enum DictError {
    #[error("term not found in dictionary: {0}")]
    TermNotFound(String),
    #[error("index out of range: {0}")]
    IndexOutOfRange(u16),
    #[error("serialization error: {0}")]
    Serialization(String),
}
