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

    let asize = i64::try_from(meta.len()).unwrap_or(crate::tree::MAX_SIZE_LIMIT);
    // On Unix, allocated size is blocks * 512. Saturate rather than overflow:
    // `blocks() * 512` exceeds i64 only beyond ~8 EiB, but wrapping negative
    // would be silently clamped to 0 by `TreeNode::new_file`.
    let dsize = (meta.blocks() as i64)
        .saturating_mul(512)
        .clamp(0, crate::tree::MAX_SIZE_LIMIT);
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

/// Resolve `.` and `..` components in an absolute Windows path.
///
/// Works on the already-backslash-normalized string rather than via
/// `std::path::Component`, so no filesystem access is attempted.
#[cfg(windows)]
fn lexical_normalize(path: &str) -> String {
    // Preserve a leading `\\server\share` or `\\?\` prefix verbatim.
    let (prefix, rest) = if let Some(r) = path.strip_prefix(r"\\?\") {
        (r"\\?\", r)
    } else if let Some(r) = path.strip_prefix("\\\\") {
        (r"\\", r)
    } else if let Some(r) = path.strip_prefix(r"\\.\") {
        (r"\\.\", r)
    } else {
        ("", path)
    };

    let mut out: Vec<&str> = Vec::new();
    for part in rest.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                // Never pop past a root or share name.
                if out.len() > 1 || (out.len() == 1 && !has_root_marker(out[0])) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    format!("{}\\{}", prefix, out.join("\\"))
}

/// True when a single path component names a drive or share root, which must
/// never be popped by a following `..`.
#[cfg(windows)]
fn has_root_marker(component: &str) -> bool {
    component.ends_with(':') || component == "." || component == ".."
}

#[cfg(windows)]
pub fn fix_path(path: &std::path::Path) -> std::path::PathBuf {
    if !path.is_absolute() {
        return path.to_path_buf();
    }
    let path_str = path.to_string_lossy();
    if path_str.starts_with(r"\\?\") || path_str.starts_with(r"\\.\") {
        return path.to_path_buf();
    }
    let clean = path_str.replace('/', "\\");
    // A UNC path must keep its `UNC` marker under the `\\?\` prefix:
    // `\\server\share` becomes `\\?\UNC\server\share`. Prefixing it directly
    // yields `\\?\\server\share`, which every Win32 call rejects with
    // ERROR_INVALID_NAME.
    if let Some(unc) = clean.strip_prefix(r"\\") {
        if !unc.is_empty() {
            return std::path::PathBuf::from(format!(r"\\?\UNC\{}", unc));
        }
    }
    // The `\\?\` prefix disables Win32 `.`/`..` normalization, so those
    // components have to be resolved lexically or the path becomes unusable.
    std::path::PathBuf::from(format!(r"\\?\{}", lexical_normalize(&clean)))
}

#[cfg(not(windows))]
pub fn fix_path(path: &std::path::Path) -> std::path::PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn get_drive_cluster_size_and_dev(path: &std::path::Path) -> (u64, u64) {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::sync::RwLock;
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceW, GetVolumeInformationW};

    static CACHE: std::sync::LazyLock<RwLock<HashMap<std::path::PathBuf, (u64, u64)>>> =
        std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

    let root = path
        .components()
        .next()
        .map(|c| std::path::PathBuf::from(c.as_os_str()))
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\"));
    let mut root_str = root.to_string_lossy().into_owned();
    if !root_str.ends_with('\\') && !root_str.ends_with('/') {
        root_str.push('\\');
    }
    let root_path = std::path::PathBuf::from(&root_str);

    if let Ok(read_guard) = CACHE.read() {
        if let Some(&val) = read_guard.get(&root_path) {
            return val;
        }
    }

    let clean_root = root_str.strip_prefix(r"\\?\").unwrap_or(&root_str);
    let wide_root: Vec<u16> = clean_root
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut sectors_per_cluster = 0u32;
    let mut bytes_per_sector = 0u32;
    let mut number_of_free_clusters = 0u32;
    let mut total_number_of_clusters = 0u32;
    let mut cluster_size = 4096u64;

    // SAFETY: wide_root is a null-terminated UTF-16 string and the out-pointers point to valid mutable u32 integers.
    let ok = unsafe {
        GetDiskFreeSpaceW(
            wide_root.as_ptr(),
            &mut sectors_per_cluster,
            &mut bytes_per_sector,
            &mut number_of_free_clusters,
            &mut total_number_of_clusters,
        )
    };
    if ok != 0 && sectors_per_cluster > 0 && bytes_per_sector > 0 {
        cluster_size = (sectors_per_cluster as u64) * (bytes_per_sector as u64);
    }

    let mut serial_num = 0u32;
    // SAFETY: wide_root is null-terminated and &mut serial_num is a valid pointer to u32.
    let ok = unsafe {
        GetVolumeInformationW(
            wide_root.as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut serial_num,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        root_str.to_uppercase().hash(&mut hasher);
        serial_num = hasher.finish() as u32;
    }

    let res = (cluster_size, serial_num as u64);
    if let Ok(mut write_guard) = CACHE.write() {
        write_guard.insert(root_path, res);
    }
    res
}

#[cfg(windows)]
pub fn get_metadata(path: &std::path::Path, meta: &Metadata, extended: bool) -> PlatformMetadata {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::time::UNIX_EPOCH;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        GetFileInformationByHandle,
    };

    let (cluster_size, root_dev) = get_drive_cluster_size_and_dev(path);
    let asize = i64::try_from(meta.len()).unwrap_or(crate::tree::MAX_SIZE_LIMIT);
    // Round up to a whole cluster, saturating at the representable maximum so
    // an enormous size cannot wrap negative and be clamped to 0 downstream.
    let dsize = if cluster_size > 0 {
        ((asize as u64)
            .div_ceil(cluster_size)
            .saturating_mul(cluster_size)
            .min(crate::tree::MAX_SIZE_LIMIT as u64)) as i64
    } else {
        asize
    };

    let mut dev = root_dev;
    let mut ino = 0u64;
    let mut nlink = 1u32;

    // Query file handle if extended metadata, inode, or hard link information is needed
    if extended {
        let mut opts = std::fs::OpenOptions::new();
        opts.access_mode(FILE_READ_ATTRIBUTES);
        opts.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        if let Ok(file) = opts.open(path) {
            let handle = file.as_raw_handle() as _;
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            // SAFETY: file is a valid open Windows file handle and info points to valid uninitialized memory.
            let ok = unsafe { GetFileInformationByHandle(handle, info.as_mut_ptr()) };
            if ok != 0 {
                // SAFETY: GetFileInformationByHandle succeeded, so info is fully initialized.
                let info = unsafe { info.assume_init() };
                dev = info.dwVolumeSerialNumber as u64;
                ino = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
                nlink = info.nNumberOfLinks;
            }
        }
    }

    let extended_info = if extended {
        // `duration_since(..).ok()` maps every pre-1970 timestamp to 0, so a
        // file from 1960 was displayed as 1970-01-01 and was then discarded by
        // the `max()` in `recalculate_stats`. Negate the error duration instead.
        let mtime = meta
            .modified()
            .ok()
            .map(|t| match t.duration_since(UNIX_EPOCH) {
                Ok(d) => d.as_secs() as i64,
                Err(e) => -(e.duration().as_secs() as i64),
            })
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
