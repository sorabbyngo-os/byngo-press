use serde::{Deserialize, Serialize};

/// A single field delta — what changed between two records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldDelta {
    /// Field unchanged — omit from delta
    Same,
    /// Field changed — store new value
    Changed(serde_json::Value),
    /// Field added
    Added(serde_json::Value),
    /// Field removed
    Removed,
}

/// Delta between two JSON records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDelta {
    /// Index of the base record this delta applies to
    pub base_idx: usize,
    /// Per-field deltas (only changed fields)
    pub fields:   std::collections::HashMap<String, FieldDelta>,
}

impl RecordDelta {
    /// Compute delta between base and target records
    pub fn compute(
        base_idx: usize,
        base:     &serde_json::Value,
        target:   &serde_json::Value,
    ) -> Self {
        let mut fields = std::collections::HashMap::new();

        let base_obj   = base.as_object();
        let target_obj = target.as_object();

        match (base_obj, target_obj) {
            (Some(b), Some(t)) => {
                // Fields in target
                for (key, tval) in t {
                    match b.get(key) {
                        None       => { fields.insert(key.clone(), FieldDelta::Added(tval.clone())); }
                        Some(bval) => {
                            if bval != tval {
                                fields.insert(key.clone(), FieldDelta::Changed(tval.clone()));
                            }
                            // Same → omit (implicit)
                        }
                    }
                }
                // Fields removed from base
                for key in b.keys() {
                    if !t.contains_key(key) {
                        fields.insert(key.clone(), FieldDelta::Removed);
                    }
                }
            }
            _ => {
                // Non-object records — store full value as change
                fields.insert(
                    "_value".into(),
                    FieldDelta::Changed(target.clone()),
                );
            }
        }

        Self { base_idx, fields }
    }

    /// Apply this delta to a base record to reconstruct target
    pub fn apply(&self, base: &serde_json::Value) -> serde_json::Value {
        let mut result = base.clone();

        if let serde_json::Value::Object(ref mut map) = result {
            for (key, delta) in &self.fields {
                match delta {
                    FieldDelta::Same                => {}
                    FieldDelta::Changed(v)          => { map.insert(key.clone(), v.clone()); }
                    FieldDelta::Added(v)            => { map.insert(key.clone(), v.clone()); }
                    FieldDelta::Removed             => { map.remove(key); }
                }
            }
        }

        result
    }

    /// How many fields changed
    pub fn changed_count(&self) -> usize {
        self.fields.values().filter(|d| !matches!(d, FieldDelta::Same)).count()
    }
}

/// Compute deltas for a sequence of records
/// First record stored as-is (base), subsequent records stored as deltas
pub fn compute_sequence(records: &[serde_json::Value]) -> DeltaSequence {
    if records.is_empty() {
        return DeltaSequence { base: vec![], deltas: vec![] };
    }

    let base   = records[0].clone();
    let mut deltas = Vec::with_capacity(records.len().saturating_sub(1));

    let mut prev = &records[0];

    for (i, record) in records[1..].iter().enumerate() {
        let delta = RecordDelta::compute(i, prev, record);
        deltas.push(delta);
        prev = record;
    }

    DeltaSequence { base: vec![base], deltas }
}

/// Reconstruct all records from a delta sequence
pub fn reconstruct_sequence(seq: &DeltaSequence) -> Vec<serde_json::Value> {
    if seq.base.is_empty() {
        return vec![];
    }

    let mut records = vec![seq.base[0].clone()];
    let prev    = &seq.base[0];

    // We need to hold reconstructed records to use as base for next
    let mut reconstructed_prev: Option<serde_json::Value> = None;

    for delta in &seq.deltas {
        let base   = reconstructed_prev.as_ref().unwrap_or(prev);
        let record = delta.apply(base);
        records.push(record.clone());
        reconstructed_prev = Some(record);
    }

    records
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSequence {
    pub base:   Vec<serde_json::Value>,
    pub deltas: Vec<RecordDelta>,
}
