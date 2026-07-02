# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
