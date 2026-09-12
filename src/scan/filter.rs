use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct Filter {
    exclude_patterns: GlobSet,
    exclude_caches: bool,
    exclude_kernfs: bool,
}

impl Filter {
    pub fn new(
        exclude_strs: &[String],
        exclude_from: Option<&Path>,
        exclude_caches: bool,
        exclude_kernfs: bool,
    ) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();

        // Compile CLI patterns
        for pat_str in exclude_strs {
            if let Ok(glob) = Glob::new(pat_str) {
                builder.add(glob);
            }
        }

        // Compile patterns from file
        if let Some(file_path) = exclude_from {
            match File::open(file_path) {
                Ok(file) => {
                    let reader = std::io::BufReader::new(file);
                    for line in std::io::BufRead::lines(reader) {
                        let line = line?;
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('#') {
                            if let Ok(glob) = Glob::new(trimmed) {
                                builder.add(glob);
                            }
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        }

        let exclude_patterns = builder.build().unwrap_or_else(|_| GlobSet::empty());

        Ok(Self {
            exclude_patterns,
            exclude_caches,
            exclude_kernfs,
        })
    }

    pub fn is_kernfs_path(&self, path: &Path) -> bool {
        if !self.exclude_kernfs {
            return false;
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(path_str) = path.to_str() {
                if let Ok(c_path) = std::ffi::CString::new(path_str) {
                    let mut buf = std::mem::MaybeUninit::<libc::statfs>::uninit();
                    // SAFETY: c_path is a valid null-terminated C string, and buf points to valid uninitialized memory for libc::statfs.
                    let res = unsafe { libc::statfs(c_path.as_ptr(), buf.as_mut_ptr()) };
                    if res == 0 {
                        // SAFETY: libc::statfs returned 0 indicating buf was successfully initialized.
                        let buf = unsafe { buf.assume_init() };
                        let f_type = buf.f_type as u64;
                        const PROC_SUPER_MAGIC: u64 = 0x9fa0;
                        const SYSFS_MAGIC: u64 = 0x62656572;
                        const DEVPTS_SUPER_MAGIC: u64 = 0x1cd1;
                        const CGROUP_SUPER_MAGIC: u64 = 0x27e0eb;
                        const CGROUP2_SUPER_MAGIC: u64 = 0x63677270;
                        const SECURITYFS_MAGIC: u64 = 0x73636673;
                        const DEBUGFS_MAGIC: u64 = 0x64626720;
                        const TRACEFS_MAGIC: u64 = 0x74726163;
                        const BPF_FS_MAGIC: u64 = 0xcafe4a11;
                        const RAMFS_MAGIC: u64 = 0x858458f6;

                        if matches!(
                            f_type,
                            PROC_SUPER_MAGIC
                                | SYSFS_MAGIC
                                | DEVPTS_SUPER_MAGIC
                                | CGROUP_SUPER_MAGIC
                                | CGROUP2_SUPER_MAGIC
                                | SECURITYFS_MAGIC
                                | DEBUGFS_MAGIC
                                | TRACEFS_MAGIC
                                | BPF_FS_MAGIC
                                | RAMFS_MAGIC
                        ) {
                            return true;
                        }
                    }
                }
            }
        }

        if let Some(path_str) = path.to_str() {
            let kernfs_prefixes = &[
                "/proc",
                "/sys",
                "/sys/fs",
                "/dev/pts",
                "/sys/kernel/debug",
                "/sys/fs/cgroup",
                "/sys/fs/bpf",
            ];
            for prefix in kernfs_prefixes {
                if path_str == *prefix
                    || path_str
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('/'))
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_glob_match(&self, path: &Path) -> bool {
        if self.exclude_patterns.is_empty() {
            return false;
        }
        if let Some(file_name) = path.file_name() {
            if self.exclude_patterns.is_match(file_name) {
                return true;
            }
        }
        self.exclude_patterns.is_match(path)
    }

    pub fn should_exclude_path(&self, path: &Path) -> bool {
        self.is_glob_match(path) || self.is_kernfs_path(path)
    }

    pub fn verify_cachedir_tag(&self, tag_file_path: &Path) -> bool {
        if !self.exclude_caches {
            return false;
        }

        // Check if tag file signature is correct: Signature: 8a477f597d28d172789f06886806bc55
        if let Ok(mut file) = File::open(tag_file_path) {
            let mut buf = [0u8; 43];
            if file.read_exact(&mut buf).is_ok() {
                if let Ok(contents) = std::str::from_utf8(&buf) {
                    return contents.starts_with("Signature: 8a477f597d28d172789f06886806bc55");
                }
            }
        }

        false
    }

    pub fn has_cachedir_tag(&self, dir_path: &Path) -> bool {
        if !self.exclude_caches {
            return false;
        }

        let tag_file_path = dir_path.join("CACHEDIR.TAG");
        if !tag_file_path.exists() {
            return false;
        }

        self.verify_cachedir_tag(&tag_file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_glob_exclusions() -> Result<()> {
        let patterns = vec!["*.tmp".to_string(), "node_modules".to_string()];
        let filter = Filter::new(&patterns, None, false, false)?;

        assert!(filter.should_exclude_path(Path::new("test.tmp")));
        assert!(filter.should_exclude_path(Path::new("node_modules")));
        assert!(!filter.should_exclude_path(Path::new("main.rs")));
        Ok(())
    }

    #[test]
    fn test_kernfs_exclusions() -> Result<()> {
        let filter = Filter::new(&[], None, false, true)?;

        assert!(filter.should_exclude_path(Path::new("/proc/cpuinfo")));
        assert!(filter.should_exclude_path(Path::new("/sys/class")));
        assert!(!filter.should_exclude_path(Path::new("/home/user/doc")));
        Ok(())
    }

    #[test]
    fn test_verify_cachedir_tag() -> Result<()> {
        let filter = Filter::new(&[], None, true, false)?;

        let temp_dir = std::env::temp_dir();
        let valid_path = temp_dir.join("test_valid_cachedir.tag");
        {
            let mut file = File::create(&valid_path)?;
            write!(
                file,
                "Signature: 8a477f597d28d172789f06886806bc55\nHeader info"
            )?;
        }
        assert!(filter.verify_cachedir_tag(&valid_path));
        let _ = std::fs::remove_file(&valid_path);

        let invalid_path = temp_dir.join("test_invalid_cachedir.tag");
        {
            let mut file = File::create(&invalid_path)?;
            write!(file, "Invalid signature header")?;
        }
        assert!(!filter.verify_cachedir_tag(&invalid_path));
        let _ = std::fs::remove_file(&invalid_path);

        Ok(())
    }
}
