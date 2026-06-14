use anyhow::Result;
use glob::Pattern;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct Filter {
    exclude_patterns: Vec<Pattern>,
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
        let mut exclude_patterns = Vec::new();

        // Compile CLI patterns
        for pat_str in exclude_strs {
            if let Ok(pat) = Pattern::new(pat_str) {
                exclude_patterns.push(pat);
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
                        if let Ok(pat) = Pattern::new(trimmed) {
                            exclude_patterns.push(pat);
                        }
                    }
                }
            }
        }

        Ok(Self {
            exclude_patterns,
            exclude_caches,
            exclude_kernfs,
        })
    }

    pub fn should_exclude_path(&self, path: &Path) -> bool {
        // 1. Check glob patterns on the filename or relative path
        if let Some(file_name) = path.file_name() {
            let file_name_str = file_name.to_string_lossy();
            for pattern in &self.exclude_patterns {
                if pattern.matches(&file_name_str) {
                    return true;
                }
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

    pub fn has_cachedir_tag(&self, dir_path: &Path) -> bool {
        if !self.exclude_caches {
            return false;
        }

        let tag_file_path = dir_path.join("CACHEDIR.TAG");
        if !tag_file_path.exists() {
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
}
