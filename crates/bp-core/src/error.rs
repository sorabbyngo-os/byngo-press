use thiserror::Error;

#[derive(Debug, Error)]
pub enum BpError {
    #[error("invalid magic bytes — not a ByngoPress file")]
    InvalidMagic,
    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u8),
    #[error("block {0} not found in skip index")]
    BlockNotFound(u64),
    #[error("checksum mismatch — data corrupted")]
    ChecksumMismatch,
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("compression failed: {0}")]
    CompressionFailed(String),
    #[error("schema detection failed: {0}")]
    SchemaDetectionFailed(String),
    #[error("pipeline stage failed at {stage}: {reason}")]
    PipelineFailed { stage: String, reason: String },
    #[error("profile {0} does not support stage {1}")]
    StageNotSupported(String, String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type BpResult<T> = Result<T, BpError>;
