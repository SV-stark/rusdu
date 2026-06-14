mod browser;
mod theme;

use crate::cli::Args;
use crate::tree::{NodeId, TreeArena};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

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
        crossterm::cursor::Hide
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
        show_hidden: args.show_hidden,
        group_dirs_first: args.group_directories_first,
        graph_mode: match (args.show_graph, args.show_percent) {
            (true, true) => GraphMode::Both,
            (false, true) => GraphMode::Percent,
            (true, false) => GraphMode::Graph,
            (false, false) => GraphMode::None,
        },
        shared_column_mode: match args.shared_column.as_str() {
            "off" => SharedColumnMode::Off,
            "unique" => SharedColumnMode::Unique,
            _ => SharedColumnMode::Shared,
        },
        active_dialog: Dialog::None,
        args,
    };

    // Render loop
    loop {
        terminal.draw(|f| browser::draw(f, &mut state))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Ignore key releases
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                // Global abort/exit checks
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                if state.active_dialog != Dialog::None {
                    if handle_dialog_keys(key.code, &mut state)? {
                        continue;
                    }
                } else {
                    if handle_browser_keys(key.code, &mut state)? {
                        break;
                    }
                }
            }
        }
    }

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;

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
                    // Perform deletion
                    let item_path = get_node_path(&state.arena, node_id);
                    let read_only = state.args.read_only >= 1;
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
                        // Reset cursor if out of bounds
                        let items = get_visible_children(state, state.current_dir);
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
        Dialog::None => {}
    }
    Ok(true)
}

fn handle_browser_keys(code: KeyCode, state: &mut AppState) -> Result<bool> {
    let visible_children = get_visible_children(state, state.current_dir);

    match code {
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
            if !visible_children.is_empty() && state.selected_idx < visible_children.len() - 1 {
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
            if !visible_children.is_empty() {
                if state.selected_idx + 10 < visible_children.len() {
                    state.selected_idx += 10;
                } else {
                    state.selected_idx = visible_children.len() - 1;
                }
            }
        }
        KeyCode::Home => {
            state.selected_idx = 0;
        }
        KeyCode::End => {
            if !visible_children.is_empty() {
                state.selected_idx = visible_children.len() - 1;
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            if !visible_children.is_empty() {
                let selected_id = visible_children[state.selected_idx];
                if state.arena.get(selected_id).is_dir() {
                    state.history.push((state.current_dir, state.selected_idx));
                    state.current_dir = selected_id;
                    state.selected_idx = 0;
                    state.scroll_offset = 0;
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            if let Some((parent_id, prev_idx)) = state.history.pop() {
                state.current_dir = parent_id;
                state.selected_idx = prev_idx;
                state.scroll_offset = 0;
            }
        }

        // Sorting toggles
        KeyCode::Char('n') => {
            // Sort by name
            toggle_sort(state, "name");
        }
        KeyCode::Char('s') => {
            // Sort by size
            toggle_sort(state, "disk-usage");
        }
        KeyCode::Char('C') => {
            // Sort by item count
            toggle_sort(state, "itemcount");
        }
        KeyCode::Char('M') => {
            // Sort by mtime
            if state.args.extended {
                toggle_sort(state, "mtime");
            }
        }
        KeyCode::Char('t') => {
            state.group_dirs_first = !state.group_dirs_first;
        }

        // UI Toggles
        KeyCode::Char('a') => {
            state.apparent_size = !state.apparent_size;
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
        }

        // Dialogs & Actions
        KeyCode::Char('?') => {
            state.active_dialog = Dialog::Help(HelpPage::Keys);
        }
        KeyCode::Char('i') => {
            if !visible_children.is_empty() {
                state.active_dialog = Dialog::Info(visible_children[state.selected_idx]);
            }
        }
        KeyCode::Char('d') => {
            if !visible_children.is_empty() {
                let selected_id = visible_children[state.selected_idx];
                if state.args.confirm_delete {
                    state.active_dialog = Dialog::ConfirmDelete(selected_id);
                } else {
                    // Delete immediately
                    let item_path = get_node_path(&state.arena, selected_id);
                    let read_only = state.args.read_only >= 1;
                    let _ = crate::delete::delete_item(
                        &item_path,
                        state.args.delete_command.as_deref(),
                        read_only,
                    );
                    state.arena.delete_node(selected_id);
                    crate::tree::stats::recalculate_stats(&mut state.arena);
                }
            }
        }
        KeyCode::Char('b') => {
            let current_path = get_node_path(&state.arena, state.current_dir);
            let read_only = state.args.read_only >= 2;
            let _ = crate::shell::spawn_shell(&current_path, read_only);
        }
        KeyCode::Char('r') => {
            // Recalculate/refresh from disk if supported
            if state.args.read_only < 1 {
                let current_path = get_node_path(&state.arena, state.current_dir);
                let opts = crate::scan::ScanOptions {
                    one_file_system: state.args.one_file_system,
                    exclude_patterns: state.args.exclude.clone(),
                    exclude_from: state.args.exclude_from.clone(),
                    exclude_caches: state.args.exclude_caches,
                    exclude_kernfs: state.args.exclude_kernfs,
                    follow_symlinks: state.args.follow_symlinks,
                    threads: state.args.threads.unwrap_or(1),
                    extended: state.args.extended,
                };
                if let Ok(new_arena) = crate::scan::scan_directory(
                    &current_path,
                    opts,
                    crate::scan::ProgressMode::Silent,
                ) {
                    // Replace children of current dir with newly scanned tree
                    state.arena.get_mut(state.current_dir).children =
                        new_arena.nodes[new_arena.root.0].children.clone();
                    // Copy node data
                    // For simplicity, let's just update the whole subtree or merge.
                    // A simple refresh merges nodes or replaces the arena entirely. Let's merge or replace:
                    // If refreshing from root, we can just replace the whole arena!
                    if state.current_dir == state.arena.root {
                        state.arena = new_arena;
                        state.selected_idx = 0;
                    }
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
        let sort_col = state.args.sort.split('-').next().unwrap_or("disk-usage");

        let ord = match sort_col {
            "name" => {
                if state.args.enable_natsort {
                    crate::natsort::natural_compare(&a.name, &b.name)
                } else {
                    a.name.cmp(&b.name)
                }
            }
            "apparent-size" => a.asize.cmp(&b.asize),
            "itemcount" => a.stats.item_count.cmp(&b.stats.item_count),
            "mtime" => {
                let a_time = a.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
                let b_time = b.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
                a_time.cmp(&b_time)
            }
            _ => {
                // Default: disk-usage
                a.dsize.cmp(&b.dsize)
            }
        };

        if is_desc {
            ord.reverse()
        } else {
            ord
        }
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
