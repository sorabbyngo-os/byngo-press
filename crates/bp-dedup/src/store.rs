use std::collections::HashMap;

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::DedupError;

/// A content-addressed chunk store
/// Key: Blake3 hash of chunk content (32 bytes → 64 hex chars)
/// Value: the chunk bytes
#[derive(Debug, Default)]
pub struct ChunkStore {
    /// hash → chunk bytes
    chunks:     HashMap<String, Vec<u8>>,
    /// Total bytes stored (before dedup)
    total_in:   u64,
    /// Unique bytes stored (after dedup)
    unique_out: u64,
    /// Number of duplicate chunks eliminated
    dupes:      u64,
}

impl ChunkStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a chunk, returns its hash reference
    /// If already stored, returns the existing hash (dedup hit)
    pub fn put(&mut self, chunk: &[u8]) -> String {
        let hash = blake3_hash(chunk);
        self.total_in += chunk.len() as u64;

        if self.chunks.contains_key(&hash) {
            self.dupes += 1;
            debug!(hash = %hash, bytes = chunk.len(), "dedup hit");
        } else {
            self.chunks.insert(hash.clone(), chunk.to_vec());
            self.unique_out += chunk.len() as u64;
            debug!(hash = %hash, bytes = chunk.len(), "chunk stored");
        }

        hash
    }

    /// Retrieve a chunk by hash
    pub fn get(&self, hash: &str) -> Option<&[u8]> {
        self.chunks.get(hash).map(|v| v.as_slice())
    }

    pub fn ratio(&self) -> f64 {
        if self.unique_out == 0 { return 1.0; }
        self.total_in as f64 / self.unique_out as f64
    }

    pub fn stats(&self) -> DedupStats {
        DedupStats {
            total_in:   self.total_in,
            unique_out: self.unique_out,
            dupes:      self.dupes,
            chunks:     self.chunks.len() as u64,
            ratio:      self.ratio(),
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Serialize the store for embedding in compressed output
    pub fn to_bytes(&self) -> Result<Vec<u8>, DedupError> {
        let map: HashMap<String, Vec<u8>> = self.chunks.clone();
        bincode_serialize(&map)
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, DedupError> {
        let chunks: HashMap<String, Vec<u8>> = bincode_deserialize(data)?;
        Ok(Self {
            unique_out: chunks.values().map(|v| v.len() as u64).sum(),
            chunks,
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupStats {
    pub total_in:   u64,
    pub unique_out: u64,
    pub dupes:      u64,
    pub chunks:     u64,
    pub ratio:      f64,
}

fn blake3_hash(data: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize().to_hex().to_string()
}

fn bincode_serialize<T: Serialize>(v: &T) -> Result<Vec<u8>, DedupError> {
    bincode::serialize(v).map_err(|e| DedupError::Serialization(
        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    ))
}

fn bincode_deserialize<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, DedupError> {
    bincode::deserialize(data).map_err(|e| DedupError::Serialization(
        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    ))
}
