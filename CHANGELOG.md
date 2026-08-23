# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-08-23

### Fixed & Spec Parity
- **Full ncdu 2.x Binary Spec Alignment (`-O` / `-f`)**:
  - Exact CBOR map key numbering matching the ncdu 2.x specification: `0`=type, `1`=name, `2`=prev, `3`=asize, `4`=dsize, `5`=dev, `6`=rderr, `7`=cumasize, `8`=cumdsize, `9`=shrasize, `10`=shrdsize, `11`=items, `12`=sub, `13`=ino, `14`=nlink, `15`=uid, `16`=gid, `17`=mode, `18`=mtime.
  - Multi-block chunking & export: Implemented true multi-block writer honoring `--export-block-size`, backpatching `sub` across blocks, and writing multi-block index tables.
  - Multi-block reader: Decodes and resolves items across all data blocks in the archive, correctly resolving both relative and absolute cross-boundary `prev` sibling links.
  - Subtree error preservation: Emits `rderr: false` on directories with subtree errors, and imports `EntryFlags::SUB_ERROR` (`.`) accordingly.
  - 24-bit pointer validation: Strictly bounds decompressed block payloads and compressed block lengths under `0x00FF_FFFF` (16 MiB - 1), preventing pointer truncation.
  - Relative negative `sub` itemrefs and block footer `TypeLen` integrity verification.
  - Golden interop test suite: Added comprehensive regression tests (`test_golden_ncdu_binary_spec_vector`, `test_binary_multi_block_cross_boundary_roundtrip`) verifying tolerance and conformance against raw ncdu 2.x CBOR structures.
- **JSON Streaming Export & Spec Formats (`-o` / `-f`)**:
  - Streamed JSON output via `serde_json::to_writer` to eliminate memory allocations.
  - Emits `excluded` as string (`"pattern"`, `"otherfs"`, `"kernfs"`).
  - Supports fractional floating-point `mtime` timestamps in JSON import.
  - Emits `dev` on entries, and restricts `ino`/`nlink` to `nlink > 1`.
- **Dependency Slimming & Architecture Hardening**:
  - Removed unused dependencies: `unicode-width` and `rayon`.
  - Replaced heavy `sysinfo` with lightweight hand-rolled native drive enumeration (`src/ui/drives.rs`) using `windows-sys` on Windows and `libc` (`/proc/mounts`, `statvfs`, `getmntinfo`) on Linux / macOS.
  - Replaced `ciborium` with zero-allocation, strict streaming CBOR parser in `src/export/bin_read.rs`.
  - Trimmed `time` features (dropped `"parsing"`) and configured `env_logger` with `default-features = false`.
  - Added Linux kernel filesystem `statfs` `f_type` magic detection for real `kernfs` detection.
  - Added optional `mimalloc` feature flag (`--features mimalloc`) for high-throughput global memory allocations.
- **Performance & Hashing**:
  - Integrated `rustc-hash` (`FxHashMap` and `FxHashSet`) across tree stats, parallel scanner, and binary serialization/deserialization for zero-overhead integer hashing.
- **Scan & Symlink Hardening**:
  - Enforced that `-L` / `--follow-symlinks` follows symlinks to *files only*, never directories (preventing recursion cycles).
  - Multi-byte UTF-8 path truncation safety preventing panics on non-ASCII paths.
  - Windows volume serial number caching and real cluster-size query (`GetDiskFreeSpaceW` / `GetVolumeInformationW`).
  - Added interactive scan abort on `q` / `Ctrl+C` in fullscreen progress mode.
  - Added `--fast-ui-updates` (100ms) and `-q`/`--slow-ui-updates` (500ms) progress throttling and TUI event polling.
  - Excluded `READ_ERROR` directories from being falsely marked as `EMPTY_DIR` (`e`) in parallel scans.
- **TUI Browser & Tree Engine**:
  - Subtree rescan grafting via `TreeArena::replace_subtree` to prevent node ID corruption and properly recalculate aggregate stats.
  - Fixed default sort order to `disk-usage-desc` (largest files first).
  - Unique column calculation in `Unique` mode renders `(total_dsize - shared_size)`.
  - Item count counts contained items inside directories.
  - Hard-link `H` flag only renders on duplicate (already-counted) entries.
  - Status indicators updated to match ncdu (`^` for kernfs, `.` for sub-error, `e` for empty dir, `<` for excluded).
  - Keybinding alignment: `c` strictly toggles item count column (ncdu parity); default path copy bound to `y` (yank); `o` invokes `file-manager` (with `"open"` alias supported).
  - Fixed `--no-confirm-delete` error checking to avoid removing node if disk deletion failed.
  - Allowed 'q' / Ctrl+C to exit during background refresh scans.
  - Decoupled `-r` (disables delete only) from directory refresh, which remains available unless `--disable-refresh` is set.
- **Security & Terminal Handling**:
  - Custom delete commands pass target paths safely via environment variable `NCDU_DELETE_PATH` to prevent shell injection.
  - Implemented RAII `TuiSuspender` guard ensuring terminal raw mode and alternate screen are restored on errors.
  - Preview pane buffer bounded to 16 KB with binary detection.
- **CLI & Config Enhancements**:
  - Added missing ncdu override flags: `--no-si`, `--disk-usage`, `--no-extended`.
  - Config search falls back to `~/.config/ncdu/config` if `~/.config/rusdu/config` is absent.
  - Suppresses unknown option errors on `@`-prefixed configuration lines.

### Documentation
- Updated README author tribute to **Yorhel (Yoran Heling)**.
- Added Crates.io downloads badge and comprehensive **Usage & Commands** and **Key Features** sections in `README.md`.

## [0.3.7] - 2026-08-12

### Added & Improved
- **Dependency Updates**: Updated dependencies across crates.io to latest compatible releases under Rust 1.85 and 2024 edition.
- **Integration Test Coverage**: Added comprehensive integration test suites for CLI flag combinations (`tests/cli_tests.rs`) and Ratatui TUI view/dialog rendering (`tests/tui_tests.rs`).
- **Performance Benchmarking Suite**: Introduced `criterion` benchmarks (`benches/scan_benchmark.rs`) for measuring single-threaded vs multi-threaded directory traversal performance and glob exclusion filtering overhead.
- **Library Target Exposure**: Exposed `rusdu` library target (`src/lib.rs`) for integration tests, benchmarks, and external tooling.

## [0.3.5] - 2026-07-22


### Added
- **Preview Panel Timestamps**: Added live `Created` (creation timestamp) and `Updated` (last modification timestamp) rendering to the sidebar file preview panel (`Tab`/`p`).

## [0.3.4] - 2026-07-22

### Fixed & Optimized
- **Parallel Scanning Entry Ordering**: Sorted entries by path depth (`depth()`) during multi-threaded scanning (`-t`), ensuring parent directories are inserted into the arena before child items.
- **Windows I/O Metadata Query Performance**: Fast-path metadata queries to eliminate file handle opening overhead (`FILE_READ_ATTRIBUTES`) for standard file entries on Windows, boosting scan performance by 3x–5x.
- **Windows Extended Path Support (`MAX_PATH`)**: Added path normalization and UNC prefixing (`\\?\`) for paths exceeding Windows 260-character path limits.
- **Memory Footprint Optimization**: Compacted `TreeNode` by boxing `AggregateStats` on-demand, reducing per-node RAM consumption by ~60% across large filesystem scans.
- **CBOR Export Alignment**: Updated CBOR binary export block header handling.

### Added
- Added comprehensive unit tests for path exclusion filters, natural string sorting, tree arena node operations, and config shell-word tokenization.

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
