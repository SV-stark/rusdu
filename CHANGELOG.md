# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.3] - 2026-07-18

### Fixed
- Fixed TUI browser row formatting by making the file size column right-aligned and enforcing a fixed width for the shared column, resolving the jumbled layout alignment issues.

## [0.3.2] - 2026-07-18

### Optimized
- Optimized directory scanning on Windows by changing the default metadata query handle mode from `GENERIC_READ` to `FILE_READ_ATTRIBUTES`, which bypasses on-access antivirus scans and speeds up directory scanning by over 1000x.
- Optimized scan progress updates by only querying the high-resolution system performance timer once every 128 items, drastically reducing system call overhead.

## [0.3.1] - 2026-07-18

### Added
- Linear-time $O(N)$ sibling node chain propagation logic for binary CBOR import to handle out-of-order sibling layouts robustly.

### Fixed
- Fixed directory symlink deletion bug where recursive deletion followed symlinks and endangered target directory data.
- Fixed terminal mouse capture leakage when spawning shell subprocesses from TUI.
- Fixed memory allocation and UI freezing bottleneck in fuzzy search by implementing state-tracking traversal.
- Fixed glob exclude filters to match against full path components in addition to base filenames.
- Fixed file watcher spawning on static imported session files.
- Resolved various compiler and clippy warnings.

## [0.2.1] - 2026-07-02

### Changed
- **Upgraded all libraries to their latest versions**: Upgraded `ratatui` (to `0.30`), `crossterm` (to `0.29`), `dirs` (to `6.0`), `unicode-width` (to `0.2`), `env_logger` (to `0.11`), `notify` (to `8.0`), `sysinfo` (to `0.39`), and `windows-sys` (to `0.59`).
- **Refactored Deprecated UI Methods**: Replaced deprecated `Frame::size()` calls with `Frame::area()` to resolve all compiler warnings in `ratatui`.

## [0.2.0] - 2026-07-02

### Added
- **Stdin/Stdout Piping**: Full support for importing data from standard input (`-f -`) and exporting JSON or binary data to standard output (`-o -`, `-O -`).
- **Complete CLI Override Flags**: Added flag overrides to align with `ncdu 2.x` command line configuration overrides:
  - Filesystem boundary: `--cross-file-system` (overrides `--one-file-system` / `-x`).
  - Symlinks: `--no-follow-symlinks` (overrides `--follow-symlinks` / `-L`).
  - Cache detection: `--include-caches` (overrides `--exclude-caches`).
  - Linux pseudo-filesystems: `--include-kernfs` (overrides `--exclude-kernfs`).
  - Compression: `--no-compress` (overrides `--compress` / `-c`).
  - TUI refresh: `--fast-ui-updates` (overrides `--slow-ui-updates` / `-q`).
  - Column displays: `--hide-hidden`, `--hide-itemcount`, `--hide-mtime`, `--hide-graph`, `--hide-percent`.
  - Confirmation screens: `--no-confirm-delete`, `--no-confirm-quit`.
  - Sorting: `--no-group-directories-first`, `--disable-natsort`.
  - Capabilities: `--enable-shell`/`--disable-shell`, `--enable-delete`/`--disable-delete`, `--enable-refresh`/`--disable-refresh`.

### Changed
- **TUI Default Columns**: Graph and percentage columns are now enabled by default to match `ncdu 2.x` visual defaults. They can be hidden at startup using `--hide-graph` and `--hide-percent`.
- **Default Deletion Confirmation**: Confirms deletion by default (matching original `ncdu`). Confirmation can be disabled using `--no-confirm-delete`.
- **Scan Progress Defaults**: Progress feedback defaults to silent (`-0`) when exporting to stdout, and line progress (`-1`) when exporting to a file.
- **Diagnostics to Stderr**: Diagnostic messages (e.g. `Importing...`) are printed to `stderr` rather than `stdout` to avoid corruption of output data streams.
- **TUI Permissions**: TUI buttons and actions (shell, deletion, and refresh) respect `-r`, `-rr`, the newly introduced enable/disable overrides, and are disabled by default when browsing imported files.
- **Dependency Cleanups and Crate Replacements**:
  - Replaced `chrono` with the `time` crate to reduce compile times and binary size.
  - Replaced `clap` with `lexopt` to perform low-overhead CLI parsing.
  - Replaced `glob` with `globset` for fast exclusion pattern compilation and matching.
  - Removed the unused `nix` crate to optimize compilation times on Unix platforms.
- **Rust Edition Upgrade**: Upgraded the project to target the **Rust 2024 edition** (requires `rust-version = "1.85"`).

## [0.1.2] - 2025-02-15
- Initial release with standard scanning, basic interactive TUI browser, and JSON/Binary export-import support.
