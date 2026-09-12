use crate::ui::DriveInfo;

#[cfg(windows)]
pub fn get_system_drives() -> Vec<DriveInfo> {
    use std::path::PathBuf;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetLogicalDrives, GetVolumeInformationW,
    };

    let mut drives = Vec::new();
    // SAFETY: GetLogicalDrives has no parameters and queries system drive bitmask.
    let bitmask = unsafe { GetLogicalDrives() };
    for i in 0..26 {
        if (bitmask & (1 << i)) != 0 {
            let drive_letter = (b'A' + i) as char;
            let wide_root: [u16; 4] = [drive_letter as u16, b':' as u16, b'\\' as u16, 0];
            let root_str = format!("{}:\\", drive_letter);

            let mut free_bytes_available: u64 = 0;
            let mut total_number_of_bytes: u64 = 0;
            let mut total_number_of_free_bytes: u64 = 0;

            // SAFETY: wide_root is null-terminated and out pointers are valid mutable pointers.
            let ok = unsafe {
                GetDiskFreeSpaceExW(
                    wide_root.as_ptr(),
                    &mut free_bytes_available as *mut u64,
                    &mut total_number_of_bytes as *mut u64,
                    &mut total_number_of_free_bytes as *mut u64,
                )
            };

            if ok != 0 {
                let mut vol_name = [0u16; 260];
                let mut fs_name = [0u16; 260];
                let mut serial_num = 0u32;
                let mut max_comp_len = 0u32;
                let mut flags = 0u32;

                // SAFETY: wide_root is null-terminated and output buffers have matching lengths.
                let _ = unsafe {
                    GetVolumeInformationW(
                        wide_root.as_ptr(),
                        vol_name.as_mut_ptr(),
                        vol_name.len() as u32,
                        &mut serial_num,
                        &mut max_comp_len,
                        &mut flags,
                        fs_name.as_mut_ptr(),
                        fs_name.len() as u32,
                    )
                };

                let len = vol_name.iter().position(|&c| c == 0).unwrap_or(0);
                let label = String::from_utf16_lossy(&vol_name[..len]);
                let name = if label.is_empty() {
                    format!("Local Disk ({}:)", drive_letter)
                } else {
                    format!("{} ({}:)", label, drive_letter)
                };

                drives.push(DriveInfo {
                    name,
                    mount_point: PathBuf::from(root_str),
                    total_space: total_number_of_bytes,
                    available_space: free_bytes_available,
                });
            }
        }
    }
    drives
}

#[cfg(target_os = "linux")]
pub fn get_system_drives() -> Vec<DriveInfo> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;

    let mut drives = Vec::new();
    if let Ok(file) = File::open("/proc/mounts").or_else(|_| File::open("/proc/self/mounts")) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let dev = parts[0];
                let mount_point = parts[1];
                let fstype = parts[2];

                // Include physical / standard mounts
                if dev.starts_with("/dev/") || fstype == "zfs" || fstype == "btrfs" {
                    if let Ok(c_path) = std::ffi::CString::new(mount_point) {
                        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
                        // SAFETY: c_path is a valid null-terminated C string and stat points to valid uninitialized memory.
                        let res = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
                        if res == 0 {
                            // SAFETY: libc::statvfs returned 0 indicating stat is fully initialized.
                            let stat = unsafe { stat.assume_init() };
                            let total_space =
                                (stat.f_blocks as u64).saturating_mul(stat.f_frsize as u64);
                            let available_space =
                                (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64);
                            drives.push(DriveInfo {
                                name: format!("{} ({})", mount_point, dev),
                                mount_point: PathBuf::from(mount_point),
                                total_space,
                                available_space,
                            });
                        }
                    }
                }
            }
        }
    }
    drives
}

#[cfg(target_os = "macos")]
pub fn get_system_drives() -> Vec<DriveInfo> {
    use std::path::PathBuf;
    let mut drives = Vec::new();
    let mut mntbuf: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo writes an array of statfs structures to mntbuf and returns the count.
    let count = unsafe { libc::getmntinfo(&mut mntbuf, libc::MNT_NOWAIT) };
    if count > 0 && !mntbuf.is_null() {
        // SAFETY: mntbuf is non-null and valid for count elements as returned by getmntinfo.
        let entries = unsafe { std::slice::from_raw_parts(mntbuf, count as usize) };
        for entry in entries {
            // SAFETY: f_mntonname and f_mntfromname are null-terminated byte arrays in struct statfs.
            let mount_point = unsafe { std::ffi::CStr::from_ptr(entry.f_mntonname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            let dev = unsafe { std::ffi::CStr::from_ptr(entry.f_mntfromname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            if dev.starts_with("/dev/") {
                let total_space = (entry.f_blocks as u64).saturating_mul(entry.f_bsize as u64);
                let available_space = (entry.f_bavail as u64).saturating_mul(entry.f_bsize as u64);
                drives.push(DriveInfo {
                    name: format!("{} ({})", mount_point, dev),
                    mount_point: PathBuf::from(mount_point),
                    total_space,
                    available_space,
                });
            }
        }
    }
    drives
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn get_system_drives() -> Vec<DriveInfo> {
    Vec::new()
}
