use tracing::info;
use std::collections::HashMap;
use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;
use blake3::Hasher;

pub struct SemanticDedupStage;

fn blake3_hash(data: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize().to_hex().to_string()
}

impl CompressionStage for SemanticDedupStage {
    fn stage(&self) -> Stage {
        Stage::SemanticDedup
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let records: Vec<serde_json::Value> = serde_json::from_slice(input)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "semantic-dedup".into(),
                reason: e.to_string(),
            })?;

        let mut hash_to_idx: HashMap<String, u32> = HashMap::new();
        let mut unique_records: Vec<serde_json::Value> = Vec::new();
        let mut indices: Vec<u32> = Vec::with_capacity(records.len());
        let mut dupes = 0usize;

        for record in &records {
            let bytes = serde_json::to_vec(record)
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "semantic-dedup".into(),
                    reason: e.to_string(),
                })?;
            let hash = blake3_hash(&bytes);
            if let Some(&idx) = hash_to_idx.get(&hash) {
                indices.push(idx);
                dupes += 1;
            } else {
                let idx = unique_records.len() as u32;
                hash_to_idx.insert(hash, idx);
                unique_records.push(record.clone());
                indices.push(idx);
            }
        }

        let unique = unique_records.len();
        info!(
            records = records.len(),
            unique  = unique,
            dupes   = dupes,
            ratio   = format!("{:.1}x", records.len() as f64 / unique.max(1) as f64),
            "dedup stage complete"
        );

        // Adaptive: zero duplicates means the {_dedup, u, i} wrapper is pure
        // overhead (an extra u32 index array + JSON structure for no benefit).
        // Pass the original array through unchanged.
        if dupes == 0 {
            info!(adaptive_skip = true, "dedup skipped — no duplicates found");
            return Ok(input.to_vec());
        }

        let out = serde_json::json!({
            "_dedup": true,
            "u": unique_records,
            "i": indices,
        });
        serde_json::to_vec(&out).map_err(|e| BpError::PipelineFailed {
            stage:  "semantic-dedup".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let val: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // No _dedup wrapper — dedup was skipped at compress time, pass through
        if val.get("_dedup").is_none() {
            return Ok(input.to_vec());
        }

        let unique: Vec<serde_json::Value> = serde_json::from_value(val["u"].clone())
            .map_err(|e| BpError::DecompressionFailed(format!("missing u: {}", e)))?;
        let indices: Vec<u32> = serde_json::from_value(val["i"].clone())
            .map_err(|e| BpError::DecompressionFailed(format!("missing i: {}", e)))?;

        let mut records = Vec::with_capacity(indices.len());
        for idx in indices {
            let record = unique.get(idx as usize)
                .ok_or_else(|| BpError::DecompressionFailed(
                    format!("dedup index {} out of range (unique={})", idx, unique.len())
                ))?;
            records.push(record.clone());
        }

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
