use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;
use tracing::debug;

use crate::encoders::*;

pub struct DomainEncodingStage;

fn unwrap_data(input: &[u8]) -> Vec<u8> {
    // If input is a dedup frame, extract the data field
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(input) {
        if let Some(data) = val.get("data") {
            if let Some(s) = data.as_str() {
                if let Ok(bytes) = serde_json::from_str::<Vec<u8>>(s) {
                    return bytes;
                }
            }
            // data field contains the raw bytes as array
            if let Ok(bytes) = serde_json::from_value::<Vec<u8>>(data.clone()) {
                return bytes;
            }
        }
    }
    input.to_vec()
}

fn parse_records(input: &[u8]) -> Vec<serde_json::Value> {
    // Try array
    if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(input) {
        return arr;
    }
    // Try object with data field (dedup frame)
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(input) {
        if let Some(data_val) = val.get("data") {
            if let Ok(arr) = serde_json::from_value::<Vec<serde_json::Value>>(data_val.clone()) {
                return arr;
            }
            // data is raw bytes
            if let Some(arr_bytes) = data_val.as_array() {
                let bytes: Vec<u8> = arr_bytes.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) {
                    return arr;
                }
            }
        }
        // Single object
        return vec![val];
    }
    vec![]
}

impl CompressionStage for DomainEncodingStage {
    fn stage(&self) -> Stage {
        Stage::DomainEncoding
    }

    fn compress(&self, input: &[u8], schema: Schema) -> BpResult<Vec<u8>> {
        let records = parse_records(input);

        if records.is_empty() {
            return Ok(input.to_vec());
        }

        debug!(records = records.len(), "domain encoding");

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
    if key.contains("ip") || key.contains("addr") || key.ends_with("_ip") {
        if let Ok(bytes) = encode_ipv4(value) {
            return Some(serde_json::json!({ "_bp": "ipv4", "v": hex::encode(bytes) }));
        }
        if let Ok(bytes) = encode_ipv6(value) {
            return Some(serde_json::json!({ "_bp": "ipv6", "v": hex::encode(bytes) }));
        }
    }
    if key.contains("id") || key.contains("uuid") {
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
