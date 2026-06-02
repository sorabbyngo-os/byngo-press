use bp_core::error::{BpError, BpResult};
use bp_core::pipeline::CompressionStage;
use bp_core::profile::Stage;
use bp_core::schema::Schema;

use crate::columnar::ColumnarFrame;

fn parse_records(input: &[u8]) -> Vec<serde_json::Value> {
    if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(input) {
        return arr;
    }
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(input) {
        return vec![val];
    }
    vec![]
}

pub struct ColumnarStage;

impl CompressionStage for ColumnarStage {
    fn stage(&self) -> Stage {
        Stage::ColumnarReorganization
    }

    fn compress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let records = parse_records(input);

        let frame = ColumnarFrame::from_records(&records)
            .map_err(|e| BpError::PipelineFailed {
                stage:  "columnar".into(),
                reason: e.to_string(),
            })?;

        serde_json::to_vec(&frame).map_err(|e| BpError::PipelineFailed {
            stage:  "columnar".into(),
            reason: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], _schema: Schema) -> BpResult<Vec<u8>> {
        let frame: ColumnarFrame = serde_json::from_slice(input)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        let records = frame.to_records()
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))?;

        serde_json::to_vec(&records)
            .map_err(|e| BpError::DecompressionFailed(e.to_string()))
    }
}
