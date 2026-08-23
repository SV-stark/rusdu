use rusdu::export::{bin_read, bin_write, json_read, json_write};
use rusdu::format::format_size;
use rusdu::tree::{EntryFlags, ExtendedInfo, TreeArena, TreeNode};
use std::path::Path;

#[test]
fn test_utf8_progress_truncation() {
    // Tests that path truncation does not panic on multi-byte unicode code points
    let non_ascii_path =
        Path::new("/var/log/用户目录/フォルダ/🎉_deep_sub_path/sample_document_测试.dat");
    let mut stats = rusdu::scan::ScanStats::default();

    // Line mode
    rusdu::scan::update_progress(non_ascii_path, &mut stats, rusdu::scan::ProgressMode::Line);
    assert_eq!(stats.items_scanned, 0);
}

#[test]
fn test_tree_arena_replace_subtree() {
    // 1. Create main arena with root -> sub1, sub2
    let root = TreeNode::new_dir("root".to_string(), 1, 10, EntryFlags::empty(), None);
    let mut arena = TreeArena::new(root);

    let sub1 = TreeNode::new_dir("sub1".to_string(), 1, 20, EntryFlags::empty(), None);
    let sub1_id = arena.add_child(arena.root, sub1);

    let file1 = TreeNode::new_file(
        "file1.txt".to_string(),
        1000,
        4096,
        1,
        101,
        1,
        EntryFlags::empty(),
        None,
    );
    arena.add_child(sub1_id, file1);

    rusdu::tree::stats::recalculate_stats(&mut arena);
    let old_root_size = arena.get(arena.root).get_stats().total_asize;
    assert_eq!(old_root_size, 1000);

    // 2. Rescanned sub1 with new contents
    let rescan_sub1 = TreeNode::new_dir("sub1".to_string(), 1, 20, EntryFlags::empty(), None);
    let mut new_arena = TreeArena::new(rescan_sub1);

    let new_file1 = TreeNode::new_file(
        "new_file1.txt".to_string(),
        2500,
        4096,
        1,
        201,
        1,
        EntryFlags::empty(),
        None,
    );
    new_arena.add_child(new_arena.root, new_file1);

    let new_file2 = TreeNode::new_file(
        "new_file2.txt".to_string(),
        1500,
        4096,
        1,
        202,
        1,
        EntryFlags::empty(),
        None,
    );
    new_arena.add_child(new_arena.root, new_file2);

    // 3. Graft new_arena into arena at sub1_id
    arena.replace_subtree(sub1_id, &new_arena);
    rusdu::tree::stats::recalculate_stats(&mut arena);

    // Verify child nodes and parent references in grafted tree
    assert_eq!(arena.get(sub1_id).children.len(), 2);
    let child_0 = arena.get(sub1_id).children[0];
    let child_1 = arena.get(sub1_id).children[1];
    assert_eq!(arena.get(child_0).name.as_ref(), "new_file1.txt");
    assert_eq!(arena.get(child_1).name.as_ref(), "new_file2.txt");
    assert_eq!(arena.get(child_0).parent, Some(sub1_id));
    assert_eq!(arena.get(child_1).parent, Some(sub1_id));

    // Verify stats recalculation rolled up to root
    let new_root_size = arena.get(arena.root).get_stats().total_asize;
    assert_eq!(new_root_size, 4000);
}

#[test]
fn test_hard_link_dedup_zero_inode_handling() {
    // When ino == 0 (e.g. from JSON imports where ino is missing), hard link dedup must not collapse them
    let root = TreeNode::new_dir("root".to_string(), 1, 10, EntryFlags::empty(), None);
    let mut arena = TreeArena::new(root);

    let file1 = TreeNode::new_file(
        "hl1.dat".to_string(),
        5000,
        5000,
        0,
        0, // ino 0
        2,
        EntryFlags::HARD_LINK,
        None,
    );
    arena.add_child(arena.root, file1);

    let file2 = TreeNode::new_file(
        "hl2.dat".to_string(),
        7000,
        7000,
        0,
        0, // ino 0
        2,
        EntryFlags::HARD_LINK,
        None,
    );
    arena.add_child(arena.root, file2);

    rusdu::tree::stats::recalculate_stats(&mut arena);
    let root_stats = arena.get(arena.root).get_stats();
    // Both files should be counted since ino == 0
    assert_eq!(root_stats.total_asize, 12000);

    // Now test with actual inode dedup (ino > 0)
    let root2 = TreeNode::new_dir("root2".to_string(), 1, 10, EntryFlags::empty(), None);
    let mut arena2 = TreeArena::new(root2);

    let file3 = TreeNode::new_file(
        "hl3.dat".to_string(),
        5000,
        5000,
        1,
        999, // ino 999
        2,
        EntryFlags::HARD_LINK,
        None,
    );
    arena2.add_child(arena2.root, file3);

    let file4 = TreeNode::new_file(
        "hl4.dat".to_string(),
        5000,
        5000,
        1,
        999, // same ino 999
        2,
        EntryFlags::HARD_LINK,
        None,
    );
    arena2.add_child(arena2.root, file4);

    rusdu::tree::stats::recalculate_stats(&mut arena2);
    let root2_stats = arena2.get(arena2.root).get_stats();
    // Deduplicated to 5000 bytes
    assert_eq!(root2_stats.total_asize, 5000);
}

#[test]
fn test_binary_export_import_multi_sibling_and_sub_error() {
    let mut root = TreeNode::new_dir("root".to_string(), 1, 10, EntryFlags::empty(), None);
    root.extended = Some(ExtendedInfo {
        mtime: 1700000000,
        uid: 1000,
        gid: 1000,
        mode: 0o755,
    });
    let mut arena = TreeArena::new(root);

    let sub = TreeNode::new_dir("sub".to_string(), 1, 20, EntryFlags::empty(), None);
    let sub_id = arena.add_child(arena.root, sub);

    // Add failing node to trigger SUB_ERROR on parent
    let err_file = TreeNode::new_file(
        "err.dat".to_string(),
        0,
        0,
        1,
        30,
        1,
        EntryFlags::READ_ERROR,
        None,
    );
    arena.add_child(sub_id, err_file);

    // Add multiple siblings in root
    for i in 1..=50 {
        let file = TreeNode::new_file(
            format!("file_{}.txt", i),
            (i * 100) as i64,
            (i * 100) as i64,
            1,
            (100 + i) as u64,
            1,
            EntryFlags::empty(),
            None,
        );
        arena.add_child(arena.root, file);
    }

    rusdu::tree::stats::recalculate_stats(&mut arena);

    // Verify SUB_ERROR propagation
    assert!(arena.get(sub_id).flags.contains(EntryFlags::SUB_ERROR));
    assert!(arena.get(arena.root).flags.contains(EntryFlags::SUB_ERROR));

    // Test binary export roundtrip
    let bin_bytes = bin_write::export_bin(&arena, 64, 3).expect("Binary export failed");
    assert!(bin_bytes.starts_with(b"\xbfncduEX1"));

    let imported_arena = bin_read::import_bin(&bin_bytes).expect("Binary import failed");
    assert_eq!(imported_arena.get(imported_arena.root).children.len(), 51);
    let imported_stats = imported_arena.get(imported_arena.root).get_stats();
    let original_stats = arena.get(arena.root).get_stats();
    assert_eq!(imported_stats.total_asize, original_stats.total_asize);
}

#[test]
fn test_json_export_import_streaming() {
    let root = TreeNode::new_dir("root".to_string(), 555, 10, EntryFlags::empty(), None);
    let mut arena = TreeArena::new(root);

    let file = TreeNode::new_file(
        "streamed.log".to_string(),
        123456,
        126976,
        555,
        20,
        1,
        EntryFlags::empty(),
        None,
    );
    arena.add_child(arena.root, file);

    rusdu::tree::stats::recalculate_stats(&mut arena);

    let json_bytes = json_write::export_json(&arena).expect("JSON export failed");
    let json_str = std::str::from_utf8(&json_bytes).unwrap();
    assert!(json_str.starts_with("[1,2,"));
    assert!(json_str.contains("\"dev\":555"));

    let imported_arena = json_read::import_json(&json_bytes).expect("JSON import failed");
    assert_eq!(imported_arena.get(imported_arena.root).children.len(), 1);
    assert_eq!(imported_arena.get(imported_arena.root).dev, 555);
}

#[test]
fn test_format_size_ncdu_spec() {
    assert_eq!(format_size(0, false), "0 B");
    assert_eq!(format_size(512, false), "512 B");
    assert_eq!(format_size(1024, false), "1.0 KiB");
    assert_eq!(format_size(1536, false), "1.5 KiB");
    assert_eq!(format_size(99 * 1024, false), "99.0 KiB");
    assert_eq!(format_size(100 * 1024, false), "100 KiB");
    assert_eq!(format_size(1024 * 1024, false), "1.0 MiB");
}

#[test]
fn test_json_string_excluded_and_fractional_mtime() {
    let json_data = r#"[1,2,{"progname":"ncdu","progver":"1.18","timestamp":1700000000},[
        {"name":"/root","asize":0,"dsize":0,"dev":1,"ino":1},
        {"name":"excluded_file.tmp","asize":500,"dsize":4096,"excluded":"pattern"},
        {"name":"kernfs_entry","asize":0,"dsize":0,"excluded":"kernfs"},
        {"name":"file_with_frac_mtime.txt","asize":100,"dsize":1024,"mtime":1700000123.456}
    ]]"#;

    let arena =
        json_read::import_json(json_data.as_bytes()).expect("Import JSON with spec types failed");
    let root = arena.get(arena.root);
    assert_eq!(root.children.len(), 3);

    let child0 = arena.get(root.children[0]);
    assert!(child0.flags.contains(EntryFlags::EXCLUDED));

    let child1 = arena.get(root.children[1]);
    assert!(child1.flags.contains(EntryFlags::KERNFS));
    assert!(child1.flags.contains(EntryFlags::EXCLUDED));

    let child2 = arena.get(root.children[2]);
    assert_eq!(child2.extended.as_ref().unwrap().mtime, 1700000123);
}

#[test]
fn test_contained_item_counts_and_child_order_preservation() {
    let root = TreeNode::new_dir("root".to_string(), 1, 10, EntryFlags::empty(), None);
    let mut arena = TreeArena::new(root);

    let dir1 = TreeNode::new_dir("dir1".to_string(), 1, 20, EntryFlags::empty(), None);
    let dir1_id = arena.add_child(arena.root, dir1);

    for i in 0..10 {
        let f = TreeNode::new_file(
            format!("file_{:02}.txt", i),
            100,
            100,
            1,
            100 + i,
            1,
            EntryFlags::empty(),
            None,
        );
        arena.add_child(dir1_id, f);
    }

    rusdu::tree::stats::recalculate_stats(&mut arena);

    // Contained items count inside dir1 should be exactly 10 (not 11)
    assert_eq!(arena.get(dir1_id).get_stats().item_count, 10);
    // Contained items count inside root should be 11 (dir1 + 10 files)
    assert_eq!(arena.get(arena.root).get_stats().item_count, 11);

    // Test binary export roundtrip child order
    let bin_bytes = bin_write::export_bin(&arena, 64, 3).expect("Export failed");
    let imported_arena = bin_read::import_bin(&bin_bytes).expect("Import failed");
    let imp_dir1_id = imported_arena.get(imported_arena.root).children[0];
    let imp_children = &imported_arena.get(imp_dir1_id).children;
    assert_eq!(imp_children.len(), 10);

    for (i, &child_id) in imp_children.iter().enumerate() {
        assert_eq!(
            imported_arena.get(child_id).name.as_ref(),
            format!("file_{:02}.txt", i)
        );
    }
}

#[test]
fn test_binary_multi_block_cross_boundary_roundtrip() {
    let mut arena = TreeArena::new(TreeNode::new_dir(
        "multi_block_root".to_string(),
        1,
        1,
        EntryFlags::empty(),
        None,
    ));

    // Create 1000 child files so that with block_size_kb=32 it spans multiple blocks
    for i in 0..1000 {
        let f = TreeNode::new_file(
            format!("large_child_{:04}.dat", i),
            2048,
            4096,
            1,
            2000 + i as u64,
            1,
            EntryFlags::empty(),
            None,
        );
        arena.add_child(arena.root, f);
    }

    rusdu::tree::stats::recalculate_stats(&mut arena);

    // Export with small 32KB block size forcing multiple data blocks
    let bin_bytes = bin_write::export_bin(&arena, 32, 1).expect("Multi-block export failed");
    let imported = bin_read::import_bin(&bin_bytes).expect("Multi-block import failed");

    let root_node = imported.get(imported.root);
    assert_eq!(root_node.children.len(), 1000);

    for (i, &child_id) in root_node.children.iter().enumerate() {
        let child = imported.get(child_id);
        assert_eq!(child.name.as_ref(), format!("large_child_{:04}.dat", i));
        assert_eq!(child.asize, 2048);
        assert_eq!(child.dsize, 4096);
    }
}

#[test]
fn test_cbor_exact_key_numbering_spec() {
    let mut root = TreeNode::new_dir("test_dir".to_string(), 1, 10, EntryFlags::empty(), None);
    root.extended = Some(ExtendedInfo {
        mtime: 1700000000,
        uid: 1000,
        gid: 1000,
        mode: 0o755,
    });
    let mut arena = TreeArena::new(root);

    let file = TreeNode::new_file(
        "hardlink.bin".to_string(),
        500,
        4096,
        1,
        9999,
        2, // nlink > 1 -> type 3
        EntryFlags::HARD_LINK,
        Some(ExtendedInfo {
            mtime: 1700000005,
            uid: 1001,
            gid: 1001,
            mode: 0o644,
        }),
    );
    arena.add_child(arena.root, file);
    rusdu::tree::stats::recalculate_stats(&mut arena);

    let bin_bytes = bin_write::export_bin(&arena, 64, 3).expect("Export failed");
    let imported = bin_read::import_bin(&bin_bytes).expect("Import failed");

    let root_node = imported.get(imported.root);
    assert_eq!(root_node.extended.as_ref().unwrap().uid, 1000);
    assert_eq!(root_node.extended.as_ref().unwrap().gid, 1000);
    assert_eq!(root_node.extended.as_ref().unwrap().mode, 0o755);
    assert_eq!(root_node.extended.as_ref().unwrap().mtime, 1700000000);

    let child = imported.get(root_node.children[0]);
    assert_eq!(child.ino, 9999);
    assert_eq!(child.nlink, 2);
    assert_eq!(child.extended.as_ref().unwrap().uid, 1001);
    assert_eq!(child.extended.as_ref().unwrap().gid, 1001);
    assert_eq!(child.extended.as_ref().unwrap().mode, 0o644);
    assert_eq!(child.extended.as_ref().unwrap().mtime, 1700000005);
}

#[test]
fn test_kernfs_flag_priority_over_excluded() {
    let node = TreeNode::new_dir(
        "sysfs".to_string(),
        1,
        1,
        EntryFlags::KERNFS | EntryFlags::EXCLUDED,
        None,
    );
    assert!(node.flags.contains(EntryFlags::KERNFS));
    assert!(node.flags.contains(EntryFlags::EXCLUDED));

    // Ensure the flag check orders KERNFS before EXCLUDED
    let flag = if node.flags.contains(EntryFlags::READ_ERROR) {
        "!"
    } else if node.flags.contains(EntryFlags::SUB_ERROR) {
        "."
    } else if node.flags.contains(EntryFlags::KERNFS) {
        "^"
    } else if node.flags.contains(EntryFlags::EXCLUDED) {
        "<"
    } else {
        " "
    };

    assert_eq!(flag, "^");
}

#[test]
fn test_golden_ncdu_binary_spec_vector() {
    enum TestCborVal {
        Int(i64),
        Text(&'static str),
        Bool(bool),
    }

    fn encode_test_item(fields: &[(u8, TestCborVal)]) -> Vec<u8> {
        let mut buf = Vec::new();
        let map_len = fields.len();
        if map_len < 24 {
            buf.push(0xa0 | (map_len as u8));
        } else {
            buf.push(0xb8);
            buf.push(map_len as u8);
        }
        for (k, v) in fields {
            if *k < 24 {
                buf.push(*k);
            } else {
                buf.push(0x18);
                buf.push(*k);
            }
            match v {
                TestCborVal::Int(i) => {
                    if *i >= 0 {
                        let u = *i as u64;
                        if u < 24 {
                            buf.push(u as u8);
                        } else if u <= 0xff {
                            buf.push(0x18);
                            buf.push(u as u8);
                        } else if u <= 0xffff {
                            buf.push(0x19);
                            buf.extend_from_slice(&(u as u16).to_be_bytes());
                        } else if u <= 0xffffffff {
                            buf.push(0x1a);
                            buf.extend_from_slice(&(u as u32).to_be_bytes());
                        } else {
                            buf.push(0x1b);
                            buf.extend_from_slice(&u.to_be_bytes());
                        }
                    } else {
                        let n = (-1 - *i) as u64;
                        if n < 24 {
                            buf.push(0x20 | (n as u8));
                        } else if n <= 0xff {
                            buf.push(0x38);
                            buf.push(n as u8);
                        } else {
                            buf.push(0x39);
                            buf.extend_from_slice(&(n as u16).to_be_bytes());
                        }
                    }
                }
                TestCborVal::Text(s) => {
                    let bytes = s.as_bytes();
                    if bytes.len() < 24 {
                        buf.push(0x60 | (bytes.len() as u8));
                    } else {
                        buf.push(0x78);
                        buf.push(bytes.len() as u8);
                    }
                    buf.extend_from_slice(bytes);
                }
                TestCborVal::Bool(b) => buf.push(if *b { 0xf5 } else { 0xf4 }),
            }
        }
        buf
    }

    // Child 1: regular file
    let child1_bytes = encode_test_item(&[
        (0, TestCborVal::Int(0)), // type: 0 (file)
        (1, TestCborVal::Text("first_child.txt")),
        (3, TestCborVal::Int(5000)),
        (4, TestCborVal::Int(8192)),
        (5, TestCborVal::Int(1)),
    ]);

    // Child 2: hardlink candidate (last child)
    let rel_prev = -(child1_bytes.len() as i64);
    let child2_bytes = encode_test_item(&[
        (0, TestCborVal::Int(3)), // type: 3 (hardlink)
        (1, TestCborVal::Text("second_child.bin")),
        (2, TestCborVal::Int(rel_prev)), // relative pointer to child 1
        (3, TestCborVal::Int(5000)),
        (4, TestCborVal::Int(8192)),
        (5, TestCborVal::Int(1)),
        (13, TestCborVal::Int(77777)), // ino
        (14, TestCborVal::Int(2)),     // nlink
        (15, TestCborVal::Int(1001)),  // uid
        (16, TestCborVal::Int(1001)),  // gid
        (17, TestCborVal::Int(0o644)), // mode
        (18, TestCborVal::Int(1700000010)),
    ]);

    // Root directory item (Offset 0 in block 0)
    let dummy_root_bytes = encode_test_item(&[
        (0, TestCborVal::Int(1)), // type: 1 (dir)
        (1, TestCborVal::Text("/golden_root")),
        (3, TestCborVal::Int(0)),
        (4, TestCborVal::Int(0)),
        (5, TestCborVal::Int(1)),
        (6, TestCborVal::Bool(false)), // rderr: false (subtree error)
        (7, TestCborVal::Int(10000)),
        (8, TestCborVal::Int(16384)),
        (11, TestCborVal::Int(2)),
        (12, TestCborVal::Int(100)), // placeholder sub
        (15, TestCborVal::Int(1000)),
        (16, TestCborVal::Int(1000)),
        (17, TestCborVal::Int(0o755)),
        (18, TestCborVal::Int(1700000000)),
        (99, TestCborVal::Text("unknown_field_tolerance")),
    ]);

    let child2_offset = dummy_root_bytes.len() + child1_bytes.len();
    let root_bytes = encode_test_item(&[
        (0, TestCborVal::Int(1)),
        (1, TestCborVal::Text("/golden_root")),
        (3, TestCborVal::Int(0)),
        (4, TestCborVal::Int(0)),
        (5, TestCborVal::Int(1)),
        (6, TestCborVal::Bool(false)),
        (7, TestCborVal::Int(10000)),
        (8, TestCborVal::Int(16384)),
        (11, TestCborVal::Int(2)),
        (12, TestCborVal::Int(child2_offset as i64)), // absolute sub to last child
        (15, TestCborVal::Int(1000)),
        (16, TestCborVal::Int(1000)),
        (17, TestCborVal::Int(0o755)),
        (18, TestCborVal::Int(1700000000)),
        (99, TestCborVal::Text("unknown_field_tolerance")),
    ]);
    assert_eq!(root_bytes.len(), dummy_root_bytes.len());

    let mut decompressed_final = Vec::new();
    decompressed_final.extend_from_slice(&root_bytes);
    decompressed_final.extend_from_slice(&child1_bytes);
    decompressed_final.extend_from_slice(&child2_bytes);

    // 2. Package into ncdu 2.x binary container
    let mut golden_file = Vec::new();
    golden_file.extend_from_slice(b"\xbfncduEX1");

    // Compress Block 0
    let compressed_block_0 = zstd::stream::encode_all(&decompressed_final[..], 3).unwrap();
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&0u32.to_be_bytes()); // block 0
    data_payload.extend_from_slice(&compressed_block_0);

    let block_len = 4 + data_payload.len() as u32 + 4;
    let typelen = block_len; // Type 0
    let typelen_bytes = typelen.to_be_bytes();

    let block_0_file_offset = golden_file.len() as u64;
    golden_file.extend_from_slice(&typelen_bytes);
    golden_file.extend_from_slice(&data_payload);
    golden_file.extend_from_slice(&typelen_bytes);

    // Index Block
    let mut index_content = Vec::new();
    let pointer_0 = (block_0_file_offset << 24) | (block_len as u64 & 0xFFFFFF);
    index_content.extend_from_slice(&pointer_0.to_be_bytes());
    let root_itemref = 0u64; // Block 0, offset 0
    index_content.extend_from_slice(&root_itemref.to_be_bytes());

    let index_block_len = 4 + index_content.len() as u32 + 4;
    let index_typelen = (1u32 << 28) | (index_block_len & 0x0FFFFFFF);
    let index_typelen_bytes = index_typelen.to_be_bytes();

    golden_file.extend_from_slice(&index_typelen_bytes);
    golden_file.extend_from_slice(&index_content);
    golden_file.extend_from_slice(&index_typelen_bytes);

    // 3. Test importation
    let imported = bin_read::import_bin(&golden_file).expect("Import golden binary file failed");
    let root_node = imported.get(imported.root);

    // Check root metadata & SUB_ERROR flag propagation from rderr=false
    assert_eq!(root_node.name.as_ref(), "/golden_root");
    assert!(root_node.flags.contains(EntryFlags::SUB_ERROR));
    assert_eq!(root_node.extended.as_ref().unwrap().uid, 1000);
    assert_eq!(root_node.extended.as_ref().unwrap().mode, 0o755);

    // Check children ordering (right-to-left prev chain reversed to left-to-right)
    assert_eq!(root_node.children.len(), 2);
    let c1 = imported.get(root_node.children[0]);
    let c2 = imported.get(root_node.children[1]);

    assert_eq!(c1.name.as_ref(), "first_child.txt");
    assert_eq!(c1.asize, 5000);

    assert_eq!(c2.name.as_ref(), "second_child.bin");
    assert_eq!(c2.ino, 77777);
    assert_eq!(c2.nlink, 2);
    assert_eq!(c2.extended.as_ref().unwrap().uid, 1001);
}
