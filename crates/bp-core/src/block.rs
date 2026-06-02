use serde::{Deserialize, Serialize};
use crate::profile::{Profile, Stage};
use crate::schema::Schema;

pub const MAGIC: &[u8; 8] = b"BYNGOPRS";
pub const FORMAT_VERSION: u8 = 1;

/// Universal block format — same across all profiles and devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Magic bytes: "BYNGOPRS"
    pub magic:           [u8; 8],
    /// Format version
    pub version:         u8,
    /// Profile used for compression
    pub profile:         Profile,
    /// Data schema
    pub schema:          Schema,
    /// Number of data blocks
    pub block_count:     u64,
    /// Total uncompressed size in bytes
    pub original_size:   u64,
    /// Total compressed size in bytes
    pub compressed_size: u64,
    /// Achieved compression ratio (original / compressed)
    pub ratio:           f64,
    /// Stages applied during compression
    pub stages_applied:  Vec<Stage>,
    /// Blake3 hash of the full compressed payload
    pub payload_hash:    [u8; 32],
    /// Creation timestamp (Unix epoch seconds)
    pub created_at:      i64,
}

/// Skip index entry — maps block number to byte offset in compressed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipEntry {
    /// Block number (0-indexed)
    pub block_id:            u64,
    /// Byte offset of this block in the compressed file
    pub offset:              u64,
    /// Compressed size of this block in bytes
    pub compressed_size:     u64,
    /// Uncompressed size of this block in bytes
    pub uncompressed_size:   u64,
    /// Original byte range this block covers
    pub original_start:      u64,
    pub original_end:        u64,
    /// Blake3 checksum of this block's compressed bytes
    pub checksum:            [u8; 32],
}

/// Full skip index — O(log n) random access into compressed data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipIndex {
    pub entries: Vec<SkipEntry>,
}

impl SkipIndex {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Find the block containing a given original byte offset
    pub fn find_block(&self, original_offset: u64) -> Option<&SkipEntry> {
        self.entries.iter().find(|e| {
            original_offset >= e.original_start && original_offset < e.original_end
        })
    }

    /// Binary search by block_id
    pub fn get(&self, block_id: u64) -> Option<&SkipEntry> {
        self.entries.binary_search_by_key(&block_id, |e| e.block_id)
            .ok()
            .map(|i| &self.entries[i])
    }

    pub fn push(&mut self, entry: SkipEntry) {
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for SkipIndex {
    fn default() -> Self { Self::new() }
}

/// A single compressed data block
#[derive(Debug)]
pub struct Block {
    pub id:   u64,
    pub data: Vec<u8>,
}
