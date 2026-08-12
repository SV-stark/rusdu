use ratatui::Terminal;
use ratatui::backend::TestBackend;
use rusdu::cli::Args;
use rusdu::tree::{EntryFlags, TreeArena, TreeNode};
use rusdu::ui::browser;
use rusdu::ui::{AppState, Dialog, GraphMode, HelpPage, SharedColumnMode};

fn create_test_state() -> AppState {
    let root = TreeNode::new_dir("/test".to_string(), 1, 10, EntryFlags::empty(), None);
    let mut arena = TreeArena::new(root);

    let docs = TreeNode::new_dir("docs".to_string(), 1, 20, EntryFlags::empty(), None);
    let docs_id = arena.add_child(arena.root, docs);

    let pdf = TreeNode::new_file(
        "report.pdf".to_string(),
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        1,
        30,
        1,
        EntryFlags::empty(),
        None,
    );
    arena.add_child(docs_id, pdf);

    let txt = TreeNode::new_file(
        "notes.txt".to_string(),
        4 * 1024,
        4 * 1024,
        1,
        40,
        1,
        EntryFlags::empty(),
        None,
    );
    arena.add_child(docs_id, txt);

    let args = Args::default();
    let root_id = arena.root;

    let mut state = AppState {
        arena,
        current_dir: root_id,
        selected_idx: 0,
        scroll_offset: 0,
        history: Vec::new(),
        args,
        apparent_size: false,
        si: false,
        show_itemcount: true,
        show_mtime: false,
        show_hidden: true,
        group_dirs_first: true,
        graph_mode: GraphMode::Both,
        shared_column_mode: SharedColumnMode::Off,
        active_dialog: Dialog::None,
        show_icons: false,
        refreshing_rx: None,
        visible_children: Vec::new(),
        custom_actions: std::collections::HashMap::new(),
        filter_query: None,
        show_preview: false,
        fs_modified: false,
        watcher: None,
        watcher_rx: None,
    };
    state.update_visible_children();
    state
}

#[test]
fn test_tui_main_browser_render() {
    let mut state = create_test_state();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| browser::draw(f, &mut state))
        .expect("Failed to draw TUI main browser");

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Verify main components exist in rendered buffer
    assert!(content.contains("rusdu"), "Header should display app name");
    assert!(content.contains("docs"), "Directory list should show 'docs'");
}

#[test]
fn test_tui_dialog_confirm_quit() {
    let mut state = create_test_state();
    state.active_dialog = Dialog::ConfirmQuit;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| browser::draw(f, &mut state))
        .expect("Failed to draw confirm quit dialog");

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    assert!(content.contains("Quit"), "Dialog should display quit header or prompt");
    assert!(content.contains("Really quit"), "Dialog text should ask confirmation");

}

#[test]
fn test_tui_dialog_help_render() {
    let mut state = create_test_state();
    state.active_dialog = Dialog::Help(HelpPage::Keys);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| browser::draw(f, &mut state))
        .expect("Failed to draw help dialog");

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    assert!(content.contains("Help"), "Dialog should contain 'Help'");
}

#[test]
fn test_tui_filter_dialog_render() {
    let mut state = create_test_state();
    state.active_dialog = Dialog::FilterInput("pdf".to_string());

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| browser::draw(f, &mut state))
        .expect("Failed to draw filter input dialog");

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    assert!(content.contains("pdf"), "Filter dialog should display input query 'pdf'");
}

#[test]
fn test_tui_compact_viewport_resilience() {
    let mut state = create_test_state();
    // Test small terminal viewport (30x10) to verify no overflow / index panic
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    let res = terminal.draw(|f| browser::draw(f, &mut state));
    assert!(res.is_ok(), "TUI rendering must be resilient to small viewport sizes");
}
