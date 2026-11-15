# 🦀 rusdu — Rust 磁盘空间分析器

<div align="center">

[**English**](./README.md) | [**简体中文**](./README.zh-CN.md)

</div>

[![Crates.io](https://img.shields.io/crates/v/rusdu.svg)](https://crates.io/crates/rusdu)
[![Downloads](https://img.shields.io/crates/d/rusdu.svg)](https://crates.io/crates/rusdu)
[![Build Status](https://github.com/SV-stark/rusdu/actions/workflows/ci.yml/badge.svg)](https://github.com/SV-stark/rusdu/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg)](#)

一个现代、快速且功能完备的经典 **ncdu**（NCurses 磁盘使用）分析器的 Rust 重写版。

> [!NOTE]  
> **致敬原作**：本项目受 **Yorhel** 创作的原始 **ncdu** 工具启发，并在此向其致敬。十多年来，`ncdu` 一直是快速、基于终端的磁盘分析的黄金标准。**rusdu** 使用 Rust 从零开始编写，保持了与 `ncdu` 的命令行参数、交互式快捷键以及二进制/JSON 导出模式的兼容性，同时将一流的跨平台支持扩展到了 Windows。

---

## 🚀 主要特性

* **快速多线程扫描**：高度优化的递归遍历，支持并行工作窃取（work-stealing）执行（-t 标志）和交互式中止（q 键）。
* **双重导出与导入格式**：
    * **流式 JSON 导出与导入（`-o` / `-f`）**：直接符合标准 ncdu 格式的 JSON 输出。
    * **二进制 CBOR 导出与导入（`-O` / `-f`）**：完全兼容 ncdu 2.x 二进制格式，支持压缩 CBOR 映射、多块分片（--export-block-size）、索引偏移和双向块指针。
* **丰富的交互式 TUI**：基于 ratatui 的高性能终端界面，支持实际大小与磁盘使用切换、自然排序、包含项计数和可定制图表条。
* **实时后台文件监视器**：自动监控活动目录的文件系统变更，并在文件变化时提醒。
* **强大导航与分析**：
    * **全局模糊搜索（`f` / `Ctrl+F`）**：在所有扫描目录中即时搜索。
    * **实时快速过滤（`/`）**：在当前视图中实时过滤。
    * **文件预览侧边栏（`p` / `Tab`）**：显示文本内容、二进制检测和扩展权限的预览面板。
    * **扩展分析（`E`）**：按文件扩展名递归分析磁盘使用和百分比图表。
    * **驱动器与挂载选择器（`V`）**：快速切换挂载的文件系统和 Windows 驱动器盘符。
* **高级过滤与安全**：
    * 排除支持：标准 shell 通配符（--exclude）、缓存标签（CACHEDIR.TAG）、内核文件系统（--exclude-kernfs）和文件系统边界（-x）。
    * 安全删除：确认对话框、自定义删除命令（--delete-command）和分层只读限制（-r 和 -rr）。
* **一流的跨平台支持**：在 Linux、macOS 和 Windows（PowerShell、CMD、Windows Terminal）上无缝运行，支持簇大小精度和卷序列检测。

---

## ⚖️ 与原始 ncdu 2.x（Zig）的对比

### rusdu 的优势
* **原生 Windows 支持**：原始 ncdu 主要针对 POSIX 系统，而 rusdu 在 Windows CMD、PowerShell 和 Windows Terminal 上原生运行。
* **编译时内存安全**：Rust 的借用检查器防止内存泄漏、悬垂指针和数据竞争，特别适合多线程目录遍历。
* **活跃的生态系统**：通过 cargo 更容易构建、扩展和打包，无需外部 C 库依赖。

### rusdu 的劣势
* **二进制大小**：编译后的 Rust TUI 二进制文件稍大（约 2-3MB 剥离后），而 Zig 的二进制文件极轻量（小于 1MB）。
* **编译时间**：Rust 的编译器优化和 TUI 依赖树导致编译时间比 Zig 长。

---

## 🛠️ 安装与配置

### 方法 1：通过 Cargo 安装（推荐 Rust 用户）
可以从 [crates.io](https://crates.io/crates/rusdu) 使用 `cargo` 直接安装：
```bash
cargo install rusdu
```

确保你的 cargo bin 目录（Unix/macOS 通常是 `~/.cargo/bin`，Windows 是 `%USERPROFILE%\.cargo\bin`）在系统的 `PATH` 中。

---

### 方法 2：下载预编译二进制
从 **[releases](https://github.com/SV-stark/rusdu/releases)** 下载最新的预编译二进制文件：
* **Linux**：`rusdu-linux-amd64.tar.gz`
* **macOS**：`rusdu-macos-amd64.tar.gz`
* **Windows**：`rusdu-windows-amd64.zip`

---

### 方法 3：从源码构建
如需手动编译：
```bash
git clone https://github.com/SV-stark/rusdu.git
cd rusdu
cargo build --release
```
编译后的二进制文件位于 `target/release/rusdu`（在Windows上为 `target/release/rusdu.exe`）

---

## 📖 使用与命令

```bash
1 rusdu [路径] [选项]
```

###     命令行选项


| 参数 | 长格式 | 描述 |
| :--- | :--- | :--- |
| `-f <FILE>` | `--import <FILE>` | 加载并浏览之前导出的 JSON 或二进制文件（使用 `-` 表示标准输入） |
| `-o <FILE>` | `--export-json <FILE>` | 扫描目录并将结果流式传输到兼容 ncdu 的 JSON 文件（使用 `-` 表示标准输出） |
| `-O <FILE>` | `--export-bin <FILE>` | 扫描目录并将结果导出到 ncdu 2.x 二进制 CBOR 文件（使用 `-` 表示标准输出） |
| | `--export-block-size <KB>`| 设置二进制导出的数据块大小，单位为 KiB（默认：64） |
| `-c <N>` | `--compress-level <N>` | 设置二进制导出的 Zstandard 压缩级别（1–22，默认：3） |
| `-e` | `--extended` | 启用扩展元数据模式（捕获 mtime、uid、gid、mode） |
| | `--no-extended` | 禁用扩展元数据模式 |
| `-x` | `--one-file-system` | 停留在当前文件系统分区 / 驱动器 |
| | `--cross-file-system` | 允许跨文件系统边界（默认） |
| `-L` | `--follow-symlinks` | 跟随指向文件的符号链接（绝不递归进入符号链接的目录） |
| | `--no-follow-symlinks`| 不跟随符号链接（默认） |
| | `--exclude <PATTERN>` | 排除匹配 shell 通配符的文件/目录（可多次指定） |
| `-X <FILE>` | `--exclude-from <FILE>` | 从文件读取排除模式（每行一个） |
| | `--exclude-caches` | 排除包含 `CACHEDIR.TAG` 标记文件的目录（默认） |
| | `--include-caches` | 在扫描中包含缓存目录 |
| | `--exclude-kernfs` | 排除 Linux 伪文件系统（`/proc`、`/sys`、`/dev/pts` 等，默认） |
| | `--include-kernfs` | 在扫描中包含 Linux 伪文件系统 |
| `-t <N>` | `--threads <N>` | 设置扫描工作线程数（默认：1） |
| `-0` | `--silent` | 扫描期间完全静默，不输出进度 |
| `-1` | `--line-progress` | 扫描期间显示简单的单行进度更新 |
| `-2` | `--fullscreen-progress`| 带有扫描指标和 'q' 中止的全屏进度界面（默认） |
| `-q` | `--slow-ui-updates` | 每秒更新进度 UI 2 次（500 毫秒） |
| | `--fast-ui-updates` | 每秒更新进度 UI 10 次（100 毫秒） |
| `-r` | | 只读模式（`-r` 禁用删除；`-rr` 同时禁用 shell） |
| | `--enable-delete` / `--disable-delete` | 显式启用或禁用文件删除 |
| | `--enable-shell` / `--disable-shell` | 显式启用或禁用生成子 shell |
| | `--enable-refresh` / `--disable-refresh` | 显式启用或禁用目录刷新 |
| | `--confirm-quit` / `--no-confirm-quit` | 退出前提示确认 |
| | `--confirm-delete` / `--no-confirm-delete` | 删除文件前需要确认提示 |
| | `--si` | 使用 1000 为基数的 SI 公制单位显示大小（kB, MB, GB） |
| | `--no-si` | 使用 1024 为基数的二进制单位显示大小（KiB, MiB, GiB，默认） |
| | `--apparent-size` | 显示表观文件大小而不是磁盘占用 |
| | `--disk-usage` | 显示实际分配的磁盘块占用（默认） |
| | `--show-hidden` / `--no-show-hidden` | 默认显示或隐藏已排除/隐藏的项目 |
| | `--sort <COLUMN>` | 设置初始排序顺序（`disk-usage-desc`、`name-asc`、`items-desc` 等） |
| | `--color <SCHEME>` | 设置颜色主题（`default`、`dark`、`classic`、`off`） |
| | `--graph-style <STYLE>`| 设置图表可视化（`both`、`percent`、`graph`、`none`） |
| | `--shared-column <MODE>`| 设置共享大小列（`off`、`shared`、`unique`） |
| | `--delete-command <CMD>`| 使用自定义 shell 命令进行删除（路径通过 `$NCDU_DELETE_PATH` 传递） |
| | `--icons` | 在 TUI 浏览器中启用 Nerd Font 文件和目录图标 |
| | `--ignore-config` | 不加载配置文件 |
| | `--log-file <FILE>` | 将诊断日志写入指定的文件路径 |

---

### 交互式快捷键与浏览器导航

*   **移动**：
    *   `↑` / `k` — 向上移动光标
    *   `↓` / `j` — 向下移动光标
    *   `Enter` / `l` / `→` — 打开选中的目录
    *   `Backspace` / `h` / `←` — 返回父目录
    *   `Page Up` / `Page Down` — 视口滚动 10 个项目
    *   `Home` / `End` — 跳转到目录列表的顶部 / 底部
*   **支持鼠标操作**：
    *   滚轮向上/向下滚动
    *   左键单击选择项目
    *   左键单击选中的目录（或双击）以打开它
*   **排序模式**：
    *   `s` — 按磁盘占用 / 大小排序（切换降序/升序）
    *   `n` — 按文件名排序（默认自然排序）
    *   `C` (Shift+C) — 按包含的项目数排序
    *   `M` (Shift+M) — 按修改时间排序（需要 `-e`）
    *   `t` — 切换是否将目录分组在文件之前
*   **显示切换**：
    *   `a` — 切换表观大小与磁盘占用
    *   `g` — 循环切换图表显示（`[#   ]`、百分比 `%`、两者都显示、无）
    *   `u` — 循环切换硬链接大小列（共享、唯一、关闭）
    *   `c` — 切换项目计数列（包含的文件 + 子目录）
    *   `m` — 切换修改时间列（需要 `-e`）
    *   `e` — 切换已排除 / 隐藏项目的显示
    *   `p` / `Tab` — 切换侧边栏文件预览面板
*   **强大的功能与分析**：
    *   `/` — **即时交互式过滤**：在当前目录中进行实时搜索查询
    *   `f` / `Ctrl+F` — **全局模糊搜索**：在所有已扫描目录中进行全树模糊搜索
    *   `V` (Shift+V) — **驱动器选择器**：切换活动驱动器 / 分区（Windows 逻辑驱动器或 Unix 挂载点）
    *   `E` (Shift+E) — **扩展名分析**：按文件扩展名递归分析磁盘占用明细
    *   `y` — 将选中项目的绝对路径复制到系统剪贴板
    *   `o` — 在系统文件管理器中显示选中的项目（资源管理器 / Finder / xdg-open）
    *   `v` — 在系统编辑器中打开选中的项目（`$VISUAL` / `$EDITOR` / 记事本）
    *   `b` — 在当前选中的目录中生成一个子 shell
    *   `d` — 删除当前选中的文件或目录
    *   `r` — 重新计算 / 刷新当前目录
    *   `i` — 显示详细的项目信息对话框（路径、大小、inode、链接、模式、mtime）
    *   `?` / `F1` — 打开交互式帮助对话框
    *   `q` — 退出（或关闭当前对话框）

---

### 项目标志图例

| 标志 | 含义 |
| :---: | :--- |
| `!` | 扫描此条目时发生读取错误 |
| `.` | 此条目的子目录中发生读取错误 |
| `<` | 条目被模式或缓存标签排除 |
| `>` | 条目位于不同的文件系统上（`-x` 边界） |
| `^` | 条目位于 Linux 内核伪文件系统上（`--exclude-kernfs`） |
| `@` | 非普通文件（符号链接、管道、套接字、设备） |
| `H` | 重复的硬链接（已计入父级大小） |
| `e` | 空目录 |

---

## ⚙️ 配置与自定义操作

`rusdu` 自动查找配置文件：
1. `~/.config/rusdu/config`（在Windows 为 `%APPDATA%\rusdu\config`）
2. `~/.config/ncdu/config`（兼容 ncdu 的备用路径）

配置文件使用标准命令行语法（每行一个选项）。以 `#` 开头的行是注释，以 `@` 开头的行忽略未知选项警告：

```ini
# ~/.config/rusdu/config
--extended
--color dark
--exclude-caches
@--unknown-ncdu-option
```

### 自定义颜色（`config.toml`）

可通过 `~/.config/rusdu/config.toml`（在Windows 为 `%APPDATA%\rusdu\config.toml`）自定义 UI 元素颜色、修饰符和大小阈值样式：

```toml
[color]
preset = "dark" # optional base: "off", "dark", "dark-bg"
header = { fg = "#ffffff", bg = "#1e3a8a", modifiers = ["bold"] }
footer = { fg = "white", bg = "blue" }
selected = { fg = "black", bg = "cyan", modifiers = ["bold"] }
dir = { fg = "bright_blue", modifiers = ["bold"] }
file = "white"
graph = "green"
percent = "yellow"
shortcut = { fg = "yellow", modifiers = ["bold"] }
border = "gray"
flag = "red"
dialog_bg = "#111111"

[[color.size_thresholds]]
min_size = "1GiB"
fg = "red"
modifiers = ["bold"]

[[color.size_thresholds]]
min_size = "100MiB"
style = "yellow"
```

颜色支持命名颜色（如 `red`、`bright_blue`、`dark_gray` 等）、`#RGB` / `#RRGGBB` 十六进制值、`rgb(r, g, b)` / `[r, g, b]`，或 256 色索引（`0..=255`）。支持的修饰符包括 `bold`（加粗）、`dim`（变暗）、`italic`（斜体）、`underlined`（下划线）、`reversed`（反色）、`crossed_out`（删除线）、`slow_blink`（慢速闪烁）、`rapid_blink`（快速闪烁）和 `hidden`（隐藏）。传入 `--ignore-config` 参数将同时跳过 `config` 和 `config.toml`。

### 自定义快捷键（`actions.conf`）

可以在 `~/.config/rusdu/actions.conf` 中定义自定义快捷键：

```ini
# 格式：键 = 命令
y = copy
o = file-manager
v = editor
```

---

## 💡 示例说明

#### 1. 扫描并导出为压缩二进制文件：
```bash
rusdu /var/log -e -O /tmp/logs.rusdu
```

#### 2. 浏览之前导出的文件：
```bash
rusdu -f /tmp/logs.rusdu
```

#### 3. 通过 SSH 远程扫描：
```bash
ssh user@remote-host "rusdu / -x -0 -O -" | rusdu -f -
```

---

## 🏗️ 代码架构

*   **`src/tree/`**：基于索引的 `TreeArena`，零指针开销存储节点
*   **`src/scan/`**：多线程遍历、通配符匹配、缓存标签检查和原生文件系统调用
*   **`src/export/`**：自定义 CBOR 映射序列化/反序列化、多块分片和流式 JSON 写入
*   **`src/ui/`**：基于 ratatui 的响应式终端界面，支持鼠标、搜索和分析

---

## 📜 更新日志与发布

请参阅 [CHANGELOG.md](CHANGELOG.md)查看有关所有版本和更新的详细说明。

## 🤝 贡献

欢迎贡献代码、提交错误报告和优化建议！请随时在 GitHub 上提交 issue 或 pull request。

## 📄 许可证

本项目根据 MIT 许可证授权 - 详情请参阅 [LICENSE](LICENSE) 文件。