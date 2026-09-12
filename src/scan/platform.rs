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
pub fn fix_path(path: &std::path::Path) -> std::path::PathBuf {
    if path.is_absolute() {
        let path_str = path.to_string_lossy();
        if !path_str.starts_with(r"\\?\") && !path_str.starts_with(r"\\.\") {
            let clean = path_str.replace('/', "\\");
            return std::path::PathBuf::from(format!(r"\\?\{}", clean));
        }
    }
    path.to_path_buf()
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
    let asize = meta.len() as i64;
    let dsize = if cluster_size > 0 {
        ((asize as u64).div_ceil(cluster_size) * cluster_size) as i64
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
