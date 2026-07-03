pub mod bin_read;
pub mod bin_write;
pub mod compress;
pub mod json_read;
pub mod json_write;

use crate::tree::TreeArena;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

pub fn import_file(path: &Path) -> Result<TreeArena> {
    // Check magic bytes or extension to select JSON or binary format
    let file_bytes = compress::read_file_maybe_compressed(path)?;

    // Check if it starts with the binary format signature: "\xbfncduEX1"
    if file_bytes.starts_with(b"\xbfncduEX1") {
        bin_read::import_bin(&file_bytes)
    } else {
        json_read::import_json(&file_bytes)
    }
}

pub fn export_json(
    arena: &TreeArena,
    path: &Path,
    compress: bool,
    compress_level: i32,
) -> Result<()> {
    let json_bytes = json_write::export_json(arena)?;
    if compress {
        compress::write_compressed_file(path, &json_bytes, compress_level)
    } else {
        if path == Path::new("-") {
            std::io::stdout().write_all(&json_bytes)?;
        } else {
            std::fs::write(path, json_bytes)?;
        }
        Ok(())
    }
}

pub fn export_bin(
    arena: &TreeArena,
    path: &Path,
    block_size_kb: usize,
    compress_level: i32,
) -> Result<()> {
    let bin_bytes = bin_write::export_bin(arena, block_size_kb, compress_level)?;
    if path == Path::new("-") {
        std::io::stdout().write_all(&bin_bytes)?;
    } else {
        std::fs::write(path, bin_bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryFlags, ExtendedInfo, TreeNode};

    #[test]
    fn test_json_and_binary_roundtrip() -> Result<()> {
        // 1. Build a mock tree arena
        let root_node = TreeNode::new_dir(
            "root".to_string(),
            1,
            100,
            EntryFlags::empty(),
            Some(ExtendedInfo {
                mtime: 123456,
                uid: 1000,
                gid: 1000,
                mode: 0o755,
            }),
        );
        let mut arena = TreeArena::new(root_node);
        let root_id = arena.root;

        let child_dir =
            TreeNode::new_dir("child_dir".to_string(), 1, 101, EntryFlags::empty(), None);
        let child_dir_id = arena.add_child(root_id, child_dir);

        let file_1 = TreeNode::new_file(
            "file_1.txt".to_string(),
            500,
            512,
            1,
            201,
            1,
            EntryFlags::empty(),
            None,
        );
        arena.add_child(child_dir_id, file_1);

        // Add a hard link
        let hard_link = TreeNode::new_file(
            "file_1_hard.txt".to_string(),
            500,
            512,
            1,
            201, // same device & inode
            2,
            EntryFlags::HARD_LINK,
            None,
        );
        arena.add_child(child_dir_id, hard_link);

        // Recalculate stats
        crate::tree::stats::recalculate_stats(&mut arena);

        // 2. Test JSON export & import roundtrip
        let json_bytes = json_write::export_json(&arena)?;
        let imported_json_arena = json_read::import_json(&json_bytes)?;

        // Verify JSON roundtrip
        assert_eq!(imported_json_arena.nodes.len(), arena.nodes.len());
        assert_eq!(
            imported_json_arena
                .get(imported_json_arena.root)
                .name
                .as_ref(),
            "root"
        );

        let children = &imported_json_arena.get(imported_json_arena.root).children;
        assert_eq!(children.len(), 1);
        let child_dir_node = imported_json_arena.get(children[0]);
        assert_eq!(child_dir_node.name.as_ref(), "child_dir");
        assert_eq!(child_dir_node.children.len(), 2);

        // 3. Test Binary CBOR export & import roundtrip
        // Use zstd level 1 compression for speed in testing
        let bin_bytes = bin_write::export_bin(&arena, 64, 1)?;
        let imported_bin_arena = bin_read::import_bin(&bin_bytes)?;

        // Verify binary roundtrip
        assert_eq!(imported_bin_arena.nodes.len(), arena.nodes.len());
        assert_eq!(
            imported_bin_arena
                .get(imported_bin_arena.root)
                .name
                .as_ref(),
            "root"
        );

        let root_bin = imported_bin_arena.get(imported_bin_arena.root);
        assert_eq!(root_bin.extended.as_ref().unwrap().mtime, 123456);

        let bin_children = &root_bin.children;
        assert_eq!(bin_children.len(), 1);
        let bin_child_dir_node = imported_bin_arena.get(bin_children[0]);
        assert_eq!(bin_child_dir_node.name.as_ref(), "child_dir");
        assert_eq!(bin_child_dir_node.children.len(), 2);

        let bin_file_1 = imported_bin_arena.get(bin_child_dir_node.children[0]);
        assert_eq!(bin_file_1.name.as_ref(), "file_1.txt");
        assert_eq!(bin_file_1.asize, 500);
        assert_eq!(bin_file_1.dsize, 512);

        let bin_hard_link = imported_bin_arena.get(bin_child_dir_node.children[1]);
        assert_eq!(bin_hard_link.name.as_ref(), "file_1_hard.txt");
        assert!(bin_hard_link.flags.contains(EntryFlags::HARD_LINK));

        Ok(())
    }
}
