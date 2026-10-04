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
    dev: Option<u64>,
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
    mtime: Option<Value>,
    #[serde(default)]
    read_error: Option<bool>,
    #[serde(default)]
    excluded: Option<Value>,
    #[serde(default)]
    notreg: Option<bool>,
    #[serde(default)]
    othfs: Option<bool>,
    #[serde(default)]
    kernfs: Option<bool>,
    #[serde(default)]
    hlnkc: Option<bool>,
}

fn parse_mtime(val: &Option<Value>) -> Option<i64> {
    match val {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else {
                n.as_f64().map(|f| f.floor() as i64)
            }
        }
        _ => None,
    }
}

fn apply_excluded_flag(val: &Option<Value>, flags: &mut EntryFlags) {
    if let Some(ex) = val {
        match ex {
            Value::Bool(b) if *b => {
                flags.insert(EntryFlags::EXCLUDED);
            }
            Value::String(s) => match s.as_str() {
                "kernfs" => flags.insert(EntryFlags::KERNFS | EntryFlags::EXCLUDED),
                "otherfs" => flags.insert(EntryFlags::OTHER_FS),
                _ => flags.insert(EntryFlags::EXCLUDED),
            },
            _ => {}
        }
    }
}

pub fn import_json(json_bytes: &[u8]) -> Result<TreeArena> {
    // The ncdu format is a nested array whose depth equals the directory
    // nesting depth, and both serde_json's parser and `parse_children_recursive`
    // recurse per level. The default 128-level serde_json limit rejected our
    // own export for trees deeper than ~120 levels, so parse on a thread with a
    // large stack and with the recursion limit disabled (see the
    // `unbounded_depth` feature in Cargo.toml).
    // Own the buffer so the closure satisfies `'static`; JSON imports can be
    // large, but the copy is a single allocation.
    let owned = json_bytes.to_vec();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || import_json_inner(&owned))
        .map_err(|e| anyhow!("failed to spawn JSON import thread: {}", e))?
        .join()
        .map_err(|_| anyhow!("JSON import panicked"))?
}

fn import_json_inner(json_bytes: &[u8]) -> Result<TreeArena> {
    let mut deser = serde_json::Deserializer::from_slice(json_bytes);
    deser.disable_recursion_limit();
    let root_val = Value::deserialize(&mut deser)?;
    deser.end()?;

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
    let (root_node, children_slice) = parse_dir_node(root_dir_val)?;
    let mut arena = TreeArena::new(root_node);
    let root_id = arena.root;

    parse_children_recursive(&mut arena, root_id, children_slice)?;

    // Recalculate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    Ok(arena)
}

/// Attach `children` under `parent_id`, iteratively.
///
/// This used to recurse once per directory level, which is a second overflow
/// risk alongside serde_json's parser on deeply nested input.
fn parse_children_recursive(
    arena: &mut TreeArena,
    parent_id: NodeId,
    children: &[Value],
) -> Result<()> {
    // (arena parent id, slice of this level's children still to process)
    let mut stack: Vec<(NodeId, &[Value])> = vec![(parent_id, children)];

    while let Some((parent_id, level)) = stack.pop() {
        for val in level {
            if val.is_array() {
                let (dir_node, sub_children) = parse_dir_node(val)?;
                let child_id = arena.add_child(parent_id, dir_node);
                if !sub_children.is_empty() {
                    stack.push((child_id, sub_children));
                }
            } else {
                let file_node = parse_file_node(val)?;
                arena.add_child(parent_id, file_node);
            }
        }
    }
    Ok(())
}

fn parse_dir_node(val: &Value) -> Result<(TreeNode, &[Value])> {
    let dir_array = val
        .as_array()
        .ok_or_else(|| anyhow!("Expected directory array in JSON"))?;
    if dir_array.is_empty() {
        return Err(anyhow!("Empty directory array in JSON"));
    }

    let meta_import: JsonFileImport = serde_json::from_value(dir_array[0].clone())?;

    let mut flags = EntryFlags::IS_DIR;
    if meta_import.read_error.unwrap_or(false) {
        flags.insert(EntryFlags::READ_ERROR);
    }
    apply_excluded_flag(&meta_import.excluded, &mut flags);
    if meta_import.othfs.unwrap_or(false) {
        flags.insert(EntryFlags::OTHER_FS);
    }
    if meta_import.kernfs.unwrap_or(false) {
        flags.insert(EntryFlags::KERNFS);
    }

    let mtime = parse_mtime(&meta_import.mtime);
    let has_extended = meta_import.uid.is_some()
        || meta_import.gid.is_some()
        || meta_import.mode.is_some()
        || mtime.is_some();

    let extended = if has_extended {
        Some(ExtendedInfo {
            mtime: mtime.unwrap_or(0),
            uid: meta_import.uid.unwrap_or(0),
            gid: meta_import.gid.unwrap_or(0),
            mode: meta_import.mode.unwrap_or(0),
        })
    } else {
        None
    };

    let dir_node = TreeNode::new_dir(
        meta_import.name,
        meta_import.dev.unwrap_or(0),
        meta_import.ino.unwrap_or(0),
        flags,
        extended,
    );

    Ok((dir_node, &dir_array[1..]))
}

fn parse_file_node(val: &Value) -> Result<TreeNode> {
    let file_import: JsonFileImport = serde_json::from_value(val.clone())?;

    let mut flags = EntryFlags::empty();
    if file_import.read_error.unwrap_or(false) {
        flags.insert(EntryFlags::READ_ERROR);
    }
    apply_excluded_flag(&file_import.excluded, &mut flags);
    if file_import.notreg.unwrap_or(false) {
        flags.insert(EntryFlags::NOT_REG);
    }
    if file_import.hlnkc.unwrap_or(false) {
        flags.insert(EntryFlags::HARD_LINK);
    }

    let mtime = parse_mtime(&file_import.mtime);
    let has_extended = file_import.uid.is_some()
        || file_import.gid.is_some()
        || file_import.mode.is_some()
        || mtime.is_some();

    let extended = if has_extended {
        Some(ExtendedInfo {
            mtime: mtime.unwrap_or(0),
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
        file_import.dev.unwrap_or(0),
        file_import.ino.unwrap_or(0),
        file_import.nlink.unwrap_or(1),
        flags,
        extended,
    );

    Ok(file_node)
}
