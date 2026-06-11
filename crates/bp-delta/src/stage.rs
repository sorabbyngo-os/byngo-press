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
        let val: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "delta-compression".into(),
                reason: e.to_string(),
            })?;

        // Detect DictFrame — extract records from encoded_data,
        // compress to delta bytes, store back in encoded_data, preserve dictionary
        if val.get("dictionary").is_some() {
            if let Some(encoded_data_val) = val.get("encoded_data") {
                let record_bytes: Vec<u8> = serde_json::from_value(encoded_data_val.clone())
                    .map_err(|e| BpError::PipelineFailed {
                        stage:  "delta-compression".into(),
                        reason: format!("encoded_data decode: {}", e),
                    })?;
                let records: Vec<serde_json::Value> = serde_json::from_slice(&record_bytes)
                    .map_err(|e| BpError::PipelineFailed {
                        stage:  "delta-compression".into(),
                        reason: format!("encoded_data parse: {}", e),
                    })?;

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

                let delta_bytes = serde_json::to_vec(&seq)
                    .map_err(|e| BpError::PipelineFailed {
                        stage:  "delta-compression".into(),
                        reason: e.to_string(),
                    })?;

                // Re-wrap: store delta bytes in encoded_data, keep dictionary field
                let mut frame = val.as_object().unwrap().clone();
                frame.insert(
                    "encoded_data".into(),
                    serde_json::to_value(&delta_bytes).unwrap(),
                );
                return serde_json::to_vec(&serde_json::Value::Object(frame))
                    .map_err(|e| BpError::PipelineFailed {
                        stage:  "delta-compression".into(),
                        reason: e.to_string(),
                    });
            }
        }

        // Plain array (no DictFrame)
        let records: Vec<serde_json::Value> = match val {
            serde_json::Value::Array(arr) => arr,
            other => vec![other],
        };
        let seq = compute_sequence(&records);
        serde_json::to_vec(&seq).map_err(|e| BpError::PipelineFailed {
            stage:  "delta-compression".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let val: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // DictFrame: encoded_data holds DeltaSequence bytes
        // Reconstruct records, put back in encoded_data, return frame for dictionary
        if val.get("dictionary").is_some() {
            if let Some(encoded_data_val) = val.get("encoded_data") {
                let delta_bytes: Vec<u8> = serde_json::from_value(encoded_data_val.clone())
                    .map_err(|e| BpError::DecompressionFailed(
                        format!("encoded_data decode: {}", e)
                    ))?;
                let seq: crate::delta::DeltaSequence = serde_json::from_slice(&delta_bytes)
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
                let records = reconstruct_sequence(&seq);
                let record_bytes = serde_json::to_vec(&records)
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

                let mut frame = val.as_object().unwrap().clone();
                frame.insert(
                    "encoded_data".into(),
                    serde_json::to_value(&record_bytes).unwrap(),
                );
                return serde_json::to_vec(&serde_json::Value::Object(frame))
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()));
            }
        }

        // Plain DeltaSequence
        let seq: crate::delta::DeltaSequence = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
        let records = reconstruct_sequence(&seq);
        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
