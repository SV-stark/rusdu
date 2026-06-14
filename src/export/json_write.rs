use crate::tree::{EntryFlags, NodeId, TreeArena};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
#[serde(untagged)]
enum JsonItem {
    File(JsonFile),
    Dir(Vec<serde_json::Value>),
}

#[derive(Serialize)]
struct JsonFile {
    name: String,
    asize: i64,
    dsize: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ino: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nlink: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notreg: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    othfs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernfs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hlnkc: Option<bool>,
}

#[derive(Serialize)]
struct Metadata {
    progname: String,
    progver: String,
    timestamp: u64,
}

pub fn export_json(arena: &TreeArena) -> Result<Vec<u8>> {
    let root_id = arena.root;
    let serialized_tree = serialize_node(arena, root_id)?;

    let metadata = Metadata {
        progname: "rusdu".to_string(),
        progver: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    // Construct top-level array: [majorver, minorver, metadata, root_directory]
    let top_level = (
        1, // majorver
        2, // minorver (with nlink and dev support)
        metadata,
        serialized_tree,
    );

    let bytes = serde_json::to_vec_pretty(&top_level)?;
    Ok(bytes)
}

fn serialize_node(arena: &TreeArena, node_id: NodeId) -> Result<serde_json::Value> {
    let node = arena.get(node_id);

    let is_read_error = node.flags.contains(EntryFlags::READ_ERROR);
    let is_excluded = node.flags.contains(EntryFlags::EXCLUDED);
    let is_not_reg = node.flags.contains(EntryFlags::NOT_REG);
    let is_othfs = node.flags.contains(EntryFlags::OTHER_FS);
    let is_kernfs = node.flags.contains(EntryFlags::KERNFS);
    let is_hlnkc = node.flags.contains(EntryFlags::HARD_LINK);

    let item = JsonFile {
        name: node.name.to_string(),
        asize: node.asize,
        dsize: node.dsize,
        ino: Some(node.ino),
        nlink: Some(node.nlink),
        uid: node.extended.as_ref().map(|e| e.uid),
        gid: node.extended.as_ref().map(|e| e.gid),
        mode: node.extended.as_ref().map(|e| e.mode),
        mtime: node.extended.as_ref().map(|e| e.mtime),
        read_error: if is_read_error { Some(true) } else { None },
        excluded: if is_excluded { Some(true) } else { None },
        notreg: if is_not_reg { Some(true) } else { None },
        othfs: if is_othfs { Some(true) } else { None },
        kernfs: if is_kernfs { Some(true) } else { None },
        hlnkc: if is_hlnkc { Some(true) } else { None },
    };

    if node.is_dir() {
        // Directory metadata node uses same fields
        let metadata_val = serde_json::to_value(&item)?;
        let mut dir_array = vec![metadata_val];

        for &child_id in &node.children {
            let child_val = serialize_node(arena, child_id)?;
            dir_array.push(child_val);
        }

        Ok(serde_json::Value::Array(dir_array))
    } else {
        let file_val = serde_json::to_value(&item)?;
        Ok(file_val)
    }
}
