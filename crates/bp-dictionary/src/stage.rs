use serde::{Deserialize, Serialize};
use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::builder::{Dictionary, DictionaryBuilder};
use crate::codec;

#[derive(Debug, Serialize, Deserialize)]
struct DictFrame {
    dictionary:   Dictionary,
    encoded_data: Vec<u8>,
}

pub struct DictionaryStage;

fn extract_records(input: &[u8]) -> BpResult<(Vec<serde_json::Value>, Option<serde_json::Value>)> {
    let val: serde_json::Value = serde_json::from_slice(input)
        .map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })?;

    // Dedup frame — records live in _refs
    if val.get("_dedup").is_some() {
        let refs = val["_refs"].as_array()
            .ok_or_else(|| BpError::PipelineFailed {
                stage:  "dictionary".into(),
                reason: "missing _refs".into(),
            })?
            .clone();
        return Ok((refs, Some(val)));
    }

    if let serde_json::Value::Array(arr) = val {
        return Ok((arr, None));
    }

    Ok((vec![val], None))
}

impl CompressionStage for DictionaryStage {
    fn stage(&self) -> Stage {
        Stage::DictionarySubstitution
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let (records, dedup_frame) = extract_records(input)?;

        let mut builder = DictionaryBuilder::new();
        for record in &records {
            builder.feed(record);
        }
        let dict = builder.build();

        info!(
            entries    = dict.len(),
            size_bytes = dict.size_bytes(),
            records    = records.len(),
            "dictionary substitution stage"
        );

        let encoded: Vec<serde_json::Value> = records.iter()
            .map(|r| codec::encode(r, &dict))
            .collect();

        let encoded_data = serde_json::to_vec(&encoded)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "dictionary".into(),
                reason: e.to_string(),
            })?;

        // Embed DictFrame. If there's a dedup frame, store it inside so it
        // travels through delta and entropy and comes back intact on decompress.
        let frame = serde_json::json!({
            "dictionary":   dict,
            "encoded_data": encoded_data,
            "dedup_frame":  dedup_frame,  // null if no dedup frame
        });

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let dict: Dictionary = serde_json::from_value(frame["dictionary"].clone())
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let encoded_data: Vec<u8> = serde_json::from_value(frame["encoded_data"].clone())
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let encoded: Vec<serde_json::Value> = serde_json::from_slice(&encoded_data)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let decoded: Result<Vec<_>, _> = encoded.iter()
            .map(|r| codec::decode(r, &dict))
            .collect();
        let records = decoded.map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // Restore dedup frame if present — put decoded records back into _refs
        let dedup_frame = &frame["dedup_frame"];
        if !dedup_frame.is_null() {
            let mut df = dedup_frame.clone();
            df["_refs"] = serde_json::Value::Array(records);
            return serde_json::to_vec(&df)
                .map_err(|e| BpError::DecompressionFailed(e.to_string()));
        }

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
