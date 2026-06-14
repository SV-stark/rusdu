pub mod bin_read;
pub mod bin_write;
pub mod compress;
pub mod json_read;
pub mod json_write;

use crate::tree::TreeArena;
use anyhow::Result;
use std::path::Path;

pub fn import_file(path: &Path) -> Result<TreeArena> {
    // Check magic bytes or extension to select JSON or binary format
    let file_bytes = compress::read_file_maybe_compressed(path)?;

    // Check if it starts with the binary format signature: "\xbfncduEX1"
    if file_bytes.starts_with(b"\xbfncduEX1") {
        bin_read::import_bin(&file_bytes)
    } else {
        json_read::import_json(&file_bytes)
    }
}

pub fn export_json(
    arena: &TreeArena,
    path: &Path,
    compress: bool,
    compress_level: i32,
) -> Result<()> {
    let json_bytes = json_write::export_json(arena)?;
    if compress {
        compress::write_compressed_file(path, &json_bytes, compress_level)
    } else {
        std::fs::write(path, json_bytes)?;
        Ok(())
    }
}

pub fn export_bin(
    arena: &TreeArena,
    path: &Path,
    block_size_kb: usize,
    compress_level: i32,
) -> Result<()> {
    let bin_bytes = bin_write::export_bin(arena, block_size_kb, compress_level)?;
    std::fs::write(path, bin_bytes)?;
    Ok(())
}
