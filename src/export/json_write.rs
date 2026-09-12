use crate::tree::{EntryFlags, NodeId, TreeArena};
use anyhow::Result;
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct JsonFile<'a> {
    name: &'a str,
    asize: i64,
    dsize: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev: Option<u64>,
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
    excluded: Option<&'static str>,
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
struct Metadata<'a> {
    progname: &'a str,
    progver: &'a str,
    timestamp: u64,
}

pub fn export_json(arena: &TreeArena) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let metadata = Metadata {
        progname: "rusdu",
        progver: env!("CARGO_PKG_VERSION"),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    write!(out, "[1,2,")?;
    serde_json::to_writer(&mut out, &metadata)?;
    write!(out, ",")?;
    write_node(arena, arena.root, &mut out)?;
    write!(out, "]")?;

    Ok(out)
}

fn write_node(arena: &TreeArena, node_id: NodeId, out: &mut Vec<u8>) -> Result<()> {
    let node = arena.get(node_id);

    let is_read_error = node.flags.contains(EntryFlags::READ_ERROR);
    let is_excluded = node.flags.contains(EntryFlags::EXCLUDED);
    let is_not_reg = node.flags.contains(EntryFlags::NOT_REG);
    let is_othfs = node.flags.contains(EntryFlags::OTHER_FS);
    let is_kernfs = node.flags.contains(EntryFlags::KERNFS);
    let is_hlnkc = node.flags.contains(EntryFlags::HARD_LINK);

    let excluded_str = if is_kernfs {
        Some("kernfs")
    } else if is_othfs {
        Some("otherfs")
    } else if is_excluded {
        Some("pattern")
    } else {
        None
    };

    let item = JsonFile {
        name: &node.name,
        asize: node.asize,
        dsize: node.dsize,
        dev: if node.dev != 0 { Some(node.dev) } else { None },
        ino: if node.nlink > 1 && node.ino != 0 {
            Some(node.ino)
        } else {
            None
        },
        nlink: if node.nlink > 1 {
            Some(node.nlink)
        } else {
            None
        },
        uid: node.extended.as_ref().map(|e| e.uid),
        gid: node.extended.as_ref().map(|e| e.gid),
        mode: node.extended.as_ref().map(|e| e.mode),
        mtime: node.extended.as_ref().map(|e| e.mtime),
        read_error: if is_read_error { Some(true) } else { None },
        excluded: excluded_str,
        notreg: if is_not_reg { Some(true) } else { None },
        othfs: if is_othfs { Some(true) } else { None },
        kernfs: if is_kernfs { Some(true) } else { None },
        hlnkc: if is_hlnkc { Some(true) } else { None },
    };

    if node.is_dir() {
        out.push(b'[');
        serde_json::to_writer(&mut *out, &item)?;
        for &child_id in &node.children {
            out.push(b',');
            write_node(arena, child_id, out)?;
        }
        out.push(b']');
    } else {
        serde_json::to_writer(&mut *out, &item)?;
    }

    Ok(())
}
