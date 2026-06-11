use serde::{Deserialize, Serialize};

use crate::error::{BpError, BpResult};
use crate::profile::{Profile, Stage};
use crate::schema::Schema;

const MAGIC: &[u8; 4] = b"BPx1";

/// Self-describing envelope written at the front of every .bp file
#[derive(Serialize, Deserialize)]
pub struct BpEnvelope {
    pub profile: Profile,
    pub schema:  Schema,
    pub payload: Vec<u8>,
}

impl BpEnvelope {
    /// Decode an envelope from raw .bp bytes; returns None for legacy files
    pub fn decode(input: &[u8]) -> Option<BpResult<Self>> {
        if !input.starts_with(MAGIC) { return None; }
        Some(
            serde_json::from_slice(&input[MAGIC.len()..])
                .map_err(|e| BpError::DecompressionFailed(
                    format!("envelope decode: {}", e)
                ))
        )
    }
}

/// A compression stage — takes bytes, returns transformed bytes
pub trait CompressionStage: Send + Sync {
    fn stage(&self) -> Stage;
    fn compress(&self, input: &[u8], schema: Schema) -> BpResult<Vec<u8>>;
    fn decompress(&self, input: &[u8], schema: Schema) -> BpResult<Vec<u8>>;
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub original_size:   usize,
    pub compressed_size: usize,
    pub ratio:           f64,
    pub stage_sizes:     Vec<(String, usize)>,
    pub duration_ms:     u64,
}

/// Orchestrates all stages for a given profile
pub struct Pipeline {
    pub profile: Profile,
    pub schema:  Schema,
    stages:      Vec<Box<dyn CompressionStage>>,
}

impl Pipeline {
    pub fn new(profile: Profile, schema: Schema) -> Self {
        Self { profile, schema, stages: Vec::new() }
    }

    pub fn add_stage(&mut self, stage: Box<dyn CompressionStage>) {
        self.stages.push(stage);
    }

    pub fn compress(&self, input: &[u8]) -> BpResult<(Vec<u8>, PipelineStats)> {
        let start         = std::time::Instant::now();
        let original_size = input.len();
        let mut data      = input.to_vec();
        let mut stage_sizes = Vec::new();

        for stage in &self.stages {
            data = stage.compress(&data, self.schema).map_err(|e| BpError::PipelineFailed {
                stage:  stage.stage().name().to_string(),
                reason: e.to_string(),
            })?;
            stage_sizes.push((stage.stage().name().to_string(), data.len()));
        }

        // Wrap payload in self-describing envelope
        let envelope = BpEnvelope { profile: self.profile, schema: self.schema, payload: data };
        let env_bytes = serde_json::to_vec(&envelope).map_err(|e| BpError::PipelineFailed {
            stage: "envelope".into(), reason: e.to_string(),
        })?;
        let mut out = MAGIC.to_vec();
        out.extend(env_bytes);

        let compressed_size = out.len();
        let ratio           = original_size as f64 / compressed_size.max(1) as f64;
        Ok((out, PipelineStats {
            original_size, compressed_size, ratio, stage_sizes,
            duration_ms: start.elapsed().as_millis() as u64,
        }))
    }

    pub fn decompress(&self, input: &[u8]) -> BpResult<Vec<u8>> {
        // New .bp files: use the profile/schema baked into the envelope
        if let Some(result) = BpEnvelope::decode(input) {
            let envelope = result?;
            // Caller (bp-cli) built this pipeline with the correct profile already;
            // if profile matches, run stages in reverse on the inner payload.
            // If it doesn't match, bp-cli should call decompress_raw directly.
            return self.decompress_raw(&envelope.payload);
        }
        // Legacy: no envelope, decompress raw bytes with current stage list
        self.decompress_raw(input)
    }

    /// Run stages in reverse on raw (non-enveloped) bytes
    pub fn decompress_raw(&self, input: &[u8]) -> BpResult<Vec<u8>> {
        let mut data = input.to_vec();
        for stage in self.stages.iter().rev() {
            data = stage.decompress(&data, self.schema).map_err(|e| BpError::PipelineFailed {
                stage:  stage.stage().name().to_string(),
                reason: e.to_string(),
            })?;
        }
        Ok(data)
    }
}
