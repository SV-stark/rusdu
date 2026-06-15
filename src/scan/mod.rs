pub mod filter;
pub mod parallel;
pub mod platform;
pub mod walker;

use crate::tree::TreeArena;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub one_file_system: bool,
    pub exclude_patterns: Vec<String>,
    pub exclude_from: Option<std::path::PathBuf>,
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
) -> anyhow::Result<TreeArena> {
    if opts.threads > 1 {
        parallel::scan_parallel(path, opts, progress_mode)
    } else {
        walker::scan_single_threaded(path, opts, progress_mode)
    }
}

#[derive(Default)]
pub struct ScanStats {
    pub items_scanned: u64,
    pub size_scanned: i64,
    pub last_update: Option<std::time::Instant>,
}

pub fn update_progress(current_path: &Path, stats: &mut ScanStats, mode: ProgressMode) {
    if mode == ProgressMode::Silent {
        return;
    }

    let now = std::time::Instant::now();
    if let Some(last) = stats.last_update {
        if now.duration_since(last).as_millis() < 100 {
            return;
        }
    }
    stats.last_update = Some(now);

    let path_str = current_path.to_string_lossy();
    let truncated_path = if path_str.len() > 50 {
        format!("...{}", &path_str[path_str.len() - 47..])
    } else {
        path_str.into_owned()
    };

    if mode == ProgressMode::Line {
        let size_str = crate::format::format_size(stats.size_scanned, false);
        eprint!(
            "\rScanning: {} items [size: {}] | Current: {}",
            stats.items_scanned, size_str, truncated_path
        );
        use std::io::Write;
        let _ = std::io::stderr().flush();
    } else if mode == ProgressMode::Fullscreen {
        use crossterm::{cursor, terminal, QueueableCommand};
        use std::io::Write;
        let mut stderr = std::io::stderr();
        let _ = stderr.queue(cursor::Hide);
        let _ = stderr.queue(terminal::Clear(terminal::ClearType::All));
        let _ = stderr.queue(cursor::MoveTo(0, 0));
        let size_str = crate::format::format_size(stats.size_scanned, false);
        let _ = writeln!(
            stderr,
            "rusdu {} ~ Scanning files...\n\n   Items scanned: {}\n   Total size   : {}\n   Scanning     : {}\n\nPress 'q' to abort.",
            env!("CARGO_PKG_VERSION"),
            stats.items_scanned,
            size_str,
            truncated_path
        );
        let _ = stderr.flush();
    }
}
