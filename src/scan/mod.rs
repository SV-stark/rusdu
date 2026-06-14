pub mod filter;
pub mod platform;
pub mod walker;
pub mod parallel;

use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::tree::TreeArena;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub one_file_system: bool,
    pub exclude_patterns: Vec<String>,
    pub exclude_from: Option<PathBuf>,
    pub exclude_caches: bool,
    pub exclude_kernfs: bool,
    pub follow_symlinks: bool,
    pub threads: usize,
    pub extended: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    Silent,
    Line,
    Fullscreen,
}

pub fn scan_directory(
    path: &Path,
    opts: ScanOptions,
    progress_mode: ProgressMode,
) -> Result<TreeArena> {
    // If threads > 1, run parallel scan, otherwise run single-threaded walker
    if opts.threads > 1 {
        parallel::scan_parallel(path, opts, progress_mode)
    } else {
        walker::scan_single_threaded(path, opts, progress_mode)
    }
}
