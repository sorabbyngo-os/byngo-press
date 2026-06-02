use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimdError {
    #[error("compression failed: {0}")]
    CompressionFailed(String),
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),
}
