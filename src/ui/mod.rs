mod actions;
mod browser;
mod theme;

use crate::cli::Args;
use crate::tree::{NodeId, TreeArena};
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveInfo {
    pub name: String,
    pub mount_point: std::path::PathBuf,
    pub total_space: u64,
    pub available_space: u64,
}

pub struct AppState {
    pub arena: TreeArena,
    pub current_dir: NodeId,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub history: Vec<(NodeId, usize)>, // Stack of (directory_node_id, selected_index)
    pub args: Args,

    // UI state toggles
    pub apparent_size: bool,
    pub si: bool,
    pub show_itemcount: bool,
    pub show_mtime: bool,
    pub show_hidden: bool,
    pub group_dirs_first: bool,
    pub graph_mode: GraphMode, // cycle: both, percent, graph, none
    pub shared_column_mode: SharedColumnMode, // off, shared, unique

    // Active dialogs
    pub active_dialog: Dialog,
    pub show_icons: bool,
    pub refreshing_rx: Option<std::sync::mpsc::Receiver<Result<TreeArena, String>>>,
    pub visible_children: Vec<NodeId>,

    // New features state
    pub custom_actions: std::collections::HashMap<char, String>,
    pub filter_query: Option<String>,
    pub show_preview: bool,
    pub fs_modified: bool,
    pub watcher: Option<notify::RecommendedWatcher>,
    pub watcher_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
}

impl AppState {
    pub fn update_visible_children(&mut self) {
        self.visible_children = get_visible_children(self, self.current_dir);
    }

    pub fn setup_watcher(&mut self) {
        self.watcher = None;
        self.watcher_rx = None;
        self.fs_modified = false;

        if self.args.import_file.is_some() {
            return;
        }

        let current_path = get_node_path(&self.arena, self.current_dir);
        if !current_path.exists() {
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        use notify::Watcher;
        let watcher_res = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let _ = tx.send(res);
        });

        if let Ok(mut w) = watcher_res {
            if w.watch(&current_path, notify::RecursiveMode::NonRecursive)
                .is_ok()
            {
                self.watcher = Some(w);
                self.watcher_rx = Some(rx);
            }
        }
    }

    pub fn allow_delete(&self) -> bool {
        let is_import = self.args.import_file.is_some();
        if self.args.disable_delete {
            false
        } else if self.args.enable_delete {
            true
        } else if is_import {
            false
        } else {
            self.args.read_only < 1
        }
    }

    pub fn allow_shell(&self) -> bool {
        let is_import = self.args.import_file.is_some();
        if self.args.disable_shell {
            false
        } else if self.args.enable_shell {
            true
        } else if is_import {
            false
        } else {
            self.args.read_only < 2
        }
    }

    pub fn allow_refresh(&self) -> bool {
        let is_import = self.args.import_file.is_some();
        if self.args.disable_refresh {
            false
        } else if self.args.enable_refresh {
            true
        } else if is_import {
            false
        } else {
            self.args.read_only < 1
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMode {
    Both,
    Percent,
    Graph,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedColumnMode {
    Off,
    Shared,
    Unique,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    None,
    Help(HelpPage),
    Info(NodeId),
    ConfirmDelete(NodeId),
    ConfirmQuit,
    FilterInput(String),
    FuzzySearch {
        query: String,
        results: Vec<(NodeId, String)>,
        selected_idx: usize,
    },
    DriveSelector {
        drives: Vec<DriveInfo>,
        selected_idx: usize,
    },
    ExtensionAnalytics {
        stats: Vec<(String, u64)>,
        selected_idx: usize,
        scroll_offset: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpPage {
    Keys,
    Format,
    About,
}

pub fn run_tui(arena: TreeArena, args: Args) -> Result<()> {
    // Set up raw mode and alternate screen
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
        crossterm::event::EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState {
        current_dir: arena.root,
        arena,
        selected_idx: 0,
        scroll_offset: 0,
        history: Vec::new(),
        apparent_size: args.apparent_size,
        si: args.si,
        show_itemcount: args.show_itemcount,
        show_mtime: args.show_mtime,
        show_hidden: !args.hide_hidden,
        group_dirs_first: args.group_directories_first,
        graph_mode: match (args.hide_graph, args.hide_percent) {
            (true, true) => GraphMode::None,
            (false, true) => GraphMode::Graph,
            (true, false) => GraphMode::Percent,
            (false, false) => GraphMode::Both,
        },
        shared_column_mode: match args.shared_column.as_str() {
            "off" => SharedColumnMode::Off,
            "unique" => SharedColumnMode::Unique,
            _ => SharedColumnMode::Shared,
        },
        active_dialog: Dialog::None,
        show_icons: args.icons,
        refreshing_rx: None,
        args,
        visible_children: Vec::new(),
        custom_actions: actions::load_custom_actions(),
        filter_query: None,
        show_preview: false,
        fs_modified: false,
        watcher: None,
        watcher_rx: None,
    };
    state.update_visible_children();
    state.setup_watcher();

    // Render loop
    loop {
        // Check filesystem watcher channel
        if let Some(ref rx) = state.watcher_rx {
            while let Ok(Ok(event)) = rx.try_recv() {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    state.fs_modified = true;
                }
            }
        }

        // Check background refresh channel
        if let Some(ref rx) = state.refreshing_rx {
            if let Ok(res) = rx.try_recv() {
                if let Ok(new_arena) = res {
                    state.arena.get_mut(state.current_dir).children =
                        new_arena.nodes[new_arena.root.0].children.clone();
                    if state.current_dir == state.arena.root {
                        state.arena = new_arena;
                        state.selected_idx = 0;
                        state.scroll_offset = 0;
                    }
                }
                state.refreshing_rx = None;
                state.update_visible_children();
                state.setup_watcher();
            }
        }

        terminal.draw(|f| browser::draw(f, &mut state))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            if state.refreshing_rx.is_some() {
                continue;
            }
            match ev {
                Event::Key(key) => {
                    // Ignore key releases
                    if key.kind == event::KeyEventKind::Release {
                        continue;
                    }

                    // Global abort/exit checks
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break;
                    }

                    if state.active_dialog != Dialog::None {
                        if handle_dialog_keys(key.code, &mut state)? {
                            continue;
                        }
                    } else {
                        if handle_browser_keys(key, &mut state)? {
                            break;
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse_event(mouse, &mut state)?;
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
        crossterm::event::DisableMouseCapture
    )?;

    Ok(())
}

fn handle_mouse_event(mouse: MouseEvent, state: &mut AppState) -> Result<()> {
    if state.active_dialog != Dialog::None {
        return Ok(());
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if state.selected_idx > 0 {
                state.selected_idx -= 1;
            }
        }
        MouseEventKind::ScrollDown => {
            let len = state.visible_children.len();
            if len > 0 && state.selected_idx < len - 1 {
                state.selected_idx += 1;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let list_row = mouse.row as usize;
            if list_row >= 1 {
                let clicked_idx = state.scroll_offset + (list_row - 1);
                if clicked_idx < state.visible_children.len() {
                    if state.selected_idx == clicked_idx {
                        let selected_id = state.visible_children[state.selected_idx];
                        if state.arena.get(selected_id).is_dir() {
                            state.history.push((state.current_dir, state.selected_idx));
                            state.current_dir = selected_id;
                            state.selected_idx = 0;
                            state.scroll_offset = 0;
                            state.update_visible_children();
                        }
                    } else {
                        state.selected_idx = clicked_idx;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_dialog_keys(code: KeyCode, state: &mut AppState) -> Result<bool> {
    match &state.active_dialog {
        Dialog::Help(page) => match code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
                state.active_dialog = Dialog::None;
            }
            KeyCode::Char('1') => {
                state.active_dialog = Dialog::Help(HelpPage::Keys);
            }
            KeyCode::Char('2') => {
                state.active_dialog = Dialog::Help(HelpPage::Format);
            }
            KeyCode::Char('3') => {
                state.active_dialog = Dialog::Help(HelpPage::About);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let prev_page = match page {
                    HelpPage::Keys => HelpPage::About,
                    HelpPage::Format => HelpPage::Keys,
                    HelpPage::About => HelpPage::Format,
                };
                state.active_dialog = Dialog::Help(prev_page);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let next_page = match page {
                    HelpPage::Keys => HelpPage::Format,
                    HelpPage::Format => HelpPage::About,
                    HelpPage::About => HelpPage::Keys,
                };
                state.active_dialog = Dialog::Help(next_page);
            }
            _ => {}
        },
        Dialog::Info(_) => match code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('i') | KeyCode::Enter => {
                state.active_dialog = Dialog::None;
            }
            _ => {}
        },
        Dialog::ConfirmDelete(node_id) => {
            let node_id = *node_id;
            match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let item_path = get_node_path(&state.arena, node_id);
                    let read_only = !state.allow_delete();
                    if let Err(e) = crate::delete::delete_item(
                        &item_path,
                        state.args.delete_command.as_deref(),
                        read_only,
                    ) {
                        log::error!("Delete failed: {}", e);
                    } else {
                        // Delete successfully from memory
                        state.arena.delete_node(node_id);
                        // Recalculate sizes
                        crate::tree::stats::recalculate_stats(&mut state.arena);
                        state.update_visible_children();
                        // Reset cursor if out of bounds
                        let items = &state.visible_children;
                        if state.selected_idx >= items.len() && !items.is_empty() {
                            state.selected_idx = items.len() - 1;
                        }
                    }
                    state.active_dialog = Dialog::None;
                }
                KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                    state.active_dialog = Dialog::None;
                }
                _ => {}
            }
        }
        Dialog::ConfirmQuit => {
            match code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    return Ok(false); // Signal exit loop
                }
                KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                    state.active_dialog = Dialog::None;
                }
                _ => {}
            }
        }
        Dialog::FilterInput(query) => {
            let mut q = query.clone();
            match code {
                KeyCode::Esc => {
                    state.filter_query = None;
                    state.active_dialog = Dialog::None;
                    state.update_visible_children();
                }
                KeyCode::Enter => {
                    if q.trim().is_empty() {
                        state.filter_query = None;
                    } else {
                        state.filter_query = Some(q);
                    }
                    state.active_dialog = Dialog::None;
                    state.update_visible_children();
                }
                KeyCode::Backspace => {
                    q.pop();
                    state.active_dialog = Dialog::FilterInput(q);
                }
                KeyCode::Char(c) => {
                    q.push(c);
                    state.active_dialog = Dialog::FilterInput(q);
                }
                _ => {}
            }
        }
        Dialog::FuzzySearch {
            query,
            results,
            selected_idx,
        } => {
            let mut q = query.clone();
            let res = results.clone();
            let mut sel = *selected_idx;
            match code {
                KeyCode::Esc => {
                    state.active_dialog = Dialog::None;
                }
                KeyCode::Enter => {
                    if sel < res.len() {
                        let target_id = res[sel].0;
                        state.active_dialog = Dialog::None;
                        jump_to_node(state, target_id);
                    } else {
                        state.active_dialog = Dialog::None;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if sel > 0 {
                        sel -= 1;
                        state.active_dialog = Dialog::FuzzySearch {
                            query: q,
                            results: res,
                            selected_idx: sel,
                        };
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !res.is_empty() && sel < res.len() - 1 {
                        sel += 1;
                        state.active_dialog = Dialog::FuzzySearch {
                            query: q,
                            results: res,
                            selected_idx: sel,
                        };
                    }
                }
                KeyCode::Backspace => {
                    q.pop();
                    let updated_results = update_fuzzy_results(&state.arena, &q);
                    state.active_dialog = Dialog::FuzzySearch {
                        query: q,
                        results: updated_results,
                        selected_idx: 0,
                    };
                }
                KeyCode::Char(c) => {
                    q.push(c);
                    let updated_results = update_fuzzy_results(&state.arena, &q);
                    state.active_dialog = Dialog::FuzzySearch {
                        query: q,
                        results: updated_results,
                        selected_idx: 0,
                    };
                }
                _ => {}
            }
        }
        Dialog::DriveSelector {
            drives,
            selected_idx,
        } => {
            let mut sel = *selected_idx;
            match code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    state.active_dialog = Dialog::None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if sel > 0 {
                        sel -= 1;
                        state.active_dialog = Dialog::DriveSelector {
                            drives: drives.clone(),
                            selected_idx: sel,
                        };
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !drives.is_empty() && sel < drives.len() - 1 {
                        sel += 1;
                        state.active_dialog = Dialog::DriveSelector {
                            drives: drives.clone(),
                            selected_idx: sel,
                        };
                    }
                }
                KeyCode::Enter if sel < drives.len() => {
                    let path = drives[sel].mount_point.clone();
                    state.active_dialog = Dialog::None;
                    state.history.clear();
                    state.selected_idx = 0;
                    state.scroll_offset = 0;

                    let opts = crate::scan::ScanOptions::from_args(&state.args);
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let res = crate::scan::scan_directory(
                            &path,
                            opts,
                            crate::scan::ProgressMode::Silent,
                        )
                        .map_err(|e| e.to_string());
                        let _ = tx.send(res);
                    });
                    state.refreshing_rx = Some(rx);
                }
                _ => {}
            }
        }
        Dialog::ExtensionAnalytics {
            stats,
            selected_idx,
            scroll_offset,
        } => {
            let mut sel = *selected_idx;
            let mut scroll = *scroll_offset;
            match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                    state.active_dialog = Dialog::None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if sel > 0 {
                        sel -= 1;
                        if sel < scroll {
                            scroll = sel;
                        }
                        state.active_dialog = Dialog::ExtensionAnalytics {
                            stats: stats.clone(),
                            selected_idx: sel,
                            scroll_offset: scroll,
                        };
                    }
                }
                KeyCode::Down | KeyCode::Char('j')
                    if !stats.is_empty() && sel < stats.len() - 1 =>
                {
                    sel += 1;
                    if sel >= scroll + 10 {
                        scroll = sel - 10 + 1;
                    }
                    state.active_dialog = Dialog::ExtensionAnalytics {
                        stats: stats.clone(),
                        selected_idx: sel,
                        scroll_offset: scroll,
                    };
                }
                _ => {}
            }
        }
        Dialog::None => {}
    }
    Ok(true)
}

fn handle_browser_keys(key: event::KeyEvent, state: &mut AppState) -> Result<bool> {
    // Check Ctrl+F or 'f' for global fuzzy search
    let is_ctrl_f = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f');
    let is_f = key.code == KeyCode::Char('f');
    if is_ctrl_f || is_f {
        state.active_dialog = Dialog::FuzzySearch {
            query: String::new(),
            results: Vec::new(),
            selected_idx: 0,
        };
        return Ok(false);
    }

    match key.code {
        // Navigation keys
        KeyCode::Char('q') => {
            if state.args.confirm_quit {
                state.active_dialog = Dialog::ConfirmQuit;
            } else {
                return Ok(true); // Signal exit loop
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.selected_idx > 0 {
                state.selected_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = state.visible_children.len();
            if len > 0 && state.selected_idx < len - 1 {
                state.selected_idx += 1;
            }
        }
        KeyCode::PageUp => {
            if state.selected_idx > 10 {
                state.selected_idx -= 10;
            } else {
                state.selected_idx = 0;
            }
        }
        KeyCode::PageDown => {
            let len = state.visible_children.len();
            if len > 0 {
                if state.selected_idx + 10 < len {
                    state.selected_idx += 10;
                } else {
                    state.selected_idx = len - 1;
                }
            }
        }
        KeyCode::Home => {
            state.selected_idx = 0;
        }
        KeyCode::End => {
            let len = state.visible_children.len();
            if len > 0 {
                state.selected_idx = len - 1;
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            if !state.visible_children.is_empty() {
                let selected_id = state.visible_children[state.selected_idx];
                if state.arena.get(selected_id).is_dir() {
                    state.history.push((state.current_dir, state.selected_idx));
                    state.current_dir = selected_id;
                    state.selected_idx = 0;
                    state.scroll_offset = 0;
                    state.update_visible_children();
                    state.setup_watcher();
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            if let Some((parent_id, prev_idx)) = state.history.pop() {
                state.current_dir = parent_id;
                state.selected_idx = prev_idx;
                state.scroll_offset = 0;
                state.update_visible_children();
                state.setup_watcher();
            }
        }

        // Live filter
        KeyCode::Char('/') => {
            state.active_dialog = Dialog::FilterInput(String::new());
        }

        // Preview panel toggling
        KeyCode::Tab | KeyCode::Char('p') => {
            state.show_preview = !state.show_preview;
        }

        // Disk/Drive Selector
        KeyCode::Char('V') => {
            use sysinfo::Disks;
            let disks = Disks::new_with_refreshed_list();
            let mut drives = Vec::new();
            for disk in &disks {
                let name = disk.name().to_string_lossy().into_owned();
                let mount_point = disk.mount_point().to_path_buf();
                let total_space = disk.total_space();
                let available_space = disk.available_space();
                drives.push(DriveInfo {
                    name: if name.is_empty() {
                        "Local Disk".to_string()
                    } else {
                        name
                    },
                    mount_point,
                    total_space,
                    available_space,
                });
            }
            state.active_dialog = Dialog::DriveSelector {
                drives,
                selected_idx: 0,
            };
        }

        // Extension analytics
        KeyCode::Char('E') => {
            let stats = calculate_extension_stats(&state.arena, state.current_dir);
            state.active_dialog = Dialog::ExtensionAnalytics {
                stats,
                selected_idx: 0,
                scroll_offset: 0,
            };
        }

        // Sorting toggles
        KeyCode::Char('n') => {
            toggle_sort(state, "name");
            state.update_visible_children();
        }
        KeyCode::Char('s') => {
            toggle_sort(state, "disk-usage");
            state.update_visible_children();
        }
        KeyCode::Char('C') => {
            toggle_sort(state, "itemcount");
            state.update_visible_children();
        }
        KeyCode::Char('M') => {
            if state.args.extended {
                toggle_sort(state, "mtime");
                state.update_visible_children();
            }
        }
        KeyCode::Char('t') => {
            state.group_dirs_first = !state.group_dirs_first;
            state.update_visible_children();
        }

        // UI Toggles
        KeyCode::Char('a') => {
            state.apparent_size = !state.apparent_size;
            state.update_visible_children();
        }
        KeyCode::Char('g') => {
            state.graph_mode = match state.graph_mode {
                GraphMode::Both => GraphMode::Percent,
                GraphMode::Percent => GraphMode::Graph,
                GraphMode::Graph => GraphMode::None,
                GraphMode::None => GraphMode::Both,
            };
        }
        KeyCode::Char('u') => {
            state.shared_column_mode = match state.shared_column_mode {
                SharedColumnMode::Off => SharedColumnMode::Shared,
                SharedColumnMode::Shared => SharedColumnMode::Unique,
                SharedColumnMode::Unique => SharedColumnMode::Off,
            };
        }
        KeyCode::Char('c') => {
            state.show_itemcount = !state.show_itemcount;
        }
        KeyCode::Char('m') => {
            if state.args.extended {
                state.show_mtime = !state.show_mtime;
            }
        }
        KeyCode::Char('e') => {
            state.show_hidden = !state.show_hidden;
            state.update_visible_children();
        }

        // Dialogs & Actions
        KeyCode::Char('?') => {
            state.active_dialog = Dialog::Help(HelpPage::Keys);
        }
        KeyCode::Char('i') => {
            if !state.visible_children.is_empty() {
                state.active_dialog = Dialog::Info(state.visible_children[state.selected_idx]);
            }
        }
        KeyCode::Char('d') => {
            if state.allow_delete() && !state.visible_children.is_empty() {
                let selected_id = state.visible_children[state.selected_idx];
                if !state.args.no_confirm_delete {
                    state.active_dialog = Dialog::ConfirmDelete(selected_id);
                } else {
                    let item_path = get_node_path(&state.arena, selected_id);
                    let _ = crate::delete::delete_item(
                        &item_path,
                        state.args.delete_command.as_deref(),
                        false,
                    );
                    state.arena.delete_node(selected_id);
                    crate::tree::stats::recalculate_stats(&mut state.arena);
                    state.update_visible_children();
                }
            }
        }
        KeyCode::Char('b') => {
            if state.allow_shell() {
                let current_path = get_node_path(&state.arena, state.current_dir);
                let _ = crate::shell::spawn_shell(&current_path, false);
            }
        }
        KeyCode::Char('r') => {
            if state.allow_refresh() && state.refreshing_rx.is_none() {
                let current_path = get_node_path(&state.arena, state.current_dir);
                let opts = crate::scan::ScanOptions::from_args(&state.args);
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let res = crate::scan::scan_directory(
                        &current_path,
                        opts,
                        crate::scan::ProgressMode::Silent,
                    )
                    .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
                state.refreshing_rx = Some(rx);
            }
        }
        KeyCode::Char(c) => {
            if let Some(cmd) = state.custom_actions.get(&c).cloned() {
                if !state.visible_children.is_empty() {
                    let selected_id = state.visible_children[state.selected_idx];
                    let selected_path = get_node_path(&state.arena, selected_id);
                    let _ = actions::execute_custom_action(&cmd, &selected_path);
                    state.update_visible_children();
                    state.setup_watcher();
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn toggle_sort(state: &mut AppState, col: &str) {
    if state.args.sort.starts_with(col) {
        if state.args.sort.ends_with("-desc") {
            state.args.sort = format!("{}-asc", col);
        } else {
            state.args.sort = format!("{}-desc", col);
        }
    } else {
        state.args.sort = format!("{}-desc", col);
    }
}

pub fn get_visible_children(state: &AppState, dir_id: NodeId) -> Vec<NodeId> {
    let dir = state.arena.get(dir_id);
    let mut children = dir.children.clone();

    // Filter hidden/excluded if show_hidden is false
    if !state.show_hidden {
        children.retain(|&id| {
            let child = state.arena.get(id);
            !child.flags.contains(crate::tree::EntryFlags::EXCLUDED)
        });
    }

    // Filter by live query if active
    if let Some(ref query) = state.filter_query {
        let query_lower = query.to_lowercase();
        children.retain(|&id| {
            let child = state.arena.get(id);
            child.name.to_lowercase().contains(&query_lower)
        });
    }

    // Sort the list based on state.args.sort and state.group_dirs_first
    children.sort_by(|&a_id, &b_id| {
        let a = state.arena.get(a_id);
        let b = state.arena.get(b_id);

        if state.group_dirs_first {
            if a.is_dir() && !b.is_dir() {
                return std::cmp::Ordering::Less;
            }
            if !a.is_dir() && b.is_dir() {
                return std::cmp::Ordering::Greater;
            }
        }

        let is_desc = state.args.sort.ends_with("-desc");
        let sort_col = state
            .args
            .sort
            .strip_suffix("-desc")
            .or_else(|| state.args.sort.strip_suffix("-asc"))
            .unwrap_or(&state.args.sort);

        let ord = match sort_col {
            "name" => {
                if !state.args.disable_natsort {
                    natord::compare(&a.name, &b.name)
                } else {
                    a.name.cmp(&b.name)
                }
            }
            "apparent-size" => {
                let a_sz = if a.is_dir() {
                    a.stats.total_asize
                } else {
                    a.asize
                };
                let b_sz = if b.is_dir() {
                    b.stats.total_asize
                } else {
                    b.asize
                };
                a_sz.cmp(&b_sz)
            }
            "itemcount" => a.stats.item_count.cmp(&b.stats.item_count),
            "mtime" => {
                let a_time = a.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
                let b_time = b.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
                a_time.cmp(&b_time)
            }
            _ => {
                // Default: disk-usage
                let a_sz = if a.is_dir() {
                    a.stats.total_dsize
                } else {
                    a.dsize
                };
                let b_sz = if b.is_dir() {
                    b.stats.total_dsize
                } else {
                    b.dsize
                };
                a_sz.cmp(&b_sz)
            }
        };

        if is_desc { ord.reverse() } else { ord }
    });

    children
}

pub fn get_node_path(arena: &TreeArena, node_id: NodeId) -> std::path::PathBuf {
    let mut path_components = Vec::new();
    let mut curr = node_id;

    loop {
        let node = arena.get(curr);
        path_components.push(node.name.to_string());
        if let Some(p) = node.parent {
            curr = p;
        } else {
            break;
        }
    }

    path_components.reverse();

    // Join path components
    let mut path = std::path::PathBuf::new();
    for comp in path_components {
        path.push(comp);
    }
    path
}

fn fuzzy_match(text: &str, query: &str) -> bool {
    let mut text_chars = text.chars().flat_map(|c| c.to_lowercase());
    for q_char in query.chars().flat_map(|c| c.to_lowercase()) {
        if text_chars
            .by_ref()
            .find(|&t_char| t_char == q_char)
            .is_none()
        {
            return false;
        }
    }
    true
}

fn update_fuzzy_results(arena: &TreeArena, query: &str) -> Vec<(NodeId, String)> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Vec::new();
    }
    let query_chars: Vec<char> = query_trimmed
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect();
    if query_chars.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut stack = vec![(arena.root, 0usize)];

    while let Some((node_id, parent_matched_len)) = stack.pop() {
        let node = arena.get(node_id);
        let mut matched_len = parent_matched_len;

        let mut name_chars = node.name.chars().flat_map(|c| c.to_lowercase());
        while matched_len < query_chars.len() {
            let q_char = query_chars[matched_len];
            if name_chars
                .by_ref()
                .find(|&t_char| t_char == q_char)
                .is_some()
            {
                matched_len += 1;
            } else {
                break;
            }
        }

        let name_matches = fuzzy_match(&node.name, query_trimmed);
        let path_matches = matched_len == query_chars.len();

        if name_matches || path_matches {
            results.push((
                node_id,
                get_node_path(arena, node_id).to_string_lossy().into_owned(),
            ));
            if results.len() >= 50 {
                break;
            }
        }

        if node.is_dir() {
            let child_matched_len =
                if matched_len < query_chars.len() && query_chars[matched_len] == '/' {
                    matched_len + 1
                } else {
                    matched_len
                };
            for &child_id in node.children.iter().rev() {
                stack.push((child_id, child_matched_len));
            }
        }
    }

    results
}

fn jump_to_node(state: &mut AppState, target_id: NodeId) {
    let mut path_nodes = Vec::new();
    let mut curr = target_id;

    loop {
        path_nodes.push(curr);
        if let Some(parent) = state.arena.get(curr).parent {
            curr = parent;
        } else {
            break;
        }
    }
    path_nodes.reverse();

    state.history.clear();
    state.current_dir = state.arena.root;
    state.selected_idx = 0;
    state.scroll_offset = 0;

    let (target_dir, focus_id) = if state.arena.get(target_id).is_dir() {
        (target_id, None)
    } else {
        let parent = state
            .arena
            .get(target_id)
            .parent
            .unwrap_or(state.arena.root);
        (parent, Some(target_id))
    };

    let mut curr_dir = state.arena.root;
    for &next_id in &path_nodes {
        if next_id == state.arena.root {
            continue;
        }
        if next_id == target_dir && focus_id.is_some() {
            break;
        }
        if state.arena.get(curr_dir).is_dir() {
            state.current_dir = curr_dir;
            state.update_visible_children();
            let children = state.visible_children.clone();
            if let Some(idx) = children.iter().position(|&id| id == next_id) {
                state.history.push((curr_dir, idx));
            }
            curr_dir = next_id;
        }
    }

    state.current_dir = target_dir;
    state.update_visible_children();
    if let Some(fid) = focus_id {
        if let Some(idx) = state.visible_children.iter().position(|&id| id == fid) {
            state.selected_idx = idx;
        } else {
            state.selected_idx = 0;
        }
    } else {
        state.selected_idx = 0;
    }
    state.scroll_offset = 0;
    state.setup_watcher();
}

fn calculate_extension_stats(arena: &TreeArena, dir_id: NodeId) -> Vec<(String, u64)> {
    let mut ext_sizes = std::collections::HashMap::new();
    let mut stack = vec![dir_id];
    while let Some(node_id) = stack.pop() {
        let node = arena.get(node_id);
        if node.is_dir() {
            for &child_id in &node.children {
                stack.push(child_id);
            }
        } else {
            let ext = std::path::Path::new(&*node.name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "no extension".to_string());
            *ext_sizes.entry(ext).or_insert(0) += node.dsize as u64;
        }
    }
    let mut list: Vec<(String, u64)> = ext_sizes.into_iter().collect();
    list.sort_by_key(|b| std::cmp::Reverse(b.1));
    list
}
