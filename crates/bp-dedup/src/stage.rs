use serde::{Deserialize, Serialize};
use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::store::ChunkStore;

const CHUNK_SIZE: usize = 4096;

#[derive(Debug, Serialize, Deserialize)]
struct DedupFrame {
    store_bytes:   Vec<u8>,
    refs:          Vec<String>,
    original_size: u64,
    /// The actual data — passed through for downstream stages
    data:          Vec<u8>,
}

pub struct SemanticDedupStage;

impl CompressionStage for SemanticDedupStage {
    fn stage(&self) -> Stage {
        Stage::SemanticDedup
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let mut store = ChunkStore::new();
        let mut refs  = Vec::new();

        for chunk in input.chunks(CHUNK_SIZE) {
            let hash = store.put(chunk);
            refs.push(hash);
        }

        let stats = store.stats();
        info!(
            ratio      = format!("{:.1}x", stats.ratio),
            dupes      = stats.dupes,
            unique     = stats.chunks,
            total_in   = stats.total_in,
            unique_out = stats.unique_out,
            "dedup stage complete"
        );

        let store_bytes = store.to_bytes().map_err(|e| BpError::PipelineFailed {
            stage:  "semantic-dedup".into(),
            reason: e.to_string(),
        })?;

        // Pass original data through for downstream stages
        let frame = DedupFrame {
            store_bytes,
            refs,
            original_size: input.len() as u64,
            data: input.to_vec(),
        };

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "semantic-dedup".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: DedupFrame = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let store = ChunkStore::from_bytes(&frame.store_bytes)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let mut output = Vec::with_capacity(frame.original_size as usize);
        for hash in &frame.refs {
            let chunk = store.get(hash)
                .ok_or_else(|| BpError::DecompressionFailed(
                    format!("chunk ref not found: {}", hash)
                ))?;
            output.extend_from_slice(chunk);
        }

        Ok(output)
    }
}
