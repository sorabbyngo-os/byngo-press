use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::delta::{compute_sequence, reconstruct_sequence};

pub struct DeltaCompressionStage;

fn extract_records(input: &[u8]) -> BpResult<Vec<serde_json::Value>> {
    let val: serde_json::Value = serde_json::from_slice(input)
        .map_err(|e| BpError::PipelineFailed {
            stage:  "delta-compression".into(),
            reason: e.to_string(),
        })?;

    // DictFrame — has "dictionary" and "encoded_data" fields
    // encoded_data is Vec<u8> serialized as JSON array of numbers
    if val.get("dictionary").is_some() {
        if let Some(encoded_data) = val.get("encoded_data") {
            // encoded_data is a JSON array of u8 numbers
            let bytes: Vec<u8> = serde_json::from_value(encoded_data.clone())
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: format!("encoded_data decode: {}", e),
                })?;
            let arr: Vec<serde_json::Value> = serde_json::from_slice(&bytes)
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: format!("encoded_data parse: {}", e),
                })?;
            return Ok(arr);
        }
    }

    // Plain array
    if let serde_json::Value::Array(arr) = val {
        return Ok(arr);
    }

    Ok(vec![val])
}

impl CompressionStage for DeltaCompressionStage {
    fn stage(&self) -> Stage {
        Stage::DeltaCompression
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let records = extract_records(input)?;

        let seq = compute_sequence(&records);
        let total_fields: usize = seq.deltas.iter().map(|d| d.changed_count()).sum();

        info!(
            records        = records.len(),
            deltas         = seq.deltas.len(),
            changed_fields = total_fields,
            avg_changes    = if seq.deltas.is_empty() { 0.0 } else {
                total_fields as f64 / seq.deltas.len() as f64
            },
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

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
