use crate::tree::{EntryFlags, NodeId, TreeArena};
use anyhow::Result;
use std::collections::HashMap;

pub fn export_bin(
    arena: &TreeArena,
    _block_size_kb: usize,
    compress_level: i32,
) -> Result<Vec<u8>> {
    let mut file_bytes = Vec::new();

    // 1. Write File Signature: "\xbfncduEX1"
    file_bytes.extend_from_slice(b"\xbfncduEX1");

    // We will write all items in a single data block (Block 0)
    let mut data_buffer = Vec::new();

    // Data block header requires block number (4 bytes big-endian: 0x00000000)
    data_buffer.extend_from_slice(&[0, 0, 0, 0]);

    // Decompressed payload of block 0
    let mut decompressed = Vec::new();

    // Map to keep track of absolute offsets of each written node in `decompressed`
    let mut node_offsets = HashMap::new();

    // List of backpatch locations for `sub` pointers: (parent_node_id, offset_in_decompressed)
    let mut sub_backpatches = Vec::new();

    // Perform depth-first traversal to serialize items
    let root_id = arena.root;
    serialize_item_dfs(
        arena,
        root_id,
        &mut decompressed,
        &mut node_offsets,
        &mut sub_backpatches,
    )?;

    // Backpatch all `sub` (key 12) pointers
    // Since `sub` points to the first child, and first child was written, we get its offset.
    for (parent_id, patch_pos) in sub_backpatches {
        let parent = arena.get(parent_id);
        if let Some(&first_child_id) = parent.children.first() {
            if let Some(&child_offset) = node_offsets.get(&first_child_id) {
                // Absolute Itemref: block number (40 bits = 0) and offset (24 bits)
                // In CBOR, the value was written as a fixed 9-byte u64 (0x1b followed by 8 bytes)
                // We overwrite the 8 big-endian bytes at patch_pos.
                let absolute_ref = child_offset as u64; // Block 0, offset child_offset
                let bytes = absolute_ref.to_be_bytes();
                decompressed[patch_pos..patch_pos + 8].copy_from_slice(&bytes);
            }
        }
    }

    // Compress the decompressed data using Zstandard
    let compressed_data = zstd::stream::encode_all(&decompressed[..], compress_level)?;
    data_buffer.extend_from_slice(&compressed_data);

    // Write the Data Block (Type 0)
    // TypeLen (4 bytes): block type 0 (high 4 bits), length (low 28 bits)
    // Block length = TypeLen header (4 bytes) + Content (data_buffer.len()) + TypeLen footer (4 bytes)
    let block_len = 4 + data_buffer.len() as u32 + 4;
    let typelen = block_len; // Type is 0, so high 4 bits are 0000
    let typelen_bytes = typelen.to_be_bytes();

    file_bytes.extend_from_slice(&typelen_bytes);
    file_bytes.extend_from_slice(&data_buffer);
    file_bytes.extend_from_slice(&typelen_bytes);

    // Write the Index Block (Type 1)
    let mut index_content = Vec::new();

    // Block_pointers: array of 8-byte pointers for each data block.
    // We only have 1 data block (Block 0).
    // Pointer format: 64-bit big-endian: higher 40 bits = offset of block header (8 bytes file signature), lower 24 bits = block length
    let block_0_offset = 8u64; // Starts right after file signature
    let pointer_0 = (block_0_offset << 24) | (block_len as u64 & 0xFFFFFF);
    index_content.extend_from_slice(&pointer_0.to_be_bytes());

    // Root_itemref: final 8 bytes pointing to root item. Absolute Itemref: Block 0, Offset 0.
    let root_offset = node_offsets.get(&root_id).cloned().unwrap_or(0) as u64;
    let root_itemref = root_offset & 0xFFFFFF;
    index_content.extend_from_slice(&root_itemref.to_be_bytes());

    // Write index block
    // TypeLen: type 1 (high 4 bits: 0x1), length (low 28 bits)
    let index_block_len = 4 + index_content.len() as u32 + 4;
    let index_typelen = (1u32 << 28) | (index_block_len & 0x0FFFFFFF);
    let index_typelen_bytes = index_typelen.to_be_bytes();

    file_bytes.extend_from_slice(&index_typelen_bytes);
    file_bytes.extend_from_slice(&index_content);
    file_bytes.extend_from_slice(&index_typelen_bytes);

    Ok(file_bytes)
}

fn serialize_item_dfs(
    arena: &TreeArena,
    node_id: NodeId,
    buf: &mut Vec<u8>,
    node_offsets: &mut HashMap<NodeId, usize>,
    sub_backpatches: &mut Vec<(NodeId, usize)>,
) -> Result<()> {
    let node = arena.get(node_id);
    let offset = buf.len();
    node_offsets.insert(node_id, offset);

    // Build fields list to count maps size
    let mut fields = Vec::new();

    // 0: type
    fields.push((0u8, CborValue::Int(if node.is_dir() { 1 } else { 0 })));

    // 1: name
    fields.push((1u8, CborValue::Text(node.name.to_string())));

    // 2: prev (relative Itemref to previous sibling)
    // Find previous sibling
    if let Some(parent_id) = node.parent {
        let parent = arena.get(parent_id);
        if let Some(pos) = parent.children.iter().position(|&id| id == node_id) {
            if pos > 0 {
                let prev_sibling_id = parent.children[pos - 1];
                if let Some(&prev_offset) = node_offsets.get(&prev_sibling_id) {
                    let rel_ref = (prev_offset as i64) - (offset as i64);
                    fields.push((2u8, CborValue::Int(rel_ref)));
                }
            }
        }
    }

    // 3: asize
    fields.push((3u8, CborValue::Int(node.asize)));

    // 4: dsize
    fields.push((4u8, CborValue::Int(node.dsize)));

    // 5: dev
    fields.push((5u8, CborValue::Int(node.dev as i64)));

    // 6: rderr
    if node.flags.contains(EntryFlags::READ_ERROR) {
        fields.push((6u8, CborValue::Bool(true)));
    }

    if node.is_dir() {
        let stats = node.get_stats();
        // 7: cumasize
        fields.push((7u8, CborValue::Int(stats.total_asize)));
        // 8: cumdsize
        fields.push((8u8, CborValue::Int(stats.total_dsize)));
        // 11: items
        fields.push((11u8, CborValue::Int(stats.item_count as i64)));

        // 12: sub (first child)
        if !node.children.is_empty() {
            // Write placeholder for first child absolute Itemref (Fixed 9-byte u64)
            fields.push((12u8, CborValue::PlaceholderU64));
        }
    }

    // 13: ino
    fields.push((13u8, CborValue::Int(node.ino as i64)));

    // 14: nlink
    fields.push((14u8, CborValue::Int(node.nlink as i64)));

    if let Some(ref ext) = node.extended {
        // 15: uid
        fields.push((15u8, CborValue::Int(ext.uid as i64)));
        // 16: gid
        fields.push((16u8, CborValue::Int(ext.gid as i64)));
        // 17: mode
        fields.push((17u8, CborValue::Int(ext.mode as i64)));
        // 18: mtime
        fields.push((18u8, CborValue::Int(ext.mtime)));
    }

    // Write CBOR Map Header
    let map_len = fields.len();
    if map_len < 24 {
        buf.push(0xa0 | (map_len as u8));
    } else {
        buf.push(0xb8);
        buf.push(map_len as u8);
    }

    for (key, val) in fields {
        // Write key (always < 24)
        buf.push(key);

        // Write value
        match val {
            CborValue::Int(v) => write_cbor_int(buf, v),
            CborValue::Text(s) => write_cbor_text(buf, &s),
            CborValue::Bool(b) => buf.push(if b { 0xf5 } else { 0xf4 }),
            CborValue::PlaceholderU64 => {
                // Fixed 9-byte u64 placeholder
                buf.push(0x1b);
                let patch_offset = buf.len();
                sub_backpatches.push((node_id, patch_offset));
                buf.extend_from_slice(&[0; 8]); // Placeholder bytes
            }
        }
    }

    // Recursively serialize children
    for &child_id in &node.children {
        serialize_item_dfs(arena, child_id, buf, node_offsets, sub_backpatches)?;
    }

    Ok(())
}

enum CborValue {
    Int(i64),
    Text(String),
    Bool(bool),
    PlaceholderU64,
}

fn write_cbor_int(buf: &mut Vec<u8>, val: i64) {
    if val >= 0 {
        let u = val as u64;
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
        let n = -1 - val;
        let u = n as u64;
        if u < 24 {
            buf.push(0x20 | (u as u8));
        } else if u <= 0xff {
            buf.push(0x38);
            buf.push(u as u8);
        } else if u <= 0xffff {
            buf.push(0x39);
            buf.extend_from_slice(&(u as u16).to_be_bytes());
        } else if u <= 0xffffffff {
            buf.push(0x3a);
            buf.extend_from_slice(&(u as u32).to_be_bytes());
        } else {
            buf.push(0x3b);
            buf.extend_from_slice(&u.to_be_bytes());
        }
    }
}

fn write_cbor_text(buf: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let len = bytes.len() as u64;
    if len < 24 {
        buf.push(0x60 | (len as u8));
    } else if len <= 0xff {
        buf.push(0x78);
        buf.push(len as u8);
    } else if len <= 0xffff {
        buf.push(0x79);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= 0xffffffff {
        buf.push(0x7a);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        buf.push(0x7b);
        buf.extend_from_slice(&len.to_be_bytes());
    }
    buf.extend_from_slice(bytes);
}
