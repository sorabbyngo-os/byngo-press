use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::entropy::EntropyCodec;

pub struct EntropyCodingStage {
    codec: EntropyCodec,
}

impl EntropyCodingStage {
    pub fn new() -> Self {
        Self { codec: EntropyCodec::new() }
    }
}

impl Default for EntropyCodingStage {
    fn default() -> Self { Self::new() }
}

impl CompressionStage for EntropyCodingStage {
    fn stage(&self) -> Stage {
        Stage::EntropyCoding
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let result = self.codec.compress(input)
            .map_err(|e| BpError::CompressionFailed(e.to_string()))?;

        info!(
            input  = input.len(),
            output = result.len(),
            ratio  = input.len() as f64 / result.len().max(1) as f64,
            "entropy coding complete"
        );

        Ok(result)
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        self.codec.decompress(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
