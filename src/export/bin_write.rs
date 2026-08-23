use crate::tree::{EntryFlags, NodeId, TreeArena};
use anyhow::{Result, anyhow};
use rustc_hash::FxHashMap;

struct DecompressedBlock {
    data: Vec<u8>,
}

pub fn export_bin(arena: &TreeArena, block_size_kb: usize, compress_level: i32) -> Result<Vec<u8>> {
    let mut file_bytes = Vec::new();

    // 1. Write File Signature: "\xbfncduEX1"
    file_bytes.extend_from_slice(b"\xbfncduEX1");

    // Target block size in bytes (clamped to max 15.8 MiB to safely fit 24-bit pointer limits)
    let target_block_bytes = if block_size_kb == 0 {
        64 * 1024
    } else {
        (block_size_kb * 1024).clamp(32 * 1024, 0x00F0_0000)
    };

    let mut blocks: Vec<DecompressedBlock> = vec![DecompressedBlock { data: Vec::new() }];

    // Map each node to its (block_num, offset_in_decompressed_block)
    let mut node_offsets: FxHashMap<NodeId, (u32, usize)> = FxHashMap::default();

    // Backpatch locations for `sub` pointers: (parent_node_id, block_num, offset_in_decompressed_block)
    let mut sub_backpatches: Vec<(NodeId, usize, usize)> = Vec::new();

    // Perform depth-first traversal to serialize items across data blocks
    let root_id = arena.root;
    serialize_item_dfs(
        arena,
        root_id,
        None,
        target_block_bytes,
        &mut blocks,
        &mut node_offsets,
        &mut sub_backpatches,
    )?;

    // Backpatch all `sub` (key 12) pointers to point to the LAST child
    for (parent_id, block_idx, patch_pos) in sub_backpatches {
        let parent = arena.get(parent_id);
        if let Some(&last_child_id) = parent.children.last() {
            if let Some(&(child_block_num, child_offset)) = node_offsets.get(&last_child_id) {
                let absolute_ref =
                    ((child_block_num as u64) << 24) | (child_offset as u64 & 0xFFFFFF);
                let bytes = absolute_ref.to_be_bytes();
                blocks[block_idx].data[patch_pos..patch_pos + 8].copy_from_slice(&bytes);
            }
        }
    }

    let mut index_content = Vec::new();

    // Compress and write each Data Block (Type 0)
    for (block_idx, block) in blocks.iter().enumerate() {
        if block.data.len() > 0x00FF_FFFF {
            return Err(anyhow!(
                "Decompressed block size exceeds 16 MiB limit: {} bytes",
                block.data.len()
            ));
        }

        let compressed_data = zstd::stream::encode_all(&block.data[..], compress_level)?;

        // Block payload = 4 bytes block_num + compressed_data
        let mut data_payload = Vec::with_capacity(4 + compressed_data.len());
        data_payload.extend_from_slice(&(block_idx as u32).to_be_bytes());
        data_payload.extend_from_slice(&compressed_data);

        // Block length = TypeLen header (4 bytes) + data_payload + TypeLen footer (4 bytes)
        let block_len = 4 + data_payload.len() as u32 + 4;
        if block_len > 0x00FF_FFFF {
            return Err(anyhow!(
                "Compressed block size exceeds 24-bit pointer limit: {} bytes",
                block_len
            ));
        }

        let typelen = block_len; // Type 0 (high 4 bits 0x0)
        let typelen_bytes = typelen.to_be_bytes();

        let file_offset = file_bytes.len() as u64;

        file_bytes.extend_from_slice(&typelen_bytes);
        file_bytes.extend_from_slice(&data_payload);
        file_bytes.extend_from_slice(&typelen_bytes);

        // Record 8-byte pointer in index table
        let pointer = (file_offset << 24) | (block_len as u64 & 0xFFFFFF);
        index_content.extend_from_slice(&pointer.to_be_bytes());
    }

    // Root_itemref: final 8 bytes in index block pointing to root item
    let &(root_block, root_offset) = node_offsets
        .get(&root_id)
        .ok_or_else(|| anyhow!("Root offset missing"))?;
    let root_itemref = ((root_block as u64) << 24) | (root_offset as u64 & 0xFFFFFF);
    index_content.extend_from_slice(&root_itemref.to_be_bytes());

    // Write the Index Block (Type 1)
    let index_block_len = 4 + index_content.len() as u32 + 4;
    if index_block_len > 0x00FF_FFFF {
        return Err(anyhow!("Index block length exceeds 24-bit pointer limit"));
    }
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
    prev_sibling_id: Option<NodeId>,
    target_block_bytes: usize,
    blocks: &mut Vec<DecompressedBlock>,
    node_offsets: &mut FxHashMap<NodeId, (u32, usize)>,
    sub_backpatches: &mut Vec<(NodeId, usize, usize)>,
) -> Result<()> {
    // Check if the current block has reached target size
    let curr_block_len = blocks.last().unwrap().data.len();
    if curr_block_len >= target_block_bytes {
        blocks.push(DecompressedBlock { data: Vec::new() });
    }

    let block_idx = blocks.len() - 1;
    let offset = blocks[block_idx].data.len();
    node_offsets.insert(node_id, (block_idx as u32, offset));

    let node = arena.get(node_id);

    // Determine ncdu 2.x item type code:
    // 1 = dir, 0 = file, 2 = not-reg, 3 = hardlink, -1 = read-err, -2 = excluded, -3 = other-fs, -4 = kernfs
    let item_type = if node.flags.contains(EntryFlags::KERNFS) {
        -4i64
    } else if node.flags.contains(EntryFlags::OTHER_FS) {
        -3i64
    } else if node.flags.contains(EntryFlags::EXCLUDED) {
        -2i64
    } else if node.flags.contains(EntryFlags::READ_ERROR) && !node.is_dir() {
        -1i64
    } else if node.is_dir() {
        1i64
    } else if node.flags.contains(EntryFlags::NOT_REG) {
        2i64
    } else if node.flags.contains(EntryFlags::HARD_LINK) || node.nlink > 1 {
        3i64
    } else {
        0i64
    };

    // Build fields list to count maps size
    let mut fields = Vec::new();

    // 0: type
    fields.push((0u8, CborValue::Int(item_type)));

    // 1: name
    fields.push((1u8, CborValue::Text(node.name.to_string())));

    // 2: prev (relative Itemref if same block, or absolute if different block)
    if let Some(prev_id) = prev_sibling_id {
        if let Some(&(prev_block, prev_offset)) = node_offsets.get(&prev_id) {
            if prev_block == block_idx as u32 {
                let rel_ref = (prev_offset as i64) - (offset as i64);
                fields.push((2u8, CborValue::Int(rel_ref)));
            } else {
                let abs_ref = ((prev_block as u64) << 24) | (prev_offset as u64 & 0xFFFFFF);
                fields.push((2u8, CborValue::Int(abs_ref as i64)));
            }
        }
    }

    // 3: asize
    fields.push((3u8, CborValue::Int(node.asize)));

    // 4: dsize
    fields.push((4u8, CborValue::Int(node.dsize)));

    // 5: dev
    fields.push((5u8, CborValue::Int(node.dev as i64)));

    // 6: rderr (true = error on dir itself, false = error in subtree)
    if node.flags.contains(EntryFlags::READ_ERROR) {
        fields.push((6u8, CborValue::Bool(true)));
    } else if node.flags.contains(EntryFlags::SUB_ERROR) {
        fields.push((6u8, CborValue::Bool(false)));
    }

    if node.is_dir() {
        let stats = node.get_stats();
        // 7: cumasize
        fields.push((7u8, CborValue::Int(stats.total_asize)));
        // 8: cumdsize
        fields.push((8u8, CborValue::Int(stats.total_dsize)));

        // 9: shrasize & 10: shrdsize
        if stats.shared_size > 0 {
            fields.push((9u8, CborValue::Int(stats.shared_size)));
            fields.push((10u8, CborValue::Int(stats.shared_size)));
        }

        // 11: items
        fields.push((11u8, CborValue::Int(stats.item_count as i64)));

        // 12: sub (last child placeholder)
        if !node.children.is_empty() {
            fields.push((12u8, CborValue::PlaceholderU64));
        }
    }

    // 13: ino & 14: nlink (only for type 3 hard-link candidates or when nlink > 1)
    if item_type == 3 || node.nlink > 1 {
        if node.ino != 0 {
            fields.push((13u8, CborValue::Int(node.ino as i64)));
        }
        if node.nlink > 1 {
            fields.push((14u8, CborValue::Int(node.nlink as i64)));
        }
    }

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

    let buf = &mut blocks[block_idx].data;

    // Write CBOR Map Header
    let num_pairs = fields.len();
    if num_pairs <= 23 {
        buf.push(0xA0 | (num_pairs as u8));
    } else {
        buf.push(0xB8);
        buf.push(num_pairs as u8);
    }

    for (k, v) in fields {
        // Write key (unsigned integer)
        if k <= 23 {
            buf.push(k);
        } else {
            buf.push(0x18);
            buf.push(k);
        }

        // Write value
        match v {
            CborValue::Int(val) => {
                encode_cbor_int(buf, val);
            }
            CborValue::Text(s) => {
                encode_cbor_text(buf, &s);
            }
            CborValue::Bool(b) => {
                if b {
                    buf.push(0xF5);
                } else {
                    buf.push(0xF4);
                }
            }
            CborValue::PlaceholderU64 => {
                buf.push(0x1B);
                let patch_pos = buf.len();
                buf.extend_from_slice(&[0u8; 8]);
                sub_backpatches.push((node_id, block_idx, patch_pos));
            }
        }
    }

    // Recursively serialize children
    let children = node.children.clone();
    let mut prev_child = None;
    for child_id in children {
        serialize_item_dfs(
            arena,
            child_id,
            prev_child,
            target_block_bytes,
            blocks,
            node_offsets,
            sub_backpatches,
        )?;
        prev_child = Some(child_id);
    }

    Ok(())
}

enum CborValue {
    Int(i64),
    Text(String),
    Bool(bool),
    PlaceholderU64,
}

fn encode_cbor_int(buf: &mut Vec<u8>, val: i64) {
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

fn encode_cbor_text(buf: &mut Vec<u8>, text: &str) {
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
