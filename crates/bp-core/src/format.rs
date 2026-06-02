use blake3::Hasher;
use std::io::{Read, Write};

use crate::block::{BlockHeader, SkipIndex, MAGIC, FORMAT_VERSION};
use crate::error::{BpError, BpResult};

/// ByngoPress file layout:
/// [BlockHeader (bincode)] [SkipIndex (bincode)] [Block 0] [Block 1] ... [Block N]

pub fn write_header<W: Write>(
    w:      &mut W,
    header: &BlockHeader,
    index:  &SkipIndex,
) -> BpResult<()> {
    let header_bytes = bincode::serialize(header)
        .map_err(|e| BpError::Serialization(e.to_string()))?;
    let index_bytes  = bincode::serialize(index)
        .map_err(|e| BpError::Serialization(e.to_string()))?;

    // Write header length + header
    let hlen = (header_bytes.len() as u64).to_le_bytes();
    w.write_all(&hlen)?;
    w.write_all(&header_bytes)?;

    // Write index length + index
    let ilen = (index_bytes.len() as u64).to_le_bytes();
    w.write_all(&ilen)?;
    w.write_all(&index_bytes)?;

    Ok(())
}

pub fn read_header<R: Read>(r: &mut R) -> BpResult<(BlockHeader, SkipIndex)> {
    // Read header
    let mut hlen_buf = [0u8; 8];
    r.read_exact(&mut hlen_buf)?;
    let hlen = u64::from_le_bytes(hlen_buf) as usize;
    let mut header_bytes = vec![0u8; hlen];
    r.read_exact(&mut header_bytes)?;

    let header: BlockHeader = bincode::deserialize(&header_bytes)
        .map_err(|e| BpError::Serialization(e.to_string()))?;

    // Validate magic
    if &header.magic != MAGIC {
        return Err(BpError::InvalidMagic);
    }
    if header.version != FORMAT_VERSION {
        return Err(BpError::UnsupportedVersion(header.version));
    }

    // Read index
    let mut ilen_buf = [0u8; 8];
    r.read_exact(&mut ilen_buf)?;
    let ilen = u64::from_le_bytes(ilen_buf) as usize;
    let mut index_bytes = vec![0u8; ilen];
    r.read_exact(&mut index_bytes)?;

    let index: SkipIndex = bincode::deserialize(&index_bytes)
        .map_err(|e| BpError::Serialization(e.to_string()))?;

    Ok((header, index))
}

pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(data);
    *h.finalize().as_bytes()
}
