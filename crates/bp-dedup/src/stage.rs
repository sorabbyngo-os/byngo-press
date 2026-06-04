use tracing::info;
use std::collections::HashMap;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use blake3::Hasher;

/// Dedup works at the record level — identical records are stored once
/// Output is still a JSON array, but with duplicate records replaced by refs
/// Format: array where each element is either the record or {"_ref": "hash"}

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

        // store: hash → record (unique records only)
        let mut store: HashMap<String, serde_json::Value> = HashMap::new();
        let mut refs:  Vec<serde_json::Value> = Vec::with_capacity(records.len());
        let mut dupes  = 0usize;

        for record in &records {
            let bytes = serde_json::to_vec(record)
                .map_err(|e| BpError::PipelineFailed {
                    stage:  "semantic-dedup".into(),
                    reason: e.to_string(),
                })?;
            let hash = blake3_hash(&bytes);

            if !store.contains_key(&hash) {
                store.insert(hash.clone(), record.clone());
                refs.push(record.clone()); // first occurrence — keep full record
            } else {
                dupes += 1;
                // Replace duplicate with a reference
                refs.push(serde_json::json!({ "_ref": &hash }));
            }
        }

        let unique = store.len();
        let ratio  = records.len() as f64 / unique.max(1) as f64;

        info!(
            records = records.len(),
            unique  = unique,
            dupes   = dupes,
            ratio   = format!("{:.1}x", ratio),
            "dedup stage complete"
        );

        // Output: {"_store": {hash: record}, "_refs": [...]}
        // Still serializable as a known structure downstream stages unwrap
        let out = serde_json::json!({
            "_dedup": true,
            "_store": store,
            "_refs":  refs,
        });

        serde_json::to_vec(&out).map_err(|e| BpError::PipelineFailed {
            stage:  "semantic-dedup".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let val: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let store = val["_store"].as_object()
            .ok_or_else(|| BpError::DecompressionFailed("missing _store".into()))?;
        let refs  = val["_refs"].as_array()
            .ok_or_else(|| BpError::DecompressionFailed("missing _refs".into()))?;

        let mut records = Vec::with_capacity(refs.len());
        for r in refs {
            if let Some(hash) = r.get("_ref").and_then(|v| v.as_str()) {
                let record = store.get(hash)
                    .ok_or_else(|| BpError::DecompressionFailed(
                        format!("ref not found: {}", hash)
                    ))?;
                records.push(record.clone());
            } else {
                records.push(r.clone());
            }
        }

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
