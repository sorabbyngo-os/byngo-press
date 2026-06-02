use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Minimum frequency for a term to enter the dictionary
const MIN_FREQUENCY: usize = 10;
/// Maximum dictionary size
const MAX_ENTRIES: usize = 65535;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dictionary {
    /// term → index
    pub term_to_idx: HashMap<String, u16>,
    /// index → term (for decompression)
    pub idx_to_term: Vec<String>,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            term_to_idx: HashMap::new(),
            idx_to_term: Vec::new(),
        }
    }

    pub fn get_idx(&self, term: &str) -> Option<u16> {
        self.term_to_idx.get(term).copied()
    }

    pub fn get_term(&self, idx: u16) -> Option<&str> {
        self.idx_to_term.get(idx as usize).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.idx_to_term.len()
    }

    pub fn size_bytes(&self) -> usize {
        self.idx_to_term.iter().map(|s| s.len()).sum()
    }
}

impl Default for Dictionary {
    fn default() -> Self { Self::new() }
}

/// Builds a domain dictionary from a sample of input data
pub struct DictionaryBuilder {
    frequencies: HashMap<String, usize>,
}

impl DictionaryBuilder {
    pub fn new() -> Self {
        Self { frequencies: HashMap::new() }
    }

    /// Feed a JSON record into the builder to count term frequencies
    pub fn feed(&mut self, record: &serde_json::Value) {
        match record {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    // Count field names
                    *self.frequencies.entry(key.clone()).or_insert(0) += 1;
                    // Count string values
                    if let serde_json::Value::String(s) = val {
                        if s.len() >= 4 && s.len() <= 64 {
                            *self.frequencies.entry(s.clone()).or_insert(0) += 1;
                        }
                    }
                    // Recurse into nested objects
                    self.feed(val);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    self.feed(item);
                }
            }
            serde_json::Value::String(s) => {
                if s.len() >= 4 && s.len() <= 64 {
                    *self.frequencies.entry(s.clone()).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }

    /// Build the final dictionary from accumulated frequencies
    pub fn build(self) -> Dictionary {
        let mut entries: Vec<(String, usize)> = self.frequencies
            .into_iter()
            .filter(|(term, freq)| {
                *freq >= MIN_FREQUENCY && term.len() >= 4
            })
            .collect();

        // Sort by frequency descending — most common terms get lowest indices
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(MAX_ENTRIES);

        let mut dict = Dictionary::new();
        for (i, (term, _freq)) in entries.iter().enumerate() {
            dict.term_to_idx.insert(term.clone(), i as u16);
            dict.idx_to_term.push(term.clone());
        }

        info!(
            entries   = dict.len(),
            size_bytes = dict.size_bytes(),
            "dictionary built"
        );

        dict
    }
}

impl Default for DictionaryBuilder {
    fn default() -> Self { Self::new() }
}
