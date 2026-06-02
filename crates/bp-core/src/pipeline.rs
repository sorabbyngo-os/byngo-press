use crate::error::{BpError, BpResult};
use crate::profile::{Profile, Stage};
use crate::schema::Schema;

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
        let start = std::time::Instant::now();
        let original_size = input.len();
        let mut data = input.to_vec();
        let mut stage_sizes = Vec::new();

        for stage in &self.stages {
            data = stage.compress(&data, self.schema).map_err(|e| {
                BpError::PipelineFailed {
                    stage:  stage.stage().name().to_string(),
                    reason: e.to_string(),
                }
            })?;
            stage_sizes.push((stage.stage().name().to_string(), data.len()));
        }

        let compressed_size = data.len();
        let ratio = original_size as f64 / compressed_size.max(1) as f64;

        Ok((data, PipelineStats {
            original_size,
            compressed_size,
            ratio,
            stage_sizes,
            duration_ms: start.elapsed().as_millis() as u64,
        }))
    }

    pub fn decompress(&self, input: &[u8]) -> BpResult<Vec<u8>> {
        let mut data = input.to_vec();

        // Decompress in reverse stage order
        for stage in self.stages.iter().rev() {
            data = stage.decompress(&data, self.schema).map_err(|e| {
                BpError::PipelineFailed {
                    stage:  stage.stage().name().to_string(),
                    reason: e.to_string(),
                }
            })?;
        }

        Ok(data)
    }
}
