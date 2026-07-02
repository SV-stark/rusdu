use crate::tree::{EntryFlags, ExtendedInfo, NodeId, TreeArena, TreeNode};
use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)]
struct JsonFileImport {
    name: String,
    #[serde(default)]
    asize: i64,
    #[serde(default)]
    dsize: i64,
    #[serde(default)]
    ino: Option<u64>,
    #[serde(default)]
    nlink: Option<u32>,
    #[serde(default)]
    uid: Option<u32>,
    #[serde(default)]
    gid: Option<u32>,
    #[serde(default)]
    mode: Option<u32>,
    #[serde(default)]
    mtime: Option<i64>,
    #[serde(default)]
    read_error: Option<bool>,
    #[serde(default)]
    excluded: Option<bool>,
    #[serde(default)]
    notreg: Option<bool>,
    #[serde(default)]
    othfs: Option<bool>,
    #[serde(default)]
    kernfs: Option<bool>,
    #[serde(default)]
    hlnkc: Option<bool>,
}

pub fn import_json(json_bytes: &[u8]) -> Result<TreeArena> {
    let root_val: Value = serde_json::from_slice(json_bytes)?;

    // Expected shape: [majorver, minorver, metadata, root_directory]
    let root_array = root_val
        .as_array()
        .ok_or_else(|| anyhow!("JSON top-level is not an array"))?;

    if root_array.len() < 4 {
        return Err(anyhow!("Invalid JSON import array layout"));
    }

    let majorver = root_array[0]
        .as_i64()
        .ok_or_else(|| anyhow!("Invalid major version"))?;
    if majorver != 1 {
        return Err(anyhow!("Unsupported major version: {}", majorver));
    }

    let root_dir_val = &root_array[3];
    let mut arena = deserialize_node(root_dir_val, None, None)?;

    // Recalculate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    Ok(arena)
}

fn deserialize_node(
    val: &Value,
    mut arena: Option<&mut TreeArena>,
    parent_id: Option<NodeId>,
) -> Result<TreeArena> {
    if val.is_array() {
        // Directory
        let dir_array = val.as_array().unwrap();
        if dir_array.is_empty() {
            return Err(anyhow!("Empty directory array in JSON"));
        }

        // Index 0 is metadata
        let meta_import: JsonFileImport = serde_json::from_value(dir_array[0].clone())?;

        let mut flags = EntryFlags::IS_DIR;
        if meta_import.read_error.unwrap_or(false) {
            flags.insert(EntryFlags::READ_ERROR);
        }
        if meta_import.excluded.unwrap_or(false) {
            flags.insert(EntryFlags::EXCLUDED);
        }
        if meta_import.othfs.unwrap_or(false) {
            flags.insert(EntryFlags::OTHER_FS);
        }
        if meta_import.kernfs.unwrap_or(false) {
            flags.insert(EntryFlags::KERNFS);
        }

        let has_extended = meta_import.uid.is_some()
            || meta_import.gid.is_some()
            || meta_import.mode.is_some()
            || meta_import.mtime.is_some();

        let extended = if has_extended {
            Some(ExtendedInfo {
                mtime: meta_import.mtime.unwrap_or(0),
                uid: meta_import.uid.unwrap_or(0),
                gid: meta_import.gid.unwrap_or(0),
                mode: meta_import.mode.unwrap_or(0),
            })
        } else {
            None
        };

        let dir_node = TreeNode::new_dir(
            meta_import.name,
            0, // dev not present directly in import struct
            meta_import.ino.unwrap_or(0),
            flags,
            extended,
        );

        let mut local_arena = None;
        let active_arena;
        let current_dir_id;

        match arena.as_mut() {
            Some(a) => {
                let parent = parent_id.unwrap();
                current_dir_id = a.add_child(parent, dir_node);
                active_arena = &mut **a;
            }
            None => {
                let la = TreeArena::new(dir_node);
                current_dir_id = la.root;
                local_arena = Some(la);
                active_arena = local_arena.as_mut().unwrap();
            }
        }

        // Remaining elements are children
        for child_val in &dir_array[1..] {
            deserialize_node(child_val, Some(active_arena), Some(current_dir_id))?;
        }

        if local_arena.is_some() {
            Ok(local_arena.unwrap())
        } else {
            // Dummy arena returned when nested
            Ok(TreeArena::new(TreeNode::new_dir(
                String::new(),
                0,
                0,
                EntryFlags::empty(),
                None,
            )))
        }
    } else {
        // File
        let file_import: JsonFileImport = serde_json::from_value(val.clone())?;

        let mut flags = EntryFlags::empty();
        if file_import.read_error.unwrap_or(false) {
            flags.insert(EntryFlags::READ_ERROR);
        }
        if file_import.excluded.unwrap_or(false) {
            flags.insert(EntryFlags::EXCLUDED);
        }
        if file_import.notreg.unwrap_or(false) {
            flags.insert(EntryFlags::NOT_REG);
        }
        if file_import.hlnkc.unwrap_or(false) {
            flags.insert(EntryFlags::HARD_LINK);
        }

        let has_extended = file_import.uid.is_some()
            || file_import.gid.is_some()
            || file_import.mode.is_some()
            || file_import.mtime.is_some();

        let extended = if has_extended {
            Some(ExtendedInfo {
                mtime: file_import.mtime.unwrap_or(0),
                uid: file_import.uid.unwrap_or(0),
                gid: file_import.gid.unwrap_or(0),
                mode: file_import.mode.unwrap_or(0),
            })
        } else {
            None
        };

        let file_node = TreeNode::new_file(
            file_import.name,
            file_import.asize,
            file_import.dsize,
            0,
            file_import.ino.unwrap_or(0),
            file_import.nlink.unwrap_or(1),
            flags,
            extended,
        );

        if let Some(ref mut a) = arena {
            let parent = parent_id.unwrap();
            a.add_child(parent, file_node);
        }

        Ok(TreeArena::new(TreeNode::new_dir(
            String::new(),
            0,
            0,
            EntryFlags::empty(),
            None,
        )))
    }
}
