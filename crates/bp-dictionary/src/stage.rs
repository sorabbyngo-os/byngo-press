use tracing::info;

use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::builder::DictionaryBuilder;
use crate::codec;

pub struct DictionaryStage;

/// Determine which records to feed the dictionary builder, and how to
/// restore them afterwards.
///
/// Returns (records, shell, records_key):
///   - shell is the original wrapper value (or the original value itself if
///     it has nothing else worth preserving) — None only for a bare array
///     with no wrapper at all.
///   - records_key is the field inside `shell` that held the records
///     ("u" for dedup frames, "d" for delta-only frames, "" if no shell).
///
/// If the wrapper indicates delta was applied (records_key's value is a
/// DeltaSequence object, not an array of records), records is empty and
/// the whole input should be passed through unchanged.
fn extract_records(input: &[u8]) -> BpResult<(Vec<serde_json::Value>, Option<serde_json::Value>, &'static str)> {
    let val: serde_json::Value = serde_json::from_slice(input)
        .map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })?;

    let (records_key, delta_applied) = if val.get("_dedup").is_some() {
        ("u", val.get("_delta").and_then(|v| v.as_bool()).unwrap_or(false))
    } else if val.get("_delta").is_some() {
        ("d", val.get("_delta").and_then(|v| v.as_bool()).unwrap_or(false))
    } else if let serde_json::Value::Array(arr) = &val {
        return Ok((arr.clone(), None, ""));
    } else {
        return Ok((vec![val], None, ""));
    };

    if delta_applied {
        // records_key's value is a DeltaSequence object — nothing to substitute
        return Ok((vec![], Some(val), records_key));
    }

    let refs = val[records_key].as_array()
        .ok_or_else(|| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: format!("missing '{}' in frame", records_key),
        })?
        .clone();

    Ok((refs, Some(val), records_key))
}

impl CompressionStage for DictionaryStage {
    fn stage(&self) -> Stage {
        Stage::DictionarySubstitution
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let (records, shell, records_key) = extract_records(input)?;

        if records.is_empty() {
            return Ok(input.to_vec());
        }

        let mut builder = DictionaryBuilder::new();
        for record in &records {
            builder.feed(record);
        }
        let dict = builder.build();

        let encoded: Vec<serde_json::Value> = records.iter()
            .map(|r| codec::encode(r, &dict))
            .collect();

        // Adaptive: only wrap in a dict frame if substitution actually shrinks
        // the data net of the dictionary's own vocabulary overhead.
        let encoded_size  = serde_json::to_string(&encoded).map(|s| s.len()).unwrap_or(usize::MAX);
        let original_size = serde_json::to_string(&records).map(|s| s.len()).unwrap_or(0);
        let dict_overhead  = serde_json::to_string(&dict).map(|s| s.len()).unwrap_or(0);

        if encoded_size + dict_overhead >= original_size {
            info!(
                entries = dict.len(),
                encoded_size, original_size, dict_overhead,
                adaptive_skip = true,
                "dictionary substitution skipped — no net benefit"
            );
            return Ok(input.to_vec());
        }

        info!(
            entries    = dict.len(),
            size_bytes = dict.size_bytes(),
            records    = records.len(),
            encoded_size, original_size, dict_overhead,
            "dictionary substitution stage"
        );

        let frame = serde_json::json!({
            "dictionary":   dict,
            "encoded_data": encoded,
            "shell":        shell,
            "records_key":  records_key,
        });

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        // Passthrough: not a dict frame — pass straight through
        if frame.get("dictionary").is_none() {
            return Ok(input.to_vec());
        }

        let dict: crate::builder::Dictionary =
            serde_json::from_value(frame["dictionary"].clone())
                .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let encoded: Vec<serde_json::Value> =
            serde_json::from_value(frame["encoded_data"].clone())
                .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let decoded: Result<Vec<_>, _> = encoded.iter()
            .map(|r| codec::decode(r, &dict))
            .collect();
        let records = decoded.map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let records_key = frame["records_key"].as_str().unwrap_or("");
        let shell = &frame["shell"];

        if !shell.is_null() && !records_key.is_empty() {
            let mut shell_obj = shell.clone();
            shell_obj[records_key] = serde_json::Value::Array(records);
            return serde_json::to_vec(&shell_obj)
                .map_err(|e| BpError::DecompressionFailed(e.to_string()));
        }

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
