use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;
use tracing::info;

use crate::encoders::*;

pub struct DomainEncodingStage;

/// Extract records from whatever the previous stage output
fn extract_records(input: &[u8]) -> BpResult<Vec<serde_json::Value>> {
    let val: serde_json::Value = serde_json::from_slice(input)
        .map_err(|e| BpError::PipelineFailed {
            stage:  "domain-encoding".into(),
            reason: e.to_string(),
        })?;

    // Dedup frame
    if val.get("_dedup").is_some() {
        let refs = val["_refs"].as_array()
            .ok_or_else(|| BpError::PipelineFailed {
                stage:  "domain-encoding".into(),
                reason: "missing _refs in dedup frame".into(),
            })?;
        return Ok(refs.clone());
    }

    // Plain array
    if let serde_json::Value::Array(arr) = val {
        return Ok(arr);
    }

    // Single object
    Ok(vec![val])
}

impl CompressionStage for DomainEncodingStage {
    fn stage(&self) -> Stage {
        Stage::DomainEncoding
    }

    fn compress(&self, input: &[u8], schema: Schema) -> BpResult<Vec<u8>> {
        let records = extract_records(input)?;

        info!(records = records.len(), "domain encoding");

        let encoded: Vec<serde_json::Value> = records.iter()
            .map(|r| encode_record(r.clone(), schema))
            .collect();

        serde_json::to_vec(&encoded).map_err(|e| BpError::PipelineFailed {
            stage:  "domain-encoding".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        Ok(input.to_vec())
    }
}

fn encode_record(mut record: serde_json::Value, schema: Schema) -> serde_json::Value {
    // Skip ref markers from dedup
    if record.get("_ref").is_some() {
        return record;
    }
    if let serde_json::Value::Object(ref mut map) = record {
        let keys: Vec<String> = map.keys().cloned().collect();
        for key in keys {
            let val = match map.get(&key) {
                Some(v) => v.clone(),
                None    => continue,
            };
            if let serde_json::Value::String(ref s) = val {
                if let Some(new_val) = encode_field(&key, s, schema) {
                    map.insert(key, new_val);
                }
            }
        }
    }
    record
}

fn encode_field(key: &str, value: &str, _schema: Schema) -> Option<serde_json::Value> {
    if key.contains("ip") || key.ends_with("_ip") {
        if let Ok(bytes) = encode_ipv4(value) {
            return Some(serde_json::json!({ "_bp": "ipv4", "v": hex::encode(bytes) }));
        }
        if let Ok(bytes) = encode_ipv6(value) {
            return Some(serde_json::json!({ "_bp": "ipv6", "v": hex::encode(bytes) }));
        }
    }
    if key.contains("_id") || key == "uuid" {
        if value.len() == 36 && value.chars().filter(|&c| c == '-').count() == 4 {
            if let Ok(bytes) = encode_uuid(value) {
                return Some(serde_json::json!({ "_bp": "uuid", "v": hex::encode(bytes) }));
            }
        }
    }
    if key == "severity" || key == "level" || key == "priority" {
        if let Some(b) = encode_severity(value) {
            return Some(serde_json::json!({ "_bp": "severity", "v": b }));
        }
    }
    if key.contains("cve") || value.starts_with("CVE-") {
        if let Ok(bytes) = encode_cve(value) {
            return Some(serde_json::json!({ "_bp": "cve", "v": hex::encode(bytes) }));
        }
    }
    if key.contains("mac") {
        if let Ok(bytes) = encode_mac(value) {
            return Some(serde_json::json!({ "_bp": "mac", "v": hex::encode(bytes) }));
        }
    }
    if key.contains("class") || key.contains("clearance") {
        if let Some(b) = encode_classification(value) {
            return Some(serde_json::json!({ "_bp": "classification", "v": b }));
        }
    }
    None
}
