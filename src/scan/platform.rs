use std::fs::Metadata;
use crate::tree::ExtendedInfo;

#[derive(Debug, Clone)]
pub struct PlatformMetadata {
    pub asize: i64,
    pub dsize: i64,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u32,
    pub extended: Option<ExtendedInfo>,
}

#[cfg(unix)]
pub fn get_metadata(meta: &Metadata, extended: bool) -> PlatformMetadata {
    use std::os::unix::fs::MetadataExt;

    let asize = meta.len() as i64;
    // On Unix, allocated size is blocks * 512
    let dsize = meta.blocks() as i64 * 512;
    let dev = meta.dev();
    let ino = meta.ino();
    let nlink = meta.nlink() as u32;

    let extended_info = if extended {
        Some(ExtendedInfo {
            mtime: meta.mtime(),
            uid: meta.uid(),
            gid: meta.gid(),
            mode: meta.mode(),
        })
    } else {
        None
    };

    PlatformMetadata {
        asize,
        dsize,
        dev,
        ino,
        nlink,
        extended: extended_info,
    }
}

#[cfg(windows)]
pub fn get_metadata(meta: &Metadata, extended: bool) -> PlatformMetadata {
    use std::os::windows::fs::MetadataExt;
    use std::time::UNIX_EPOCH;

    let asize = meta.len() as i64;
    // On Windows, fallback to apparent size or align to 4096 bytes block size
    let dsize = ((asize + 4095) / 4096) * 4096;

    // Use volume serial number and file index if available
    let dev = meta.volume_serial_number().unwrap_or(0) as u64;
    let ino = meta.file_index().unwrap_or(0);
    let nlink = meta.number_of_links().unwrap_or(1);

    let extended_info = if extended {
        let mtime = meta.modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Some(ExtendedInfo {
            mtime,
            uid: 0,
            gid: 0,
            mode: 0o644,
        })
    } else {
        None
    };

    PlatformMetadata {
        asize,
        dsize,
        dev,
        ino,
        nlink,
        extended: extended_info,
    }
}
