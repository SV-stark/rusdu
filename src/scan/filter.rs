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
            if file_path.exists() {
                let file = File::open(file_path)?;
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
        }

        let exclude_patterns = builder.build().unwrap_or_else(|_| GlobSet::empty());

        Ok(Self {
            exclude_patterns,
            exclude_caches,
            exclude_kernfs,
        })
    }

    pub fn should_exclude_path(&self, path: &Path) -> bool {
        if self.exclude_patterns.is_empty() && !self.exclude_kernfs {
            return false;
        }

        // 1. Check glob patterns on the filename or relative path
        if !self.exclude_patterns.is_empty() {
            if let Some(file_name) = path.file_name() {
                if self.exclude_patterns.is_match(file_name) {
                    return true;
                }
            }
            if self.exclude_patterns.is_match(path) {
                return true;
            }
        }

        // 2. Check kernfs paths if on Linux and exclude_kernfs is enabled
        if self.exclude_kernfs {
            if let Some(path_str) = path.to_str() {
                // Known pseudo-filesystem prefixes
                let kernfs_prefixes = &["/proc/", "/sys/", "/dev/", "/run/", "/sys/fs/"];
                for prefix in kernfs_prefixes {
                    if path_str.starts_with(prefix) {
                        return true;
                    }
                }
            }
        }

        false
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
