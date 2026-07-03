use lexopt::{Arg, ValueExt};
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("CLI argument parsing error: {0}")]
    Lexopt(#[from] lexopt::Error),

    #[error("Failed to parse integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Unexpected argument: {0:?}")]
    UnexpectedArgument(std::ffi::OsString),

    #[error("Unknown option: {0:?}")]
    UnknownOption(String),
}

#[derive(Debug, Clone)]
pub struct Args {
    pub path: Option<PathBuf>,
    pub import_file: Option<PathBuf>,
    pub export_json: Option<PathBuf>,
    pub export_bin: Option<PathBuf>,
    pub extended: bool,
    pub ignore_config: bool,
    pub one_file_system: bool,
    pub cross_file_system: bool,
    pub exclude: Vec<String>,
    pub exclude_from: Option<PathBuf>,
    pub exclude_caches: bool,
    pub include_caches: bool,
    pub exclude_kernfs: bool,
    pub include_kernfs: bool,
    pub follow_symlinks: bool,
    pub no_follow_symlinks: bool,
    pub threads: Option<usize>,
    pub compress: bool,
    pub no_compress: bool,
    pub compress_level: Option<i32>,
    pub export_block_size: Option<usize>,
    pub silent: bool,
    pub line_progress: bool,
    pub fullscreen_progress: bool,
    pub slow_updates: bool,
    pub fast_ui_updates: bool,
    pub read_only: u8,
    pub enable_shell: bool,
    pub disable_shell: bool,
    pub enable_delete: bool,
    pub disable_delete: bool,
    pub enable_refresh: bool,
    pub disable_refresh: bool,
    pub si: bool,
    pub apparent_size: bool,
    pub show_hidden: bool,
    pub hide_hidden: bool,
    pub show_itemcount: bool,
    pub hide_itemcount: bool,
    pub show_mtime: bool,
    pub hide_mtime: bool,
    pub show_graph: bool,
    pub hide_graph: bool,
    pub show_percent: bool,
    pub hide_percent: bool,
    pub graph_style: String,
    pub shared_column: String,
    pub sort: String,
    pub enable_natsort: bool,
    pub disable_natsort: bool,
    pub group_directories_first: bool,
    pub no_group_directories_first: bool,
    pub confirm_quit: bool,
    pub no_confirm_quit: bool,
    pub confirm_delete: bool,
    pub no_confirm_delete: bool,
    pub delete_command: Option<String>,
    pub color: String,
    pub icons: bool,
    pub log_file: Option<PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            path: None,
            import_file: None,
            export_json: None,
            export_bin: None,
            extended: false,
            ignore_config: false,
            one_file_system: false,
            cross_file_system: false,
            exclude: Vec::new(),
            exclude_from: None,
            exclude_caches: false,
            include_caches: false,
            exclude_kernfs: false,
            include_kernfs: false,
            follow_symlinks: false,
            no_follow_symlinks: false,
            threads: None,
            compress: false,
            no_compress: false,
            compress_level: None,
            export_block_size: None,
            silent: false,
            line_progress: false,
            fullscreen_progress: false,
            slow_updates: false,
            fast_ui_updates: false,
            read_only: 0,
            enable_shell: false,
            disable_shell: false,
            enable_delete: false,
            disable_delete: false,
            enable_refresh: false,
            disable_refresh: false,
            si: false,
            apparent_size: false,
            show_hidden: false,
            hide_hidden: false,
            show_itemcount: false,
            hide_itemcount: false,
            show_mtime: false,
            hide_mtime: false,
            show_graph: false,
            hide_graph: false,
            show_percent: false,
            hide_percent: false,
            graph_style: "hash".to_string(),
            shared_column: "shared".to_string(),
            sort: "disk-usage".to_string(),
            enable_natsort: false,
            disable_natsort: false,
            group_directories_first: false,
            no_group_directories_first: false,
            confirm_quit: false,
            no_confirm_quit: false,
            confirm_delete: false,
            no_confirm_delete: false,
            delete_command: None,
            color: "off".to_string(),
            icons: false,
            log_file: None,
        }
    }
}

impl Args {
    pub fn parse() -> Result<Self, CliError> {
        let args = std::env::args_os();
        Self::try_parse_from(args)
    }

    pub fn try_parse_from<I, S>(iter: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>,
    {
        let mut parser = lexopt::Parser::from_iter(iter);
        let mut args = Args::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Arg::Short('h') | Arg::Long("help") => {
                    print_help();
                    std::process::exit(0);
                }
                Arg::Short('v') | Arg::Short('V') | Arg::Long("version") => {
                    println!("rusdu {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                Arg::Short('f') | Arg::Long("import") => {
                    args.import_file = Some(parser.value()?.into());
                }
                Arg::Short('o') | Arg::Long("export-json") => {
                    args.export_json = Some(parser.value()?.into());
                }
                Arg::Short('O') | Arg::Long("export-bin") => {
                    args.export_bin = Some(parser.value()?.into());
                }
                Arg::Short('e') | Arg::Long("extended") => {
                    args.extended = true;
                }
                Arg::Long("ignore-config") => {
                    args.ignore_config = true;
                }
                Arg::Short('x') | Arg::Long("one-file-system") => {
                    args.one_file_system = true;
                    args.cross_file_system = false;
                }
                Arg::Long("cross-file-system") => {
                    args.cross_file_system = true;
                    args.one_file_system = false;
                }
                Arg::Long("exclude") => {
                    args.exclude.push(parser.value()?.string()?);
                }
                Arg::Short('X') | Arg::Long("exclude-from") => {
                    args.exclude_from = Some(parser.value()?.into());
                }
                Arg::Long("exclude-caches") => {
                    args.exclude_caches = true;
                    args.include_caches = false;
                }
                Arg::Long("include-caches") => {
                    args.include_caches = true;
                    args.exclude_caches = false;
                }
                Arg::Long("exclude-kernfs") => {
                    args.exclude_kernfs = true;
                    args.include_kernfs = false;
                }
                Arg::Long("include-kernfs") => {
                    args.include_kernfs = true;
                    args.exclude_kernfs = false;
                }
                Arg::Short('L') | Arg::Long("follow-symlinks") => {
                    args.follow_symlinks = true;
                    args.no_follow_symlinks = false;
                }
                Arg::Long("no-follow-symlinks") => {
                    args.no_follow_symlinks = true;
                    args.follow_symlinks = false;
                }
                Arg::Short('t') | Arg::Long("threads") => {
                    let val: usize = parser.value()?.parse()?;
                    args.threads = Some(val);
                }
                Arg::Short('c') | Arg::Long("compress") => {
                    args.compress = true;
                    args.no_compress = false;
                }
                Arg::Long("no-compress") => {
                    args.no_compress = true;
                    args.compress = false;
                }
                Arg::Long("compress-level") => {
                    let val: i32 = parser.value()?.parse()?;
                    args.compress_level = Some(val);
                }
                Arg::Long("export-block-size") => {
                    let val: usize = parser.value()?.parse()?;
                    args.export_block_size = Some(val);
                }
                Arg::Short('0') | Arg::Long("silent") => {
                    args.silent = true;
                    args.line_progress = false;
                    args.fullscreen_progress = false;
                }
                Arg::Short('1') | Arg::Long("line-progress") => {
                    args.line_progress = true;
                    args.silent = false;
                    args.fullscreen_progress = false;
                }
                Arg::Short('2') | Arg::Long("fullscreen-progress") => {
                    args.fullscreen_progress = true;
                    args.silent = false;
                    args.line_progress = false;
                }
                Arg::Short('q') | Arg::Long("slow-ui-updates") => {
                    args.slow_updates = true;
                    args.fast_ui_updates = false;
                }
                Arg::Long("fast-ui-updates") => {
                    args.fast_ui_updates = true;
                    args.slow_updates = false;
                }
                Arg::Short('r') => {
                    args.read_only += 1;
                }
                Arg::Long("enable-shell") => {
                    args.enable_shell = true;
                    args.disable_shell = false;
                }
                Arg::Long("disable-shell") => {
                    args.disable_shell = true;
                    args.enable_shell = false;
                }
                Arg::Long("enable-delete") => {
                    args.enable_delete = true;
                    args.disable_delete = false;
                }
                Arg::Long("disable-delete") => {
                    args.disable_delete = true;
                    args.enable_delete = false;
                }
                Arg::Long("enable-refresh") => {
                    args.enable_refresh = true;
                    args.disable_refresh = false;
                }
                Arg::Long("disable-refresh") => {
                    args.disable_refresh = true;
                    args.enable_refresh = false;
                }
                Arg::Long("si") => {
                    args.si = true;
                }
                Arg::Long("apparent-size") => {
                    args.apparent_size = true;
                }
                Arg::Long("show-hidden") => {
                    args.show_hidden = true;
                    args.hide_hidden = false;
                }
                Arg::Long("hide-hidden") => {
                    args.hide_hidden = true;
                    args.show_hidden = false;
                }
                Arg::Long("show-itemcount") => {
                    args.show_itemcount = true;
                    args.hide_itemcount = false;
                }
                Arg::Long("hide-itemcount") => {
                    args.hide_itemcount = true;
                    args.show_itemcount = false;
                }
                Arg::Long("show-mtime") => {
                    args.show_mtime = true;
                    args.hide_mtime = false;
                }
                Arg::Long("hide-mtime") => {
                    args.hide_mtime = true;
                    args.show_mtime = false;
                }
                Arg::Long("show-graph") => {
                    args.show_graph = true;
                    args.hide_graph = false;
                }
                Arg::Long("hide-graph") => {
                    args.hide_graph = true;
                    args.show_graph = false;
                }
                Arg::Long("show-percent") => {
                    args.show_percent = true;
                    args.hide_percent = false;
                }
                Arg::Long("hide-percent") => {
                    args.hide_percent = true;
                    args.show_percent = false;
                }
                Arg::Long("graph-style") => {
                    args.graph_style = parser.value()?.string()?;
                }
                Arg::Long("shared-column") => {
                    args.shared_column = parser.value()?.string()?;
                }
                Arg::Long("sort") => {
                    args.sort = parser.value()?.string()?;
                }
                Arg::Long("enable-natsort") => {
                    args.enable_natsort = true;
                    args.disable_natsort = false;
                }
                Arg::Long("disable-natsort") => {
                    args.disable_natsort = true;
                    args.enable_natsort = false;
                }
                Arg::Long("group-directories-first") => {
                    args.group_directories_first = true;
                    args.no_group_directories_first = false;
                }
                Arg::Long("no-group-directories-first") => {
                    args.no_group_directories_first = true;
                    args.group_directories_first = false;
                }
                Arg::Long("confirm-quit") => {
                    args.confirm_quit = true;
                    args.no_confirm_quit = false;
                }
                Arg::Long("no-confirm-quit") => {
                    args.no_confirm_quit = true;
                    args.confirm_quit = false;
                }
                Arg::Long("confirm-delete") => {
                    args.confirm_delete = true;
                    args.no_confirm_delete = false;
                }
                Arg::Long("no-confirm-delete") => {
                    args.no_confirm_delete = true;
                    args.confirm_delete = false;
                }
                Arg::Long("delete-command") => {
                    args.delete_command = Some(parser.value()?.string()?);
                }
                Arg::Long("color") => {
                    args.color = parser.value()?.string()?;
                }
                Arg::Long("icons") => {
                    args.icons = true;
                }
                Arg::Long("log-file") => {
                    args.log_file = Some(parser.value()?.into());
                }
                Arg::Value(val) => {
                    if args.path.is_none() {
                        args.path = Some(val.into());
                    } else {
                        return Err(CliError::UnexpectedArgument(val.into()));
                    }
                }
                _ => {
                    return Err(CliError::UnknownOption(format!("{:?}", arg)));
                }
            }
        }

        Ok(args)
    }
}

fn print_help() {
    println!(
        "rusdu {} - Rust rewrite of ncdu — NCurses Disk Usage analyzer\n\n\
        Usage: rusdu [path] [options]\n\n\
        Options:\n\
          -h, --help                  Print this help message\n\
          -v, -V, --version           Print version\n\
          -f, --import FILE           Load and browse a previously exported JSON or binary file (use '-' for stdin)\n\
          -o, --export-json FILE      Scan the directory and export results to a JSON file (use '-' for stdout)\n\
          -O, --export-bin FILE       Scan and export results to a binary CBOR file (use '-' for stdout)\n\
          -x, --one-file-system       Stay on the same filesystem partition\n\
              --cross-file-system     Cross filesystem boundaries (default)\n\
          -e, --extended              Enable extended info mode (mtime, uid, gid, mode)\n\
              --ignore-config         Do not attempt to load any configuration files\n\
              --exclude PATTERN       Exclude files that match pattern\n\
          -X, --exclude-from FILE     Exclude files that match any pattern in file\n\
              --exclude-caches        Exclude directories containing CACHEDIR.TAG\n\
              --include-caches        Include directories containing CACHEDIR.TAG (default)\n\
              --exclude-kernfs        Exclude Linux pseudo filesystems (e.g. /proc, /sys)\n\
              --include-kernfs        Include Linux pseudo filesystems (default)\n\
          -L, --follow-symlinks       Follow symlinks\n\
              --no-follow-symlinks    Do not follow symlinks (default)\n\
          -t, --threads N             Number of threads to use when scanning (default: 1)\n\
          -c, --compress              Enable Zstandard compression when exporting to JSON (-o)\n\
              --no-compress           Disable Zstandard compression (default)\n\
              --compress-level N      Set the Zstandard compression level (1-19, default: 4)\n\
              --export-block-size N   Set block size in KiB for binary export format (-O)\n\
          -0, --silent                Silent mode (no progress feedback)\n\
          -1, --line-progress         Line progress mode\n\
          -2, --fullscreen-progress   Fullscreen progress mode (default)\n\
          -q, --slow-ui-updates       Slow down TUI updates\n\
              --fast-ui-updates       Fast TUI updates (default)\n\
          -r                          Read-only mode (-r: disable delete, -rr: also disable shell)\n\
              --enable-shell          Enable shell spawning\n\
              --disable-shell         Disable shell spawning\n\
              --enable-delete         Enable file deletion\n\
              --disable-delete        Disable file deletion\n\
              --enable-refresh        Enable directory refresh\n\
              --disable-refresh       Disable directory refresh\n\
              --si                    Use SI units (base 10) instead of binary (base 2)\n\
              --apparent-size         Show apparent size instead of disk usage\n\
              --show-hidden           Show hidden and excluded files\n\
              --hide-hidden           Hide hidden and excluded files (default: show)\n\
              --show-itemcount        Show item counts column\n\
              --hide-itemcount        Hide item counts column (default)\n\
              --show-mtime            Show last modification time column (requires -e)\n\
              --hide-mtime            Hide last mtime column (default)\n\
              --show-graph            Show relative size bar column (default)\n\
              --hide-graph            Hide relative size bar column\n\
              --show-percent          Show relative size percent column (default)\n\
              --hide-percent          Hide relative size percent column\n\
              --graph-style STYLE     Graph style: hash (default), half-block, eighth-block\n\
              --shared-column MODE    Shared column mode: off, shared (default), unique\n\
              --sort COLUMN           Default sort: disk-usage, name, apparent-size, itemcount, mtime\n\
              --enable-natsort        Enable natural sort (default)\n\
              --disable-natsort       Disable natural sort\n\
              --group-directories-first Group directories before files\n\
              --no-group-directories-first Do not group directories (default)\n\
              --confirm-quit          Require confirmation before quitting\n\
              --no-confirm-quit       Do not require confirmation before quitting (default)\n\
              --confirm-delete        Require confirmation before deleting (default)\n\
              --no-confirm-delete     Delete without confirmation\n\
              --delete-command CMD    Replace built-in deletion with custom command\n\
              --color SCHEME          Color scheme: off (default), dark, dark-bg\n\
              --icons                 Enable Nerd Font icons in TUI list\n\
              --log-file FILE         Log errors and scanning diagnostics to a file",
        env!("CARGO_PKG_VERSION")
    );
}
