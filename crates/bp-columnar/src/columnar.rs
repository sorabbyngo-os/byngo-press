use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

use crate::error::ColumnarError;

/// Columnar format — fields grouped by column name
/// Row-oriented:    [{a:1, b:"x"}, {a:2, b:"y"}, {a:3, b:"z"}]
/// Column-oriented: {a:[1,2,3], b:["x","y","z"]}
/// Column-oriented compresses 10-100x better because
/// similar values are adjacent → better entropy coding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnarFrame {
    /// column name → list of values (one per row)
    pub columns:   HashMap<String, Vec<Value>>,
    /// original row count
    pub row_count: usize,
    /// original column order for reconstruction
    pub col_order: Vec<String>,
}

impl ColumnarFrame {
    /// Convert row-oriented JSON records to columnar format
    pub fn from_records(records: &[Value]) -> Result<Self, ColumnarError> {
        if records.is_empty() {
            return Ok(Self {
                columns:   HashMap::new(),
                row_count: 0,
                col_order: vec![],
            });
        }

        // Collect all column names from first record
        let col_order: Vec<String> = records[0]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        let mut columns: HashMap<String, Vec<Value>> = col_order
            .iter()
            .map(|k| (k.clone(), Vec::with_capacity(records.len())))
            .collect();

        for record in records {
            match record.as_object() {
                Some(map) => {
                    for col in &col_order {
                        let val = map.get(col).cloned().unwrap_or(Value::Null);
                        columns.get_mut(col).unwrap().push(val);
                    }
                    // Handle extra columns not in first record
                    for (key, val) in map {
                        if !col_order.contains(key) {
                            columns
                                .entry(key.clone())
                                .or_insert_with(Vec::new)
                                .push(val.clone());
                        }
                    }
                }
                None => {
                    // Non-object record — store under "_value"
                    columns
                        .entry("_value".into())
                        .or_insert_with(Vec::new)
                        .push(record.clone());
                }
            }
        }

        let row_count = records.len();

        info!(
            rows    = row_count,
            columns = col_order.len(),
            "columnar reorganization complete"
        );

        Ok(Self { columns, row_count, col_order })
    }

    /// Reconstruct row-oriented records from columnar format
    pub fn to_records(&self) -> Result<Vec<Value>, ColumnarError> {
        if self.row_count == 0 {
            return Ok(vec![]);
        }

        let mut records = Vec::with_capacity(self.row_count);

        for i in 0..self.row_count {
            let mut map = serde_json::Map::new();
            for col in &self.col_order {
                if let Some(col_vals) = self.columns.get(col) {
                    if let Some(val) = col_vals.get(i) {
                        map.insert(col.clone(), val.clone());
                    }
                }
            }
            records.push(Value::Object(map));
        }

        Ok(records)
    }

    /// Compression benefit estimate
    /// Higher = more repetition within columns = better compression
    pub fn column_entropy_estimate(&self) -> HashMap<String, f64> {
        let mut estimates = HashMap::new();

        for (col, values) in &self.columns {
            let total = values.len() as f64;
            if total == 0.0 {
                estimates.insert(col.clone(), 0.0);
                continue;
            }

            // Count unique values
            let mut counts: HashMap<String, usize> = HashMap::new();
            for val in values {
                *counts.entry(val.to_string()).or_insert(0) += 1;
            }

            let unique = counts.len() as f64;
            // Lower unique ratio = more repetition = better compression
            let repetition_ratio = 1.0 - (unique / total);
            estimates.insert(col.clone(), repetition_ratio);
        }

        estimates
    }
}
