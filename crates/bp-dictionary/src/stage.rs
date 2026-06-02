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

fn parse_records(input: &[u8]) -> Vec<serde_json::Value> {
    if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(input) {
        return arr;
    }
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(input) {
        return vec![val];
    }
    vec![]
}

impl CompressionStage for DictionaryStage {
    fn stage(&self) -> Stage {
        Stage::DictionarySubstitution
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let records = parse_records(input);

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

        let frame = DictFrame { dictionary: dict, encoded_data };

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "dictionary".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: DictFrame = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let encoded: Vec<serde_json::Value> = serde_json::from_slice(&frame.encoded_data)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let decoded: Result<Vec<_>, _> = encoded.iter()
            .map(|r| codec::decode(r, &frame.dictionary))
            .collect();

        let records = decoded.map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
