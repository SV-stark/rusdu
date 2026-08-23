# 🦀 rusdu — Rust Disk Usage Analyzer

[![Crates.io](https://img.shields.io/crates/v/rusdu.svg)](https://crates.io/crates/rusdu)
[![Downloads](https://img.shields.io/crates/d/rusdu.svg)](https://crates.io/crates/rusdu)
[![Build Status](https://github.com/SV-stark/rusdu/actions/workflows/ci.yml/badge.svg)](https://github.com/SV-stark/rusdu/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg)](#)

A modern, fast, and feature-complete Rust rewrite of the classic **ncdu** (NCurses Disk Usage) analyzer.

> [!NOTE]  
> **Tribute to the Original**: This project is inspired by and pays tribute to the original **ncdu** tool created by **Yorhel**. For over a decade, `ncdu` has been the gold standard for quick, terminal-based disk analysis. **rusdu** is written from scratch in Rust, maintaining compatibility with the command-line flags, interactive keybindings, and binary/JSON export schemas of `ncdu`, while extending first-class cross-platform support to Windows.

---

## 🚀 Key Features

*   **Fast Multi-Threaded Scanning**: Highly optimized recursive traversal with parallel work-stealing execution (`-t` flag) and interactive abort support (`q` key).
*   **Dual Export & Import Schemas**:
    *   **Streaming JSON Export & Import (`-o` / `-f`)**: Stream JSON output directly conforming to the standard `ncdu` format.
    *   **Binary CBOR Export & Import (`-O` / `-f`)**: Full compatibility with `ncdu` 2.x binary schema featuring compressed CBOR maps, multi-block chunking (`--export-block-size`), index offsets, and bidirectional block pointers.
*   **Rich Interactive TUI**: High-performance terminal interface built on `ratatui` with apparent vs. disk usage toggles, natural sorting, contained item counts, and customizable graph bars.
*   **Live Background File Watcher**: Automatically monitors the active directory for filesystem modifications and alerts you when files change on disk.
*   **Power Navigation & Analytics**:
    *   **Global Fuzzy Search (`f` / `Ctrl+F`)**: Instant tree-wide search across all scanned directories.
    *   **Live Quick Filter (`/`)**: Instant real-time filtering in the current view.
    *   **File Preview Sidebar (`p` / `Tab`)**: Bounded preview pane showing text contents, binary detection, and extended permissions.
    *   **Extension Analytics (`E`)**: Recursive disk usage breakdown and percentage charts by file extension.
    *   **Drive & Mount Selector (`V`)**: Quick-switch between mounted filesystems and Windows drive letters.
*   **Advanced Filtering & Safety**:
    *   Exclusion support for standard shell globs (`--exclude`), cache tags (`CACHEDIR.TAG`), kernel filesystems (`--exclude-kernfs`), and filesystem boundaries (`-x`).
    *   Safe deletion with confirmation dialogs, custom delete commands (`--delete-command`), and layered read-only restrictions (`-r` and `-rr`).
*   **First-Class Cross-Platform Support**: Seamless native operation across Linux, macOS, and Windows (PowerShell, CMD, Windows Terminal) with cluster-size accuracy and volume serial detection.

---

## ⚖️ Comparison with Original ncdu 2.x (Zig)

### Pros of rusdu
*   **Native Windows Support**: The original `ncdu` is primarily POSIX-targeted, whereas `rusdu` runs natively on Windows CMD, PowerShell, and Windows Terminal.
*   **Compile-Time Memory Safety**: Rust's borrow checker prevents memory leaks, dangling pointers, and data races, which is highly beneficial for multi-threaded directory traversals.
*   **Active Ecosystem**: Easier to build, extend, and package using `cargo` with zero external C library dependencies.

### Cons of rusdu
*   **Binary Size**: Compiled Rust TUI binaries are slightly larger (approx 2-3MB stripped) compared to Zig's extremely lightweight, sub-megabyte binaries.
*   **Compilation Times**: Rust's compiler optimizations and TUI dependency tree result in longer compile times than Zig.

---

## 🛠️ Installation & Setup

### Method 1: Install via Cargo (Recommended for Rust users)
You can install `rusdu` directly from [crates.io](https://crates.io/crates/rusdu) using `cargo`:
```bash
cargo install rusdu
```
Make sure your cargo bin directory (usually `~/.cargo/bin` on Unix/macOS or `%USERPROFILE%\.cargo\bin` on Windows) is in your system's `PATH`.

---

### Method 2: Download Pre-built Binaries
You can download the latest pre-compiled binaries from the **[Releases](https://github.com/SV-stark/rusdu/releases)** page:
*   **Linux**: `rusdu-linux-amd64.tar.gz`
*   **macOS**: `rusdu-macos-amd64.tar.gz`
*   **Windows**: `rusdu-windows-amd64.zip`

---

### Method 3: Build from Source
If you prefer to compile manually:
```bash
git clone https://github.com/SV-stark/rusdu.git
cd rusdu
cargo build --release
```
The compiled binary will be located in `target/release/rusdu` (or `target/release/rusdu.exe` on Windows).

---

## 📖 Usage & Commands

```bash
rusdu [PATH] [OPTIONS]
```

### CLI Options

| Flag | Long Form | Description |
| :--- | :--- | :--- |
| `-f <FILE>` | `--import <FILE>` | Load and browse a previously exported JSON or binary file (use `-` for stdin) |
| `-o <FILE>` | `--export-json <FILE>` | Scan directory and stream results to an ncdu-compatible JSON file (use `-` for stdout) |
| `-O <FILE>` | `--export-bin <FILE>` | Scan directory and export results to an ncdu 2.x binary CBOR file (use `-` for stdout) |
| | `--export-block-size <KB>`| Set data block size for binary export in KiB (default: 64) |
| `-c <N>` | `--compress-level <N>` | Set Zstandard compression level for binary export (1–22, default: 3) |
| `-e` | `--extended` | Enable extended metadata mode (captures mtime, uid, gid, mode) |
| | `--no-extended` | Disable extended metadata mode |
| `-x` | `--one-file-system` | Stay on the current filesystem partition / drive |
| | `--cross-file-system` | Allow crossing filesystem boundaries (default) |
| `-L` | `--follow-symlinks` | Follow symbolic links to files (never recurses into symlinked directories) |
| | `--no-follow-symlinks`| Do not follow symbolic links (default) |
| | `--exclude <PATTERN>` | Exclude files/directories matching shell glob (can be specified multiple times) |
| `-X <FILE>` | `--exclude-from <FILE>` | Read exclusion patterns from file (one per line) |
| | `--exclude-caches` | Exclude directories containing `CACHEDIR.TAG` marker files (default) |
| | `--include-caches` | Include cache directories in scan |
| | `--exclude-kernfs` | Exclude Linux pseudo-filesystems (`/proc`, `/sys`, `/dev/pts`, etc., default) |
| | `--include-kernfs` | Include Linux pseudo-filesystems in scan |
| `-t <N>` | `--threads <N>` | Set number of scanning worker threads (default: 1) |
| `-0` | `--silent` | Suppress progress output completely during scan |
| `-1` | `--line-progress` | Simple single-line progress update during scan |
| `-2` | `--fullscreen-progress`| Fullscreen progress screen with scan metrics and 'q' abort (default) |
| `-q` | `--slow-ui-updates` | Update progress UI 2 times per second (500 ms) |
| | `--fast-ui-updates` | Update progress UI 10 times per second (100 ms) |
| `-r` | | Read-only mode (`-r` disables delete; `-rr` also disables shell) |
| | `--enable-delete` / `--disable-delete` | Explicitly enable or disable file deletion |
| | `--enable-shell` / `--disable-shell` | Explicitly enable or disable spawning sub-shells |
| | `--enable-refresh` / `--disable-refresh` | Explicitly enable or disable directory refreshing |
| | `--confirm-quit` / `--no-confirm-quit` | Prompt for confirmation before quitting |
| | `--confirm-delete` / `--no-confirm-delete` | Require confirmation prompt before deleting files |
| | `--si` | Display sizes using SI metric powers of 1000 (kB, MB, GB) |
| | `--no-si` | Display sizes using binary powers of 1024 (KiB, MiB, GiB, default) |
| | `--apparent-size` | Show apparent file sizes instead of disk usage |
| | `--disk-usage` | Show actual allocated disk block usage (default) |
| | `--show-hidden` / `--no-show-hidden` | Show or hide excluded / hidden items by default |
| | `--sort <COLUMN>` | Set initial sort order (`disk-usage-desc`, `name-asc`, `items-desc`, etc.) |
| | `--color <SCHEME>` | Set color theme (`default`, `dark`, `classic`, `off`) |
| | `--graph-style <STYLE>`| Set graph visualization (`both`, `percent`, `graph`, `none`) |
| | `--shared-column <MODE>`| Set shared size column (`off`, `shared`, `unique`) |
| | `--delete-command <CMD>`| Use custom shell command for deletion (path passed via `$NCDU_DELETE_PATH`) |
| | `--icons` | Enable Nerd Font file and directory icons in TUI browser |
| | `--ignore-config` | Do not load configuration files |
| | `--log-file <FILE>` | Write diagnostic logs to specified file path |

---

### Interactive Keybindings & Browser Navigation

*   **Movement**:
    *   `↑` / `k` — Move cursor up
    *   `↓` / `j` — Move cursor down
    *   `Enter` / `l` / `→` — Open selected directory
    *   `Backspace` / `h` / `←` — Return to parent directory
    *   `Page Up` / `Page Down` — Scroll viewport by 10 items
    *   `Home` / `End` — Jump to the top / bottom of the directory listing
*   **Mouse Support**:
    *   Scroll wheel to scroll up/down
    *   Left-click to select an item
    *   Left-click on selected directory (or double-click) to open it
*   **Sorting Modes**:
    *   `s` — Order by disk usage / size (toggles descending/ascending)
    *   `n` — Order by filename (natural sorting by default)
    *   `C` (Shift+C) — Order by contained item count
    *   `M` (Shift+M) — Order by modification time (requires `-e`)
    *   `t` — Toggle grouping directories before files
*   **Display Toggles**:
    *   `a` — Toggle apparent size vs. disk usage
    *   `g` — Cycle graph display (`[#   ]`, percentage `%`, both, none)
    *   `u` — Cycle hard-link size column (shared, unique, off)
    *   `c` — Toggle item count column (contained files + subdirectories)
    *   `m` — Toggle modification time column (requires `-e`)
    *   `e` — Toggle display of excluded / hidden items
    *   `p` / `Tab` — Toggle sidebar file preview panel
*   **Power Features & Analytics**:
    *   `/` — **Instant Interactive Filter**: Live search query in current directory
    *   `f` / `Ctrl+F` — **Global Fuzzy Search**: Tree-wide fuzzy search across all scanned directories
    *   `V` (Shift+V) — **Drive Selector**: Switch active drive / partition (Windows logical drives or Unix mounts)
    *   `E` (Shift+E) — **Extension Analytics**: Recursive breakdown of disk usage by file extension
    *   `y` — Copy absolute path of selected item to system clipboard
    *   `o` — Reveal selected item in system file manager (Explorer / Finder / xdg-open)
    *   `v` — Open selected item in system editor (`$VISUAL` / `$EDITOR` / Notepad)
    *   `b` — Spawn a sub-shell in the currently selected directory
    *   `d` — Delete the currently selected file or directory
    *   `r` — Recalculate / refresh current directory
    *   `i` — Show detailed item information dialog (path, sizes, inode, links, mode, mtime)
    *   `?` / `F1` — Open interactive help dialog
    *   `q` — Quit (or close active dialog)

---

### Item Flag Legend

| Flag | Meaning |
| :---: | :--- |
| `!` | Read error occurred while scanning this entry |
| `.` | Read error occurred in a subdirectory within this entry |
| `<` | Entry excluded by pattern or cache tag |
| `>` | Entry located on a different filesystem (`-x` boundary) |
| `^` | Entry located on a Linux kernel pseudo-filesystem (`--exclude-kernfs`) |
| `@` | Non-regular file (symbolic link, pipe, socket, device) |
| `H` | Duplicate hard-link (already counted towards parent size) |
| `e` | Empty directory |

---

## ⚙️ Configuration & Custom Actions

`rusdu` automatically searches for a configuration file at:
1. `~/.config/rusdu/config` (or `%APPDATA%\rusdu\config` on Windows)
2. `~/.config/ncdu/config` (fallback for drop-in ncdu compatibility)

Options in the configuration file use standard command-line syntax (one option per line). Lines starting with `#` are comments, and lines starting with `@` suppress warnings on unrecognized options:

```ini
# ~/.config/rusdu/config
--extended
--color dark
--exclude-caches
@--unknown-ncdu-option
```

### Custom Actions (`actions.conf`)

You can define custom keybindings in `~/.config/rusdu/actions.conf`:

```ini
# Format: key = command
y = copy
o = file-manager
v = editor
```

---

## 💡 Practical Examples

#### 1. Scan and export to a compressed binary file:
```bash
rusdu /var/log -e -O /tmp/logs.rusdu
```

#### 2. Browse a previously exported file:
```bash
rusdu -f /tmp/logs.rusdu
```

#### 3. Remote scanning over SSH:
```bash
ssh user@remote-host "rusdu / -x -0 -O -" | rusdu -f -
```

---

## 🏗️ Codebase Architecture

*   **`src/tree/`**: Index-based `TreeArena` storing nodes with zero pointer overhead.
*   **`src/scan/`**: Multi-threaded walker, glob matching, cache tag inspection, and native filesystem calls.
*   **`src/export/`**: Custom CBOR map serialization/deserialization, multi-block chunking, and streaming JSON writer.
*   **`src/ui/`**: Responsive terminal interface powered by `ratatui` with mouse support, search, and analytics.

---

## 📜 Changelog & Releases

See the [CHANGELOG.md](CHANGELOG.md) for detailed notes on all releases and updates.

## 🤝 Contributing

Contributions, bug reports, and optimizations are welcome! Feel free to open issues or pull requests on GitHub.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
