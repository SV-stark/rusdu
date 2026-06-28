use crate::tree::{EntryFlags, NodeId};
use crate::ui::theme::get_theme;
use crate::ui::{
    get_node_path, get_visible_children, AppState, Dialog, GraphMode, HelpPage, SharedColumnMode,
};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let theme = get_theme(&state.args.color);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(3),    // Browser body
            Constraint::Length(1), // Footer
        ])
        .split(f.size());

    // 1. Draw Header
    let current_path = get_node_path(&state.arena, state.current_dir);
    let header_text = format!(
        " rusdu {} ~ {} [Use arrows to navigate, ? for help]",
        env!("CARGO_PKG_VERSION"),
        current_path.to_string_lossy()
    );
    f.render_widget(Paragraph::new(header_text).style(theme.header), chunks[0]);

    // 2. Draw Browser Body (File List)
    let visible_children = &state.visible_children;
    let mut list_items = Vec::new();

    // Find the largest size to scale the graph column
    let max_size = visible_children
        .iter()
        .map(|&id| {
            let child = state.arena.get(id);
            if child.is_dir() {
                if state.apparent_size {
                    child.stats.total_asize
                } else {
                    child.stats.total_dsize
                }
            } else {
                if state.apparent_size {
                    child.asize
                } else {
                    child.dsize
                }
            }
        })
        .max()
        .unwrap_or(0);

    // Get parent cumulative size to compute percentage
    let parent_cumulative_size = {
        let parent = state.arena.get(state.current_dir);
        if state.apparent_size {
            parent.stats.total_asize
        } else {
            parent.stats.total_dsize
        }
    }
    .max(1);

    let height = chunks[1].height as usize;
    if height > 0 {
        if state.selected_idx < state.scroll_offset {
            state.scroll_offset = state.selected_idx;
        } else if state.selected_idx >= state.scroll_offset + height {
            state.scroll_offset = state.selected_idx - height + 1;
        }
    }

    if visible_children.len() <= height {
        state.scroll_offset = 0;
    } else {
        state.scroll_offset = state.scroll_offset.min(visible_children.len() - height);
    }

    let end_idx = (state.scroll_offset + height).min(visible_children.len());

    for idx in state.scroll_offset..end_idx {
        let child_id = visible_children[idx];
        let child = state.arena.get(child_id);

        // Build prefix flag
        let flag = if child.flags.contains(EntryFlags::READ_ERROR) {
            "!"
        } else if child.flags.contains(EntryFlags::SUB_ERROR) {
            "."
        } else if child.flags.contains(EntryFlags::EXCLUDED) {
            "<"
        } else if child.flags.contains(EntryFlags::OTHER_FS) {
            ">"
        } else if child.flags.contains(EntryFlags::KERNFS) {
            "F"
        } else if child.flags.contains(EntryFlags::NOT_REG) {
            "@"
        } else if child.flags.contains(EntryFlags::HARD_LINK) {
            "H"
        } else if child.flags.contains(EntryFlags::EMPTY_DIR) {
            "e"
        } else {
            " "
        };

        // Format apparent/disk size
        let size_val = if child.is_dir() {
            if state.apparent_size {
                child.stats.total_asize
            } else {
                child.stats.total_dsize
            }
        } else {
            if state.apparent_size {
                child.asize
            } else {
                child.dsize
            }
        };
        let size_str = crate::format::format_size(size_val, state.si);

        // Optional Column: Shared column
        let mut shared_str = String::new();
        if state.shared_column_mode != SharedColumnMode::Off {
            let shared_val = child.stats.shared_size;
            shared_str = format!(" {}", crate::format::format_size(shared_val, state.si));
        }

        // Optional Column: Item count
        let mut itemcount_str = String::new();
        if state.show_itemcount {
            let items = if child.is_dir() {
                child.stats.item_count
            } else {
                1
            };
            itemcount_str = format!(" {:>5}", items);
        }

        // Optional Column: mtime
        let mut mtime_str = String::new();
        if state.show_mtime {
            let mtime_val = child.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
            if mtime_val > 0 {
                let dt = chrono::DateTime::from_timestamp(mtime_val, 0);
                mtime_str = format!(
                    " {}",
                    dt.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                );
            } else {
                mtime_str = " ".repeat(20);
            }
        }

        // Percentage & Graph calculations
        let pct = (size_val as f64 / parent_cumulative_size as f64) * 100.0;
        let pct_str = format!(" {:>5.1}%", pct);

        let mut graph_str = String::new();
        if state.graph_mode == GraphMode::Both || state.graph_mode == GraphMode::Graph {
            let max_graph_width = 10;
            let bar_len = if max_size > 0 {
                ((size_val as f64 / max_size as f64) * max_graph_width as f64).round() as usize
            } else {
                0
            };

            let char_style = match state.args.graph_style.as_str() {
                "half-block" => "▌",
                "eighth-block" => "█", // Simple representation
                _ => "#",
            };

            graph_str = format!(" [{:<10}]", char_style.repeat(bar_len));
        }

        // Format name
        let name_suffix = if child.is_dir() { "/" } else { "" };
        let display_name = if state.show_icons {
            let icon = if child.is_dir() {
                " "
            } else if child.flags.contains(EntryFlags::READ_ERROR) {
                " "
            } else if child.flags.contains(EntryFlags::NOT_REG) {
                "󱘖 "
            } else {
                " "
            };
            format!("{}{}{}", icon, child.name, name_suffix)
        } else {
            format!("{}{}", child.name, name_suffix)
        };

        // Combine fields
        let row_style = if idx == state.selected_idx {
            theme.selected
        } else if child.is_dir() {
            theme.dir
        } else {
            theme.file
        };

        // Construct raw formatted row line
        let line = format!(
            "{}{:<10}{}{}{}{}{} {}",
            flag,
            size_str,
            shared_str,
            itemcount_str,
            mtime_str,
            if state.graph_mode == GraphMode::Both || state.graph_mode == GraphMode::Percent {
                &pct_str
            } else {
                ""
            },
            graph_str,
            display_name
        );

        list_items.push(ListItem::new(line).style(row_style));
    }

    let list = List::new(list_items).block(Block::default().borders(Borders::NONE));
    f.render_widget(list, chunks[1]);

    // 3. Draw Footer
    let current_dir_node = state.arena.get(state.current_dir);
    let total_disk = crate::format::format_size(current_dir_node.stats.total_dsize, state.si);
    let total_app = crate::format::format_size(current_dir_node.stats.total_asize, state.si);
    let total_items = current_dir_node.stats.item_count;

    let footer_text = format!(
        " Total disk usage: {}   Apparent size: {}   Items: {}",
        total_disk, total_app, total_items
    );
    f.render_widget(Paragraph::new(footer_text).style(theme.footer), chunks[2]);

    // 4. Draw Dialog Overlays
    if state.refreshing_rx.is_some() {
        draw_refreshing_dialog(f, &theme);
    } else {
        match &state.active_dialog {
            Dialog::Help(page) => draw_help_dialog(f, *page, &theme),
            Dialog::Info(node_id) => draw_info_dialog(f, state, *node_id, &theme),
            Dialog::ConfirmDelete(node_id) => draw_confirm_delete(f, state, *node_id, &theme),
            Dialog::ConfirmQuit => draw_confirm_quit(f, &theme),
            Dialog::None => {}
        }
    }
}

fn draw_help_dialog(f: &mut Frame, page: HelpPage, theme: &crate::ui::theme::Theme) {
    let size = f.size();
    let area = centered_rect(60, 60, size);

    let mut text = Vec::new();
    match page {
        HelpPage::Keys => {
            text.push(Line::from(vec![
                Span::styled("Keys  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("   Format     About\n\n"),
            ]));
            text.push(Line::from("  Up, k       Move cursor up"));
            text.push(Line::from("  Down, j     Move cursor down"));
            text.push(Line::from("  Right, Enter Open directory"));
            text.push(Line::from("  Left, Backspace Parent directory"));
            text.push(Line::from("  n           Sort by name (desc/asc)"));
            text.push(Line::from("  s           Sort by size (desc/asc)"));
            text.push(Line::from("  C           Sort by items (desc/asc)"));
            text.push(Line::from("  M           Sort by mtime (desc/asc)"));
            text.push(Line::from("  d           Delete selected item"));
            text.push(Line::from("  b           Spawn shell in current dir"));
            text.push(Line::from("  r           Refresh directory"));
            text.push(Line::from("  a           Toggle apparent/disk size"));
            text.push(Line::from("  g           Toggle percentage/graph"));
            text.push(Line::from("  q           Quit rusdu"));
        }
        HelpPage::Format => {
            text.push(Line::from(vec![
                Span::raw("  Keys     "),
                Span::styled("Format", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("    About\n\n"),
            ]));
            text.push(Line::from("  !  Error occurred reading directory"));
            text.push(Line::from("  .  Error occurred reading subdirectory"));
            text.push(Line::from("  <  Excluded from statistics"));
            text.push(Line::from("  >  On another filesystem"));
            text.push(Line::from("  @  Not a regular file (symlink, socket...)"));
            text.push(Line::from("  H  Hard link (already counted)"));
            text.push(Line::from("  e  Empty directory"));
        }
        HelpPage::About => {
            text.push(Line::from(vec![
                Span::raw("  Keys     Format     "),
                Span::styled("About", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("\n\n"),
            ]));
            text.push(Line::from("  rusdu — Rust rewrite of ncdu"));
            text.push(Line::from(format!(
                "  Version: {}",
                env!("CARGO_PKG_VERSION")
            )));
            text.push(Line::from("  Designed to be 100% compatible with ncdu 2.x"));
            text.push(Line::from("  Powered by ratatui and crossterm."));
        }
    }

    let block = Block::default()
        .title(" Help / About ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_info_dialog(
    f: &mut Frame,
    state: &AppState,
    node_id: NodeId,
    theme: &crate::ui::theme::Theme,
) {
    let size = f.size();
    let area = centered_rect(70, 50, size);

    let node = state.arena.get(node_id);
    let full_path = get_node_path(&state.arena, node_id);

    let mut text = Vec::new();
    text.push(Line::from(vec![
        Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(node.name.to_string()),
    ]));
    text.push(Line::from(vec![
        Span::styled("Path: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(full_path.to_string_lossy().into_owned()),
    ]));
    text.push(Line::from(vec![
        Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(if node.is_dir() {
            "Directory"
        } else {
            "Regular File"
        }),
    ]));
    text.push(Line::from(vec![
        Span::styled(
            "Apparent size: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::format::format_size(node.asize, state.si)),
    ]));
    text.push(Line::from(vec![
        Span::styled(
            "Disk usage:    ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(crate::format::format_size(node.dsize, state.si)),
    ]));

    if node.is_dir() {
        text.push(Line::from(vec![
            Span::styled(
                "Sub-items:     ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(node.stats.item_count.to_string()),
        ]));
    }

    if let Some(ref ext) = node.extended {
        text.push(Line::from(vec![
            Span::styled(
                "Last modified: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{}",
                chrono::DateTime::from_timestamp(ext.mtime, 0)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_else(|| "Unknown".to_string())
            )),
        ]));
        text.push(Line::from(vec![
            Span::styled(
                "UID / GID:     ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} / {}", ext.uid, ext.gid)),
        ]));
        text.push(Line::from(vec![
            Span::styled(
                "Permissions:   ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:o}", ext.mode)),
        ]));
    }

    let block = Block::default()
        .title(" Item Info ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_confirm_delete(
    f: &mut Frame,
    state: &AppState,
    node_id: NodeId,
    theme: &crate::ui::theme::Theme,
) {
    let size = f.size();
    let area = centered_rect(50, 20, size);

    let node = state.arena.get(node_id);
    let mut text = Vec::new();
    text.push(Line::from("Are you sure you want to delete:"));
    text.push(Line::from(vec![Span::styled(
        &*node.name,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]));
    text.push(Line::from("\n(Press 'y' to confirm, 'n' to cancel)"));

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_confirm_quit(f: &mut Frame, theme: &crate::ui::theme::Theme) {
    let size = f.size();
    let area = centered_rect(40, 20, size);

    let mut text = Vec::new();
    text.push(Line::from("Really quit rusdu?"));
    text.push(Line::from("\n(Press 'y' to confirm, 'n' to cancel)"));

    let block = Block::default()
        .title(" Confirm Quit ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_refreshing_dialog(f: &mut Frame, theme: &crate::ui::theme::Theme) {
    let size = f.size();
    let area = centered_rect(40, 15, size);

    let mut text = Vec::new();
    text.push(Line::from(""));
    text.push(Line::from("  Refreshing directory..."));
    text.push(Line::from("  Please wait."));

    let block = Block::default()
        .title(" Refreshing ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}
