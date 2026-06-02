use rayon::prelude::*;
use tracing::info;

use crate::capability::SimdCapability;
use crate::error::SimdError;

/// Block size for parallel compression
const PARALLEL_BLOCK: usize = 4 * 1024 * 1024; // 4MB

pub struct EntropyCodec {
    capability: SimdCapability,
}

impl EntropyCodec {
    pub fn new() -> Self {
        let capability = SimdCapability::detect();
        info!(simd = capability.name(), "entropy codec initialized");
        Self { capability }
    }

    pub fn with_capability(capability: SimdCapability) -> Self {
        Self { capability }
    }

    /// Compress using zstd at the appropriate level for detected SIMD
    /// For AVX2/AVX-512: parallel block compression
    /// For scalar/NEON: single-threaded
    pub fn compress(&self, input: &[u8]) -> Result<Vec<u8>, SimdError> {
        let level   = self.capability.zstd_level();
        let threads = self.capability.thread_count();

        if threads > 1 && input.len() > PARALLEL_BLOCK {
            self.compress_parallel(input, level, threads)
        } else {
            self.compress_single(input, level)
        }
    }

    pub fn decompress(&self, input: &[u8]) -> Result<Vec<u8>, SimdError> {
        let threads = self.capability.thread_count();

        if threads > 1 {
            self.decompress_parallel(input)
        } else {
            self.decompress_single(input)
        }
    }

    fn compress_single(&self, input: &[u8], level: i32) -> Result<Vec<u8>, SimdError> {
        zstd::encode_all(input, level)
            .map_err(|e| SimdError::CompressionFailed(e.to_string()))
    }

    fn compress_parallel(
        &self,
        input:   &[u8],
        level:   i32,
        threads: usize,
    ) -> Result<Vec<u8>, SimdError> {
        // Split into blocks, compress in parallel, concatenate with block table
        let blocks: Vec<&[u8]> = input.chunks(PARALLEL_BLOCK).collect();

        let compressed_blocks: Result<Vec<Vec<u8>>, SimdError> = blocks
            .par_iter()
            .map(|block| {
                zstd::encode_all(*block, level)
                    .map_err(|e| SimdError::CompressionFailed(e.to_string()))
            })
            .collect();

        let compressed_blocks = compressed_blocks?;

        // Build output: [block_count: u32] [block_sizes: u32 * N] [block_data...]
        let block_count = compressed_blocks.len() as u32;
        let mut out = Vec::new();
        out.extend_from_slice(&block_count.to_le_bytes());

        for block in &compressed_blocks {
            out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        }
        for block in &compressed_blocks {
            out.extend_from_slice(block);
        }

        info!(
            blocks       = block_count,
            threads      = threads,
            input_bytes  = input.len(),
            output_bytes = out.len(),
            ratio        = input.len() as f64 / out.len().max(1) as f64,
            "parallel entropy compression complete"
        );

        Ok(out)
    }

    fn decompress_single(&self, input: &[u8]) -> Result<Vec<u8>, SimdError> {
        zstd::decode_all(input)
            .map_err(|e| SimdError::DecompressionFailed(e.to_string()))
    }

    fn decompress_parallel(&self, input: &[u8]) -> Result<Vec<u8>, SimdError> {
        if input.len() < 4 {
            return self.decompress_single(input);
        }

        // Try to read block table
        let block_count = u32::from_le_bytes(
            input[..4].try_into().unwrap()
        ) as usize;

        // Sanity check — if block_count is huge it's probably single-stream
        if block_count > 10_000 || input.len() < 4 + block_count * 4 {
            return self.decompress_single(input);
        }

        let mut sizes = Vec::with_capacity(block_count);
        for i in 0..block_count {
            let offset = 4 + i * 4;
            let size = u32::from_le_bytes(
                input[offset..offset+4].try_into().unwrap()
            ) as usize;
            sizes.push(size);
        }

        let data_start = 4 + block_count * 4;
        let mut blocks = Vec::with_capacity(block_count);
        let mut pos = data_start;

        for size in &sizes {
            if pos + size > input.len() {
                // Malformed — fall back to single stream
                return self.decompress_single(input);
            }
            blocks.push(&input[pos..pos + size]);
            pos += size;
        }

        let decompressed: Result<Vec<Vec<u8>>, SimdError> = blocks
            .par_iter()
            .map(|block| {
                zstd::decode_all(*block)
                    .map_err(|e| SimdError::DecompressionFailed(e.to_string()))
            })
            .collect();

        let mut out = Vec::new();
        for block in decompressed? {
            out.extend_from_slice(&block);
        }
        Ok(out)
    }
}

impl Default for EntropyCodec {
    fn default() -> Self { Self::new() }
}
