use crate::tree::{EntryFlags, NodeId};
use crate::ui::theme::get_theme;
use crate::ui::{
    AppState, Dialog, DriveInfo, GraphMode, HelpPage, SharedColumnMode, get_node_path,
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
        .split(f.area());

    // 1. Draw Header
    let current_path = get_node_path(&state.arena, state.current_dir);
    let mut header_text = format!(
        " rusdu {} ~ {}",
        env!("CARGO_PKG_VERSION"),
        current_path.to_string_lossy()
    );
    if let Some(ref q) = state.filter_query {
        header_text.push_str(&format!(" [Filter: {}]", q));
    }
    if state.fs_modified {
        header_text.push_str(" [Disk Changed - Press 'r' to refresh]");
    }
    header_text.push_str(" [Use arrows to navigate, ? for help]");

    f.render_widget(Paragraph::new(header_text).style(theme.header), chunks[0]);

    // 2. Split Browser Body horizontally if preview is enabled
    let (list_area, preview_area) = if state.show_preview {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);
        (split[0], Some(split[1]))
    } else {
        (chunks[1], None)
    };

    // Draw Browser Body (File List)
    let visible_children = &state.visible_children;
    let mut list_items = Vec::new();

    // Find the largest size to scale the graph column
    let max_size = visible_children
        .iter()
        .map(|&id| {
            let child = state.arena.get(id);
            let stats = child.get_stats();
            if child.is_dir() {
                if state.apparent_size {
                    stats.total_asize
                } else {
                    stats.total_dsize
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
        let stats = parent.get_stats();
        if state.apparent_size {
            stats.total_asize
        } else {
            stats.total_dsize
        }
    }
    .max(1);

    let height = list_area.height as usize;
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

    for (i, &child_id) in visible_children[state.scroll_offset..end_idx]
        .iter()
        .enumerate()
    {
        let idx = state.scroll_offset + i;
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
        let stats = child.get_stats();
        let size_val = if child.is_dir() {
            if state.apparent_size {
                stats.total_asize
            } else {
                stats.total_dsize
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
            let shared_val = stats.shared_size;
            let formatted = crate::format::format_size(shared_val, state.si);
            shared_str = format!(" {:>9}", formatted);
        }

        // Optional Column: Item count
        let mut itemcount_str = String::new();
        if state.show_itemcount {
            let items = if child.is_dir() { stats.item_count } else { 1 };
            itemcount_str = format!(" {:>5}", items);
        }

        // Optional Column: mtime
        let mut mtime_str = String::new();
        if state.show_mtime {
            let mtime_val = child.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
            if mtime_val > 0 {
                let formatted =
                    if let Ok(odt) = time::OffsetDateTime::from_unix_timestamp(mtime_val) {
                        let format = time::macros::format_description!(
                            "[year]-[month]-[day] [hour]:[minute]:[second]"
                        );
                        odt.format(&format)
                            .unwrap_or_else(|_| "Unknown".to_string())
                    } else {
                        "Unknown".to_string()
                    };
                mtime_str = format!(" {}", formatted);
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
            "{:2}{:>10}{}{}{}{}{} {}",
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
    f.render_widget(list, list_area);

    // Render Preview Pane if enabled
    if let Some(area) = preview_area {
        if !visible_children.is_empty() && state.selected_idx < visible_children.len() {
            let selected_id = visible_children[state.selected_idx];
            draw_sidebar_preview(f, state, selected_id, area, &theme);
        } else {
            let block = Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(theme.border)
                .bg(Color::Black);
            f.render_widget(block, area);
        }
    }

    // 3. Draw Footer
    let current_dir_node = state.arena.get(state.current_dir);
    let current_stats = current_dir_node.get_stats();
    let total_disk = crate::format::format_size(current_stats.total_dsize, state.si);
    let total_app = crate::format::format_size(current_stats.total_asize, state.si);
    let total_items = current_stats.item_count;

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
            Dialog::FilterInput(query) => draw_live_filter(f, query, &theme),
            Dialog::FuzzySearch {
                query,
                results,
                selected_idx,
            } => draw_fuzzy_search(f, query, results, *selected_idx, &theme),
            Dialog::DriveSelector {
                drives,
                selected_idx,
            } => draw_drive_selector(f, state, drives, *selected_idx, &theme),
            Dialog::ExtensionAnalytics {
                stats,
                selected_idx,
                scroll_offset,
            } => draw_extension_analytics(f, state, stats, *selected_idx, *scroll_offset, &theme),
            Dialog::None => {}
        }
    }
}

fn draw_help_dialog(f: &mut Frame, page: HelpPage, theme: &crate::ui::theme::Theme) {
    let size = f.area();
    let area = centered_rect(75, 90, size);

    let mut text = Vec::new();
    match page {
        HelpPage::Keys => {
            text.push(Line::from(vec![
                Span::styled("Keys  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("   Format     About\n\n"),
            ]));
            text.push(Line::from("  k, Up          Move cursor up"));
            text.push(Line::from("  j, Down        Move cursor down"));
            text.push(Line::from("  l, Enter, →    Open selected directory"));
            text.push(Line::from("  h, Backsp, ←   Go to parent directory"));
            text.push(Line::from("  PageUp, PageDn Scroll up/down 10 items"));
            text.push(Line::from("  Home, End      Jump to first/last item"));
            text.push(Line::from("  n              Sort by name (desc/asc)"));
            text.push(Line::from("  s              Sort by size (desc/asc)"));
            text.push(Line::from("  C              Sort by item count (desc/asc)"));
            text.push(Line::from(
                "  M              Sort by mtime (desc/asc, req. -e)",
            ));
            text.push(Line::from(
                "  t              Toggle group directories first",
            ));
            text.push(Line::from(
                "  a              Toggle apparent ↔ disk usage size",
            ));
            text.push(Line::from(
                "  g              Cycle graph: both → pct → graph → off",
            ));
            text.push(Line::from(
                "  u              Cycle shared column: off → shared → unique",
            ));
            text.push(Line::from(
                "  c              Toggle item count column visibility",
            ));
            text.push(Line::from(
                "  m              Toggle mtime column visibility (req. -e)",
            ));
            text.push(Line::from(
                "  e              Toggle hidden/excluded files visibility",
            ));
            text.push(Line::from("  d              Delete selected item"));
            text.push(Line::from(
                "  b              Spawn shell in current directory",
            ));
            text.push(Line::from(
                "  r              Refresh/rescan current directory",
            ));
            text.push(Line::from(
                "  i              Show detailed info about selected item",
            ));
            text.push(Line::from("  /              Open live filter query input"));
            text.push(Line::from("  f, Ctrl+F      Open global fuzzy search"));
            text.push(Line::from(
                "  Tab, p         Toggle sidebar file preview panel",
            ));
            text.push(Line::from("  V              Open disk/drive selector"));
            text.push(Line::from("  E              Show file extension analytics"));
            text.push(Line::from(
                "  c, o, v        Custom actions (Copy path, Open folder, Open editor)",
            ));
            text.push(Line::from("  ?, F1          Open help screen"));
            text.push(Line::from("  q              Quit (or close dialog)"));
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
    let size = f.area();
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
            Span::raw(node.get_stats().item_count.to_string()),
        ]));
    }

    if let Some(ref ext) = node.extended {
        text.push(Line::from(vec![
            Span::styled(
                "Last modified: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                if let Ok(odt) = time::OffsetDateTime::from_unix_timestamp(ext.mtime) {
                    odt.format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_else(|_| "Unknown".to_string())
                } else {
                    "Unknown".to_string()
                },
            ),
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
    let size = f.area();
    let area = centered_rect(50, 20, size);

    let node = state.arena.get(node_id);
    let text = vec![
        Line::from("Are you sure you want to delete:"),
        Line::from(vec![Span::styled(
            &*node.name,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from("\n(Press 'y' to confirm, 'n' to cancel)"),
    ];

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
    let size = f.area();
    let area = centered_rect(40, 20, size);

    let text = vec![
        Line::from("Really quit rusdu?"),
        Line::from("\n(Press 'y' to confirm, 'n' to cancel)"),
    ];

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
    let size = f.area();
    let area = centered_rect(40, 15, size);

    let text = vec![
        Line::from(""),
        Line::from("  Refreshing directory..."),
        Line::from("  Please wait."),
    ];

    let block = Block::default()
        .title(" Refreshing ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_sidebar_preview(
    f: &mut Frame,
    state: &AppState,
    node_id: NodeId,
    area: Rect,
    theme: &crate::ui::theme::Theme,
) {
    let node = state.arena.get(node_id);
    let full_path = get_node_path(&state.arena, node_id);

    let mut text = Vec::new();
    text.push(Line::from(vec![
        Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(node.name.to_string()),
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
        Span::styled("Size: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(crate::format::format_size(node.asize, state.si)),
    ]));
    text.push(Line::from(vec![
        Span::styled("Disk: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(crate::format::format_size(node.dsize, state.si)),
    ]));

    if node.is_dir() {
        let stats = node.get_stats();
        text.push(Line::from(vec![
            Span::styled("Items: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(stats.item_count.to_string()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Files: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(stats.file_count.to_string()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Dirs:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(stats.dir_count.to_string()),
        ]));
    }

    if let Some(ref ext) = node.extended {
        text.push(Line::from(vec![
            Span::styled("Owner: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{} / {}", ext.uid, ext.gid)),
        ]));
        text.push(Line::from(vec![
            Span::styled("Perms: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:o}", ext.mode)),
        ]));
        text.push(Line::from(vec![
            Span::styled("MTime: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(
                if let Ok(odt) = time::OffsetDateTime::from_unix_timestamp(ext.mtime) {
                    let format = time::macros::format_description!(
                        "[year]-[month]-[day] [hour]:[minute]:[second]"
                    );
                    odt.format(&format)
                        .unwrap_or_else(|_| "Unknown".to_string())
                } else {
                    "Unknown".to_string()
                },
            ),
        ]));
    }

    // Add visual line separator
    text.push(Line::from("-".repeat(area.width as usize)));

    // File contents preview
    if !node.is_dir() {
        text.push(Line::from(Span::styled(
            "--- File Preview ---",
            Style::default().fg(Color::Yellow),
        )));
        let preview = read_file_preview(&full_path);
        for line in preview.lines() {
            text.push(Line::from(line.to_string()));
        }
    }

    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn read_file_preview(path: &std::path::Path) -> String {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for line in reader.lines().take(15) {
            if let Ok(l) = line {
                if l.chars().count() > 40 {
                    let truncated: String = l.chars().take(40).collect();
                    lines.push(format!("{}...", truncated));
                } else {
                    lines.push(l);
                }
            } else {
                break;
            }
        }
        if lines.is_empty() {
            return "[Empty file or binary data]".to_string();
        }
        return lines.join("\n");
    }
    "[Preview not available]".to_string()
}

fn draw_fuzzy_search(
    f: &mut Frame,
    query: &str,
    results: &[(NodeId, String)],
    selected_idx: usize,
    theme: &crate::ui::theme::Theme,
) {
    let size = f.area();
    let area = centered_rect(80, 80, size);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input query block
            Constraint::Min(3),    // Results list
        ])
        .split(area);

    let input_block = Block::default()
        .title(" Fuzzy Search Query ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let input_paragraph = Paragraph::new(format!("> {}", query)).block(input_block);
    f.render_widget(Clear, popup_layout[0]);
    f.render_widget(input_paragraph, popup_layout[0]);

    let results_block = Block::default()
        .title(" Matching Items (Max 50) ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let mut list_items = Vec::new();
    for (i, res) in results.iter().enumerate() {
        let style = if i == selected_idx {
            theme.selected
        } else {
            theme.file
        };
        list_items.push(ListItem::new(res.1.clone()).style(style));
    }

    let list = List::new(list_items).block(results_block);
    f.render_widget(Clear, popup_layout[1]);
    f.render_widget(list, popup_layout[1]);
}

fn draw_live_filter(f: &mut Frame, query: &str, theme: &crate::ui::theme::Theme) {
    let size = f.area();
    let area = centered_rect(50, 20, size);

    let text = vec![
        Line::from("Type filter query (case-insensitive substring):"),
        Line::from(Span::styled(
            format!("  / {}", query),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("(Press Enter to apply, Esc to clear & cancel)"),
    ];

    let block = Block::default()
        .title(" Live Filter ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_drive_selector(
    f: &mut Frame,
    state: &AppState,
    drives: &[DriveInfo],
    selected_idx: usize,
    theme: &crate::ui::theme::Theme,
) {
    let size = f.area();
    let area = centered_rect(65, 55, size);

    let mut list_items = Vec::new();
    for (i, drive) in drives.iter().enumerate() {
        let line = format!(
            "  {:<15} [{}] (Free: {} / Total: {})",
            drive.name,
            drive.mount_point.display(),
            crate::format::format_size(drive.available_space as i64, state.si),
            crate::format::format_size(drive.total_space as i64, state.si)
        );
        let style = if i == selected_idx {
            theme.selected
        } else {
            theme.dir
        };
        list_items.push(ListItem::new(line).style(style));
    }

    let block = Block::default()
        .title(" Select Disk/Drive to Scan ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let list = List::new(list_items).block(block);
    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn draw_extension_analytics(
    f: &mut Frame,
    state: &AppState,
    stats: &[(String, u64)],
    selected_idx: usize,
    scroll_offset: usize,
    theme: &crate::ui::theme::Theme,
) {
    let size = f.area();
    let area = centered_rect(70, 60, size);

    let mut list_items = Vec::new();
    let total_ext_size: u64 = stats.iter().map(|s| s.1).sum();

    let height = (area.height as usize).saturating_sub(4); // subtract borders (2) + title (1) + padding (1)
    let height = height.max(1);
    let end_idx = (scroll_offset + height).min(stats.len());

    for (i, (ext, size_val)) in stats[scroll_offset..end_idx].iter().enumerate() {
        let idx = scroll_offset + i;
        let pct = if total_ext_size > 0 {
            (*size_val as f64 / total_ext_size as f64) * 100.0
        } else {
            0.0
        };
        let line = format!(
            "  {:<18} {:>12} ({:>5.1}%)",
            ext,
            crate::format::format_size(*size_val as i64, state.si),
            pct
        );
        let style = if idx == selected_idx {
            theme.selected
        } else {
            theme.file
        };
        list_items.push(ListItem::new(line).style(style));
    }

    let block = Block::default()
        .title(" Extension Space Distribution ")
        .borders(Borders::ALL)
        .border_style(theme.border)
        .bg(Color::Black);

    let list = List::new(list_items).block(block);
    f.render_widget(Clear, area);
    f.render_widget(list, area);
}
