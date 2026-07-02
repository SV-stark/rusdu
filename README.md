# 🦀 rusdu — Rust Disk Usage Analyzer

[![Crates.io](https://img.shields.io/crates/v/rusdu.svg)](https://crates.io/crates/rusdu)
[![Build Status](https://github.com/SV-stark/rusdu/actions/workflows/ci.yml/badge.svg)](https://github.com/SV-stark/rusdu/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg)](#)

A modern, fast, and feature-complete Rust rewrite of the classic **ncdu** (NCurses Disk Usage) analyzer.

> [!NOTE]  
> **Tribute to the Original**: This project is inspired by and pays tribute to the original **ncdu** tool created by **Yohan Helajzen (Yorhel)**. For over a decade, `ncdu` has been the gold standard for quick, terminal-based disk analysis. **rusdu** is written from scratch in Rust, preserving 100% compatibility with the command-line flags, interactive keybindings, and binary/JSON export schemas of `ncdu` 2.x, while extending first-class cross-platform support to Windows.

---

## 🚀 Key Features

*   **Fast Directory Traversal**: Recursive scanning optimized with multi-threading support (`-t` flag) via parallel work-stealing execution.
*   **Dual Export Schemas**: 
    *   **JSON Schema**: Stream JSON exports (`-o`) fully compatible with the standard `ncdu` version 1.2 layout.
    *   **Binary CBOR Schema**: Write and parse binary exports (`-O`) with compressed CBOR maps, index offsets, and bidirectional block pointers for minimal memory footprints on large directory trees.
*   **Interactive TUI**: Rich scrollable UI showing directory sizes, apparent vs. disk usage, cumulative counts, and visual bars.
*   **Advanced Filters**: Supports cache exclusions (`CACHEDIR.TAG`), standard globs (`--exclude`), pseudo-filesystems (`--exclude-kernfs`), and filesystem boundaries (`-x`).
*   **Safe Operations**: Custom deletion commands (`--delete-command`), confirmation boxes, and multiple read-only levels (`-r` and `-rr`).
*   **Windows & Unix Cross-Platform Support**: Built on `crossterm` and `ratatui`, enabling native operations on Linux, macOS, and Windows.

---

## ⚖️ Comparison with Original ncdu 2.x (Zig)

### Pros of rusdu
*   **Native Windows Support**: The original `ncdu` is POSIX-bound and relies on `ncurses`, making it incompatible with Windows. `rusdu` runs natively on Windows CMD, PowerShell, and Windows Terminal.
*   **Compile-Time Memory Safety**: Rust's borrow checker prevents memory leaks, dangling pointers, and data races, which is highly beneficial for multi-threaded directory traversals.
*   **Active Ecosystem**: Easier to build, extend, and package using `cargo` compared to Zig's pre-1.0 compiler version build system.

### Cons of rusdu
*   **Binary Size**: Compiled Rust TUI binaries are slightly larger (approx 2-3MB stripped) compared to Zig's extremely lightweight, sub-megabyte binaries.
*   **Compilation Times**: Rust's compiler optimizations and TUI dependency tree result in longer compile times than Zig.

## 🛠️ Installation & Setup

### Method 1: Download Pre-built Binaries (Recommended)
You can download the latest pre-compiled binaries from the **[Nightly Releases](../../releases/tag/nightly)** page:
*   **Linux**: `rusdu-linux-amd64.tar.gz`
*   **macOS**: `rusdu-macos-amd64.tar.gz`
*   **Windows**: `rusdu-windows-amd64.zip`

#### Adding to PATH:

##### 🐧 Linux & 🍎 macOS
1.  Extract the archive and move the binary to a directory in your PATH (e.g., `/usr/local/bin`):
    ```bash
    tar -xzf rusdu-*.tar.gz
    sudo mv rusdu /usr/local/bin/
    ```
2.  Alternatively, place it in a custom folder (e.g., `~/.local/bin`) and append it to your Shell profile (`~/.bashrc`, `~/.zshrc`):
    ```bash
    export PATH="$HOME/.local/bin:$PATH"
    ```
> [!NOTE]  
> On **macOS**, when executing the binary for the first time, you may need to grant permission via System Settings > Privacy & Security or run: `xattr -d com.apple.quarantine /usr/local/bin/rusdu`.

##### 🪟 Windows
1.  Extract `rusdu-windows-amd64.zip` to a folder (e.g., `C:\Program Files\rusdu`).
2.  Add the folder to your user Environment variables:
    *   **Via GUI**: Search for "Edit the system environment variables" > Click "Environment Variables" > Select "Path" under User variables > Click "Edit" > Click "New" > Paste `C:\Program Files\rusdu` > Click OK.
    *   **Via PowerShell (Admin)**:
        ```powershell
        [Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";C:\Program Files\rusdu", "User")
        ```

---

### Method 2: Install via Cargo (Recommended for Rust users)
You can install `rusdu` directly from [crates.io](https://crates.io/crates/rusdu) using `cargo`:
```bash
cargo install rusdu
```
Make sure your cargo bin directory (usually `~/.cargo/bin` on Unix/macOS or `%USERPROFILE%\.cargo\bin` on Windows) is in your system's `PATH`.

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

```
rusdu [PATH] [OPTIONS]
```

### Core CLI Commands

| Flag | Long Form | Description |
| :--- | :--- | :--- |
| `-f <FILE>` | `--import <FILE>` | Load and browse a previously exported JSON or binary file (use `-` for stdin) |
| `-o <FILE>` | `--export-json <FILE>` | Scan the directory and export results to a JSON file (use `-` for stdout) |
| `-O <FILE>` | `--export-bin <FILE>` | Scan and export results to a binary CBOR file (use `-` for stdout) |
| `-x` | `--one-file-system` | Stay on the same filesystem partition |
| | `--cross-file-system` | Cross filesystem boundaries (default, overrides `-x`) |
| `-t <N>` | `--threads <N>` | Set number of scanning threads (default: 1) |
| `-e` | `--extended` | Enable extended info mode (mtime, uid, gid, mode) |
| `-r` | | Read-only mode (`-r` disables deletes; `-rr` also disables shell) |
| | `--icons` | Enable Nerd Font icons (folder/file glyphs) in TUI list |
| | `--log-file <FILE>` | Log errors and scanning diagnostics to a file |
| | `--no-confirm-delete` | Bypass delete confirmation prompts (confirmation is enabled by default) |
| | `--hide-graph` | Hide the relative size bar column (columns show by default) |
| | `--hide-percent` | Hide the relative size percent column (columns show by default) |
| | `--disable-natsort` | Disable natural sorting for filenames (enabled by default) |

### Interactive Keybindings & Mouse in Browser

*   **Keyboard Navigation**: `↑`/`↓` or `k`/`j` to scroll, `→`/`l`/`Enter` to enter directories, `←`/`h`/`Backspace` to go back, `Page Up`/`Page Down` to scroll by 10 items, and `Home`/`End` to jump to the top/bottom.
*   **Mouse Interaction**: Scroll wheel to navigate, left-click to select, left-click on a selected directory (or double-click) to open it.
*   **Sorting**: `n` (by name), `s` (by size), `C` (by item count), `M` (by modification time), `t` (toggle group directories first).
*   **Toggles**: `a` (apparent size), `g` (graph & percent), `u` (shared hardlink size column), `c` (item count column), `m` (mtime column), `e` (hidden files).
*   **Advanced Features**:
    *   `/` — **Live Interactive Filter**: Filters the current list by a case-insensitive query string.
    *   `f` / `Ctrl+F` — **Global Fuzzy Search**: Recursively fuzzy matches paths tree-wide and allows direct jumps.
    *   `Tab` / `p` — **Sidebar File Preview Panel**: Toggles a side panel showing metadata, owner info, permissions, and file content previews.
    *   `v` — **Disk & Drive Selector**: Opens a list of logical system disks/drives to scan and select from.
    *   `E` (Shift+E) — **Extension Analytics**: Displays recursive file extension space-usage distribution tables and percentages.
    *   **Custom Actions & Shell Piping**: Runs custom commands with absolute paths (default bindings: `c` to copy path to clipboard, `o` to reveal in system file manager, `v` to open in editor). Add customized actions in `~/.config/rusdu/actions.conf` (Unix) or `%APPDATA%\rusdu\actions.conf` (Windows) using the format `<key>=<command>` (e.g., `e=nvim`).
*   **Real-time File Watcher**: Automatically monitors the currently browsed directory for modifications. If a background change is detected, a yellow/red `[Disk Changed - Press 'r' to refresh]` badge appears in the header.
*   **Standard Actions**: `d` to delete, `b` to spawn a shell, `r` to refresh, `i` for item info, `?`/`F1` for help, `q` to quit.

---

## 🏗️ Codebase Architecture

The project is designed with highly modular Rust packages:

*   **`src/tree/`**: Allocates nodes inside a flat index-based `TreeArena`, removing recursive pointer overhead.
*   **`src/scan/`**: Platform-specific system calls (Unix `libc` / Windows Win32 APIs) driving fast traversal algorithms.
*   **`src/export/`**: Custom CBOR map serialization, index tables, and Zstandard compression algorithms.
*   **`src/ui/`**: Drawing views and dialogs in `ratatui`.

---

## 🤝 Contributing

Contributions, bug reports, and optimizations are welcome! Feel free to open issues or pull requests. Please make sure to follow the existing coding guidelines and format your code using `cargo fmt`.

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
