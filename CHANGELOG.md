# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.3] - 2026-10-04

### Fixed — Scan Correctness
- **Hidden files silently dropped in parallel mode (`-t N`)**: `jwalk`'s `WalkDirGeneric` defaults to `skip_hidden: true`, so every dotfile and dot-directory was omitted while the single-threaded walker included them. The two backends reported wildly different totals for the same directory. Verified before the fix: a scan with `-t 4` lost `.dotfile` and `.hidden_dir/secret.bin`; after the fix both backends agree. Added `test_scan_hidden_files_present_with_multiple_threads`.
- **Symlinked directories duplicated the whole subtree under `-L`**: `entry.file_type` is the type of the *target* once `follow_links` is set, so `is_symlink()` was always `false`, a symlinked directory was registered as a real directory and recursed into. Verified before the fix: a junction was expanded 63 levels deep; the single-threaded walker never recurses into a followed symlink. Now uses `path_is_symlink()`, matching ncdu's "follow links to files only, never directories" behaviour.
- **Unreadable directories reported as empty**: `parallel.rs` discarded every walk error with `filter_map(..ok())`, so a directory the process cannot read was created as a normal directory and then flagged `EMPTY_DIR` — silently under-reporting totals with no error indicator. Walk errors are now collected, the parent is marked `READ_ERROR`, and a count is reported to the user.
- **Root-relative exclude patterns never matched**: globset anchors patterns, so only base-name patterns worked and a multi-component pattern like `src/main.rs` matched nothing — meaning `-X` pattern files (which conventionally hold root-relative paths) largely did nothing. Patterns are now also matched against the path relative to the scan root, with forward-slash normalisation so one pattern file works on Windows. Added `test_exclude_pattern_matches_path_relative_to_root`.
- **Invalid globs silently discarded**: an unparsable `--exclude` pattern was dropped with `if let Ok(..)`, so a typo excluded nothing; a failed `GlobSetBuilder::build()` fell back to an *empty* set, discarding every exclusion for the whole run. Both now report an error. Bad lines in an `-X` file are reported and skipped instead of aborting the entire scan on a single non-UTF-8 byte. Added `test_invalid_exclude_pattern_is_reported`.
- **Scanning a plain file exited 0 with a bogus tree**: both backends accepted a regular file as a scan root and produced a one-node tree flagged as a read error. Now fails with `is not a directory`. Added `test_scan_rejects_non_directory_root`.
- **Scan abort was non-functional**: `ScanStats::aborted` was set but never read, so an interrupted scan returned a truncated tree that looked complete — and the interrupted directory was flagged `EMPTY_DIR`. The parallel backend also never checked the flag at all. Both backends now check the abort flag, propagate it as an error, and no longer mark partially-scanned directories as empty.
- **Fullscreen `q` abort could never work on Unix**: raw mode was only enabled *after* the scan, so the line discipline buffered `q` until a newline and `Ctrl+C` raised `SIGINT`. A `ScanRawMode` guard now puts the terminal in raw mode for the duration of a fullscreen scan and restores it afterwards (only when stdin/stderr are real terminals, so piped input is unaffected). Also stopped hiding the cursor during progress without ever showing it again.
- **`-t` was unvalidated**: `-t 0` silently fell back to the single-threaded walker and an arbitrarily large value was passed straight to rayon. Thread count is now clamped to `1..=available_parallelism()`. Added `test_scan_thread_count_is_clamped`.

### Fixed — TUI
- **`--confirm-quit` made the application unkillable**: the quit dialog returned "exit" through a boolean whose only `false` value the caller read as "key not handled", so confirming with `y`/Enter left the dialog open forever and `q` was a dead key for the rest of the session. Quit is now signalled through explicit state.
- **A dead refresh worker bricked the TUI**: `try_recv()` only handled `Ok(_)`, never `Disconnected`. If the refresh thread panicked or returned early, the "Refreshing..." modal was drawn every frame and every key was swallowed with no way out. The terminal condition is now handled and the channel cleared.
- **Terminal left in raw mode on error**: terminal setup had no RAII guard, so any `?` early return from `draw`/`poll`/`read` or any panic in a handler left the user's shell in raw mode on the alternate screen with a hidden cursor. Added a `TerminalRestore` guard.
- **Mouse navigation broke the filesystem watcher**: navigating into a directory by double-click never re-armed the watcher, unlike the keyboard path, leaving it pointed at the parent so change detection and refresh targeted the wrong path.
- **Clicks outside the file list moved the cursor**: hit-testing used the raw screen row with no upper bound and no column check, so clicking the header/footer or anywhere in the preview pane jumped the selection to an index that was never displayed. Now bounded to the list viewport.
- **`--show-*` flags were parsed and then ignored**: `show_hidden`, `hide_itemcount`, `hide_mtime`, `show_graph`, `show_percent` and `no_confirm_quit` had no read site, so e.g. `rusdu --no-confirm-quit` still prompted and `--show-itemcount` did nothing. Both halves of each flag pair are now honoured.

### Fixed — Display & Aggregation
- **Extension analytics double-counted**: excluded entries and hard-link duplicates were summed into per-extension totals, so the percentages disagreed with the footer total. A negative `dsize` was widened to `~1.8e19`, collapsing every other percentage to `0.0%`; the cast is now clamped.
- **Directory mtime column showed the wrong timestamp**: it used the directory's own mtime rather than the newest mtime in the subtree (which is what the `mtime` sort key uses), so the two disagreed.
- **Percentages could exceed 100%**: the denominator skips excluded children while excluded entries are listed when hidden files are shown, and the value was not clamped, which also widened the column and shifted every row after it.
- **`u32` counter overflow in stats**: item/directory/file counts used `+=` while the size totals used saturating arithmetic, overflowing on trees with more than `u32::MAX` items.

### Testing
- Six new regression tests covering the hidden-file and thread-count bugs, non-directory roots, root-relative exclude patterns, invalid glob reporting, and scan cancellation.
- Full suite green (39 tests); `cargo fmt --check` and `cargo clippy --all-targets -D warnings` clean.

## [0.4.2] - 2026-09-12

### Fixed & Hardened
- **Unsafe Code Soundness & FFI Safety**:
  - Replaced `std::mem::zeroed` with `std::mem::MaybeUninit` across Win32 (`BY_HANDLE_FILE_INFORMATION`), Linux (`libc::statfs`, `libc::statvfs`), and macOS FFI calls to guarantee type validity invariants.
  - Added explicit `// SAFETY:` rationale documenting all pointer, buffer size, and alignment preconditions across all `unsafe` blocks.
  - Replaced heap-allocated strings and vectors in Windows drive queries with stack-allocated UTF-16 array buffers (`[u16; 4]`).
- **Memory & Allocation Optimizations**:
  - Zero-allocation directory stat recalculation: Eliminated `node.children.clone()` across all directories in `src/tree/stats.rs`, indexing directly into arena child buffers.
  - Zero-copy JSON export: Switched `JsonFile` and `Metadata` serialization to borrow string slices (`&'a str`), removing string allocations per file/directory.
  - Zero-copy CBOR binary export: Updated `CborValue` to borrow string references (`&'a str`), and eliminated `node.children.clone()` during depth-first serialization.
  - Non-cloning binary import: Implemented `Default` for `TreeNode` and used `std::mem::take` to assemble the tree arena directly from decoded nodes without cloning.
  - Avoided transient `format!` allocations in kernel filesystem prefix checks (`strip_prefix`) and UI breadcrumb path formatting (`get_node_path`).
  - Switched config line parser token pushes to `std::mem::take(&mut word)`.
- **Numeric & Overflow Safety**:
  - Fixed signed integer overflow panic on `i64::MIN` in CBOR negative integer encoding by computing `(!val) as u64`.
  - Added boundary checks on positive and negative CBOR integer decoding against `i64::MAX` and `i64::MIN`.
- **API Purity & Terminal RAII Safety**:
  - Pure CLI parsing: `Args::try_parse_from` now cleanly returns `CliError::HelpRequested` and `CliError::VersionRequested` without abruptly calling `std::process::exit(0)`.
  - Unified RAII `TuiSuspender` guard: Integrated `TuiSuspender` in `src/shell.rs` to guarantee terminal raw mode, alternate screen, and mouse capture restoration even on shell errors or panics.
- **Dependency Updates**:
  - Updated 28 crates to latest compatible releases (Rust 1.85 / 2024 edition).

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
