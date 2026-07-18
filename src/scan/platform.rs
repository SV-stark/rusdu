use crate::tree::ExtendedInfo;
use std::fs::Metadata;

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
pub fn get_metadata(_path: &std::path::Path, meta: &Metadata, extended: bool) -> PlatformMetadata {
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
pub fn get_metadata(path: &std::path::Path, meta: &Metadata, extended: bool) -> PlatformMetadata {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::time::UNIX_EPOCH;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
    };

    let asize = meta.len() as i64;
    // On Windows, fallback to apparent size or align to 4096 bytes block size
    let dsize = ((asize + 4095) / 4096) * 4096;

    // Stable Windows implementation of volume serial number, file index, and link count
    let mut dev = 0u64;
    let mut ino = 0u64;
    let mut nlink = 1u32;

    // Open handle to query info stably via Windows handle
    if let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
    {
        let handle = file.as_raw_handle() as _;
        unsafe {
            let mut info = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();
            if GetFileInformationByHandle(handle, &mut info) != 0 {
                dev = info.dwVolumeSerialNumber as u64;
                ino = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
                nlink = info.nNumberOfLinks;
            }
        }
    }

    let extended_info = if extended {
        let mtime = meta
            .modified()
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
