use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::delta::{compute_sequence, reconstruct_sequence};

pub struct DeltaCompressionStage;

impl CompressionStage for DeltaCompressionStage {
    fn stage(&self) -> Stage {
        Stage::DeltaCompression
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        // Input may be a JSON array or a wrapped object from a previous stage
        // Try array first, then wrap single object in array
        let records: Vec<serde_json::Value> = match serde_json::from_slice::<Vec<serde_json::Value>>(input) {
            Ok(arr) => arr,
            Err(_)  => {
                // Try as single value and wrap
                match serde_json::from_slice::<serde_json::Value>(input) {
                    Ok(val) => vec![val],
                    Err(e)  => return Err(BpError::PipelineFailed {
                        stage:  "delta-compression".into(),
                        reason: e.to_string(),
                    }),
                }
            }
        };

        let seq = compute_sequence(&records);

        let total_fields: usize = seq.deltas.iter().map(|d| d.changed_count()).sum();

        info!(
            records        = records.len(),
            deltas         = seq.deltas.len(),
            changed_fields = total_fields,
            "delta stage complete"
        );

        serde_json::to_vec(&seq).map_err(|e| BpError::PipelineFailed {
            stage:  "delta-compression".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let seq: crate::delta::DeltaSequence = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let records = reconstruct_sequence(&seq);

        serde_json::to_vec(&records).map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
