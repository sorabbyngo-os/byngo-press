use tracing::info;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::builder::DictionaryBuilder;
use crate::codec;

pub struct DictionaryStage;

fn extract_records(input: &[u8]) -> BpResult<(Vec<serde_json::Value>, Option<serde_json::Value>)> {
    let val: serde_json::Value = serde_json::from_slice(input)
        .map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })?;

    if val.get("_dedup").is_some() {
        // If "u" is a string, delta already ran and encoded unique records as b64.
        // Dictionary cannot peek inside a b64 blob — pass the whole frame through as-is.
        if val["u"].is_string() {
            // Shell carries everything; empty record list = no substitution
            return Ok((vec![], Some(val)));
        }
        // "u" is a plain array — extract records for dictionary substitution
        let refs = val["u"].as_array()
            .ok_or_else(|| BpError::PipelineFailed {
                stage:  "dictionary".into(),
                reason: "missing u in dedup frame".into(),
            })?
            .clone();
        // Shell carries "i" and "_dedup"; "u" will be replaced with encoded records
        let shell = serde_json::json!({ "_dedup": true, "i": val["i"] });
        return Ok((refs, Some(shell)));
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
        let (records, dedup_shell) = extract_records(input)?;

        // Empty records means the frame should pass through unchanged (e.g. post-delta b64)
        if records.is_empty() {
            if let Some(shell) = dedup_shell {
                return serde_json::to_vec(&shell).map_err(|e| BpError::PipelineFailed {
                    stage:  "dictionary".into(),
                    reason: e.to_string(),
                });
            }
        }

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

        let encoded_bytes = serde_json::to_vec(&encoded)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "dictionary".into(),
                reason: e.to_string(),
            })?;

        // Store as base64 string — 33% overhead vs 200-300% for JSON int array
        let encoded_b64 = B64.encode(&encoded_bytes);

        let frame = serde_json::json!({
            "dictionary":   dict,
            "encoded_data": encoded_b64,
            "dedup_shell":  dedup_shell,
        });

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // Passthrough: if this is a dedup frame with no dictionary, pass straight through
        if frame.get("_dedup").is_some() && frame.get("dictionary").is_none() {
            return Ok(input.to_vec());
        }

        let dict: crate::builder::Dictionary =
            serde_json::from_value(frame["dictionary"].clone())
                .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let encoded_b64 = frame["encoded_data"].as_str()
            .ok_or_else(|| BpError::DecompressionFailed("encoded_data not a string".into()))?;

        let encoded_bytes = B64.decode(encoded_b64)
            .map_err(|e| BpError::DecompressionFailed(format!("base64 decode: {}", e)))?;

        let encoded: Vec<serde_json::Value> = serde_json::from_slice(&encoded_bytes)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let decoded: Result<Vec<_>, _> = encoded.iter()
            .map(|r| codec::decode(r, &dict))
            .collect();
        let records = decoded.map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let dedup_shell = &frame["dedup_shell"];
        if !dedup_shell.is_null() {
            let out = serde_json::json!({
                "_dedup": true,
                "u": records,
                "i": dedup_shell["i"],
            });
            return serde_json::to_vec(&out)
                .map_err(|e| BpError::DecompressionFailed(e.to_string()));
        }

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
