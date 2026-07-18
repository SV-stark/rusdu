use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub fn read_file_maybe_compressed(path: &Path) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    if path == Path::new("-") {
        std::io::stdin()
            .read_to_end(&mut buffer)
            .context("Failed to read from stdin")?;
    } else {
        let mut file =
            File::open(path).with_context(|| format!("Failed to open file {:?}", path))?;
        file.read_to_end(&mut buffer)?;
    }

    // Check if the file starts with the Zstandard magic header (0x28B52FFD)
    if buffer.len() >= 4 && buffer[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        let decompressed =
            zstd::stream::decode_all(&buffer[..]).context("Failed to decompress Zstandard file")?;
        return Ok(decompressed);
    }

    Ok(buffer)
}

pub fn write_compressed_file(path: &Path, data: &[u8], level: i32) -> Result<()> {
    if path == Path::new("-") {
        let mut encoder = zstd::stream::Encoder::new(std::io::stdout(), level)
            .context("Failed to initialize Zstandard encoder for stdout")?;
        encoder.write_all(data)?;
        encoder.finish()?;
    } else {
        let file =
            File::create(path).with_context(|| format!("Failed to create file {:?}", path))?;
        let mut encoder = zstd::stream::Encoder::new(file, level)
            .context("Failed to initialize Zstandard encoder")?;
        encoder.write_all(data)?;
        encoder.finish()?;
    }
    Ok(())
}
