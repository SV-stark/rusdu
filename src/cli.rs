use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "rusdu",
    version,
    about = "Rust rewrite of ncdu — NCurses Disk Usage analyzer"
)]
pub struct Args {
    /// Directory or file to scan
    pub path: Option<PathBuf>,

    /// Load the given file earlier created with -o or -O
    #[arg(short = 'f', long = "import")]
    pub import_file: Option<PathBuf>,

    /// Export directory tree in JSON format to file
    #[arg(short = 'o', long = "export-json")]
    pub export_json: Option<PathBuf>,

    /// Export directory tree in binary format to file
    #[arg(short = 'O', long = "export-bin")]
    pub export_bin: Option<PathBuf>,

    /// Enable extended information mode (read uid, gid, mode, mtime)
    #[arg(short = 'e', long = "extended")]
    pub extended: bool,

    /// Do not attempt to load any configuration files
    #[arg(long = "ignore-config")]
    pub ignore_config: bool,

    /// Do not cross filesystem boundaries
    #[arg(short = 'x', long = "one-file-system")]
    pub one_file_system: bool,

    /// Exclude files that match pattern
    #[arg(long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Exclude files that match any pattern in file
    #[arg(short = 'X', long = "exclude-from", value_name = "FILE")]
    pub exclude_from: Option<PathBuf>,

    /// Exclude directories containing CACHEDIR.TAG
    #[arg(long = "exclude-caches")]
    pub exclude_caches: bool,

    /// Exclude Linux pseudo filesystems (e.g. /proc, /sys)
    #[arg(long = "exclude-kernfs")]
    pub exclude_kernfs: bool,

    /// Follow symlinks and count the size of the file they point to
    #[arg(short = 'L', long = "follow-symlinks")]
    pub follow_symlinks: bool,

    /// Number of threads to use when scanning the filesystem
    #[arg(short = 't', long = "threads")]
    pub threads: Option<usize>,

    /// Enable Zstandard compression when exporting to JSON (-o)
    #[arg(short = 'c', long = "compress")]
    pub compress: bool,

    /// Set the Zstandard compression level when using -O or -c (1-19)
    #[arg(long = "compress-level")]
    pub compress_level: Option<i32>,

    /// Set block size in KiB for binary export format (-O)
    #[arg(long = "export-block-size")]
    pub export_block_size: Option<usize>,

    /// Don't give any feedback while scanning a directory (silent mode)
    #[arg(short = '0', long = "silent")]
    pub silent: bool,

    /// Write progress information to the terminal (line mode)
    #[arg(short = '1', long = "line-progress")]
    pub line_progress: bool,

    /// Show full-screen TUI interface while scanning (default)
    #[arg(short = '2', long = "fullscreen-progress")]
    pub fullscreen_progress: bool,

    /// Slow down TUI updates (update every 2 seconds instead of 10 times a second)
    #[arg(short = 'q', long = "slow-ui-updates")]
    pub slow_updates: bool,

    /// Read-only mode: -r disables deletion, -rr disables deletion and shell spawning
    #[arg(short = 'r', action = clap::ArgAction::Count)]
    pub read_only: u8,

    /// Use base 10 prefixes (SI units) instead of base 2
    #[arg(long = "si")]
    pub si: bool,

    /// Show apparent size instead of disk usage
    #[arg(long = "apparent-size")]
    pub apparent_size: bool,

    /// Show hidden and excluded files
    #[arg(long = "show-hidden")]
    pub show_hidden: bool,

    /// Show item counts column
    #[arg(long = "show-itemcount")]
    pub show_itemcount: bool,

    /// Show last modification time column (requires -e)
    #[arg(long = "show-mtime")]
    pub show_mtime: bool,

    /// Show relative size bar column
    #[arg(long = "show-graph")]
    pub show_graph: bool,

    /// Show relative size percent column
    #[arg(long = "show-percent")]
    pub show_percent: bool,

    /// Graph style: hash, half-block, eighth-block
    #[arg(long = "graph-style", default_value = "hash")]
    pub graph_style: String,

    /// Shared column mode: off, shared, unique
    #[arg(long = "shared-column", default_value = "shared")]
    pub shared_column: String,

    /// Change the default column to sort on: disk-usage, name, apparent-size, itemcount, mtime
    #[arg(long = "sort", default_value = "disk-usage")]
    pub sort: String,

    /// Enable natural sort when sorting by file name
    #[arg(long = "enable-natsort")]
    pub enable_natsort: bool,

    /// Sort directories before files
    #[arg(long = "group-directories-first")]
    pub group_directories_first: bool,

    /// Require confirmation before quitting
    #[arg(long = "confirm-quit")]
    pub confirm_quit: bool,

    /// Require confirmation before deleting a file/directory
    #[arg(long = "confirm-delete")]
    pub confirm_delete: bool,

    /// Replace built-in deletion with custom shell command
    #[arg(long = "delete-command", value_name = "CMD")]
    pub delete_command: Option<String>,

    /// Set color scheme: off, dark, dark-bg
    #[arg(long = "color", default_value = "off")]
    pub color: String,

    /// Enable Nerd Font icons in TUI list
    #[arg(long = "icons")]
    pub icons: bool,

    /// Log errors and scanning diagnostics to a file
    #[arg(long = "log-file", value_name = "FILE")]
    pub log_file: Option<PathBuf>,
}
