use tracing::info;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;
use crate::delta::{compute_sequence, reconstruct_sequence, DeltaSequence};

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

        // Dedup frame: delta-compress only the unique records in "u"
        if val.get("_dedup").is_some() {
            let unique: Vec<serde_json::Value> = serde_json::from_value(val["u"].clone())
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: format!("u decode: {}", e),
                })?;

            let seq = compute_sequence(&unique);
            let total_fields: usize = seq.deltas.iter().map(|d| d.changed_count()).sum();
            let avg_changes = if seq.deltas.is_empty() { 0.0 } else {
                total_fields as f64 / seq.deltas.len() as f64
            };

            let delta_bytes = serde_json::to_vec(&seq)
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: e.to_string(),
                })?;
            let original_u_bytes = serde_json::to_vec(&unique)
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: e.to_string(),
                })?;
            let delta_effective = delta_bytes.len() < original_u_bytes.len();

            info!(
                records        = unique.len(),
                deltas         = seq.deltas.len(),
                changed_fields = total_fields,
                avg_changes    = avg_changes,
                delta_bytes    = delta_bytes.len(),
                original_bytes = original_u_bytes.len(),
                adaptive_skip  = !delta_effective,
                "delta stage complete"
            );

            let mut frame = val.as_object().unwrap().clone();
            if delta_effective {
                let seq_val = serde_json::to_value(&seq).unwrap();
                frame.insert("u".into(), seq_val);
                frame.insert("_delta".into(), serde_json::Value::Bool(true));
            } else {
                frame.insert("_delta".into(), serde_json::Value::Bool(false));
            }
            return serde_json::to_vec(&serde_json::Value::Object(frame))
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "delta-compression".into(),
                    reason: e.to_string(),
                });
        }

        // Dict frame (legacy ordering — dictionary before delta; not used by
        // current profiles but kept for safety)
        if val.get("dictionary").is_some() {
            if let Some(encoded_data_val) = val.get("encoded_data") {
                if let Some(b64) = encoded_data_val.as_str() {
                    let record_bytes = B64.decode(b64)
                        .map_err(|e| BpError::PipelineFailed {
                            stage:  "delta-compression".into(),
                            reason: format!("b64 decode: {}", e),
                        })?;
                    let records: Vec<serde_json::Value> = serde_json::from_slice(&record_bytes)
                        .map_err(|e| BpError::PipelineFailed {
                            stage:  "delta-compression".into(),
                            reason: format!("parse: {}", e),
                        })?;
                    let seq = compute_sequence(&records);
                    let delta_bytes = serde_json::to_vec(&seq)
                        .map_err(|e| BpError::PipelineFailed {
                            stage:  "delta-compression".into(),
                            reason: e.to_string(),
                        })?;
                    let mut frame = val.as_object().unwrap().clone();
                    frame.insert("encoded_data".into(), serde_json::Value::String(B64.encode(&delta_bytes)));
                    return serde_json::to_vec(&serde_json::Value::Object(frame))
                        .map_err(|e| BpError::PipelineFailed {
                            stage:  "delta-compression".into(),
                            reason: e.to_string(),
                        });
                }
            }
        }

        // Plain array (no dedup wrapper) — adaptive delta with {"_delta":bool,"d":...} wrapper
        let records: Vec<serde_json::Value> = match val {
            serde_json::Value::Array(arr) => arr,
            other => vec![other],
        };

        let seq = compute_sequence(&records);
        let total_fields: usize = seq.deltas.iter().map(|d| d.changed_count()).sum();
        let avg_changes = if seq.deltas.is_empty() { 0.0 } else {
            total_fields as f64 / seq.deltas.len() as f64
        };

        let delta_bytes = serde_json::to_vec(&seq)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "delta-compression".into(),
                reason: e.to_string(),
            })?;
        let original_bytes = serde_json::to_vec(&records)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "delta-compression".into(),
                reason: e.to_string(),
            })?;
        let delta_effective = delta_bytes.len() < original_bytes.len();

        info!(
            records        = records.len(),
            deltas         = seq.deltas.len(),
            changed_fields = total_fields,
            avg_changes    = avg_changes,
            delta_bytes    = delta_bytes.len(),
            original_bytes = original_bytes.len(),
            adaptive_skip  = !delta_effective,
            "delta stage complete"
        );

        let out = if delta_effective {
            serde_json::json!({ "_delta": true, "d": seq })
        } else {
            serde_json::json!({ "_delta": false, "d": records })
        };
        serde_json::to_vec(&out).map_err(|e| BpError::PipelineFailed {
            stage:  "delta-compression".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let val: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // Dedup frame: check _delta flag to know if "u" was delta-compressed
        if val.get("_dedup").is_some() {
            let delta_was_applied = val.get("_delta")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if delta_was_applied {
                let seq: DeltaSequence =
                    serde_json::from_value(val["u"].clone())
                        .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
                let unique = reconstruct_sequence(&seq);
                let mut frame = val.as_object().unwrap().clone();
                frame.insert("u".into(), serde_json::to_value(&unique).unwrap());
                return serde_json::to_vec(&serde_json::Value::Object(frame))
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()));
            } else {
                return Ok(input.to_vec());
            }
        }

        // Dict frame (legacy — not used by current profiles)
        if val.get("dictionary").is_some() {
            if let Some(encoded_data_val) = val.get("encoded_data") {
                if let Some(b64) = encoded_data_val.as_str() {
                    let delta_bytes = B64.decode(b64)
                        .map_err(|e| BpError::DecompressionFailed(format!("b64 decode: {}", e)))?;
                    let seq: DeltaSequence = serde_json::from_slice(&delta_bytes)
                        .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
                    let records = reconstruct_sequence(&seq);
                    let record_bytes = serde_json::to_vec(&records)
                        .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
                    let mut frame = val.as_object().unwrap().clone();
                    frame.insert("encoded_data".into(), serde_json::Value::String(B64.encode(&record_bytes)));
                    return serde_json::to_vec(&serde_json::Value::Object(frame))
                        .map_err(|e| BpError::DecompressionFailed(e.to_string()));
                }
            }
        }

        // Plain {"_delta":bool,"d":...} wrapper
        if let Some(delta_flag) = val.get("_delta").and_then(|v| v.as_bool()) {
            if delta_flag {
                let seq: DeltaSequence = serde_json::from_value(val["d"].clone())
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
                let records = reconstruct_sequence(&seq);
                return serde_json::to_vec(&records)
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()));
            } else {
                return serde_json::to_vec(&val["d"])
                    .map_err(|e| BpError::DecompressionFailed(e.to_string()));
            }
        }

        // Legacy fallback: plain DeltaSequence with no wrapper
        let seq: DeltaSequence = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;
        let records = reconstruct_sequence(&seq);
        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
