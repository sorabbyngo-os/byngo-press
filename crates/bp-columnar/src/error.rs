use thiserror::Error;

#[derive(Debug, Error)]
pub enum ColumnarError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("column {0} not found")]
    ColumnNotFound(String),
    #[error("row count mismatch: expected {expected}, got {actual}")]
    RowCountMismatch { expected: usize, actual: usize },
}
