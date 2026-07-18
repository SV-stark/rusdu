use crate::tree::{EntryFlags, ExtendedInfo, NodeId, TreeArena, TreeNode};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub fn import_bin(file_bytes: &[u8]) -> Result<TreeArena> {
    if file_bytes.len() < 16 {
        return Err(anyhow!("File is too short to be a valid binary export"));
    }

    // Verify signature
    if &file_bytes[0..8] != b"\xbfncduEX1" {
        return Err(anyhow!("Invalid binary file signature"));
    }

    // Parse blocks from start to find index block at the end
    let mut offset = 8;
    let mut data_blocks = HashMap::new();
    let mut index_content = None;

    while offset < file_bytes.len() {
        if offset + 4 > file_bytes.len() {
            break;
        }

        let typelen = u32::from_be_bytes(file_bytes[offset..offset + 4].try_into()?);
        let block_type = (typelen >> 28) & 0xF;
        let block_len = typelen & 0x0FFFFFFF;

        if offset + block_len as usize > file_bytes.len() {
            return Err(anyhow!("Malformed block length"));
        }

        let content_start = offset + 4;
        let content_end = offset + block_len as usize - 4;
        let content = &file_bytes[content_start..content_end];

        if block_type == 0 {
            // Data Block
            if content.len() < 4 {
                return Err(anyhow!("Malformed data block content"));
            }
            let block_num = u32::from_be_bytes(content[0..4].try_into()?);
            let compressed_data = &content[4..];

            // Decompress
            let decompressed = zstd::stream::decode_all(compressed_data)?;
            data_blocks.insert(block_num, decompressed);
        } else if block_type == 1 {
            // Index Block
            index_content = Some(content.to_vec());
        }

        offset += block_len as usize;
    }

    let index_bytes = index_content.ok_or_else(|| anyhow!("Index block not found"))?;

    // The final 8 bytes of the index block is the Root_itemref
    if index_bytes.len() < 8 {
        return Err(anyhow!("Malformed index block"));
    }
    let root_ref_offset = index_bytes.len() - 8;
    let root_itemref =
        u64::from_be_bytes(index_bytes[root_ref_offset..root_ref_offset + 8].try_into()?);

    let root_block_num = (root_itemref >> 24) as u32;
    let root_offset = (root_itemref & 0xFFFFFF) as usize;

    let root_block_data = data_blocks
        .get(&root_block_num)
        .ok_or_else(|| anyhow!("Root data block {} not found", root_block_num))?;

    // Now, decode all items in the block
    let mut id_map = HashMap::new();
    let mut node_list = Vec::new();
    let mut parent_child_links = Vec::new();
    let mut prev_sibling_links = Vec::new();

    let mut cursor = std::io::Cursor::new(root_block_data);
    while (cursor.position() as usize) < root_block_data.len() {
        let item_offset = cursor.position() as usize;

        let value: ciborium::value::Value = match ciborium::de::from_reader(&mut cursor) {
            Ok(val) => val,
            Err(_) => break, // Reached end of stream
        };

        let map = match value {
            ciborium::value::Value::Map(m) => m,
            _ => continue,
        };

        // Parse fields
        let mut item_type = 0;
        let mut name = String::new();
        let mut prev = None;
        let mut asize = 0;
        let mut dsize = 0;
        let mut dev = 0;
        let mut rderr = false;
        let mut ino = 0;
        let mut nlink = 1;
        let mut uid = None;
        let mut gid = None;
        let mut mode = None;
        let mut mtime = None;
        let mut sub = None;

        for (k, v) in map {
            let key = match k {
                ciborium::value::Value::Integer(i) => i.try_into().unwrap_or(255u8),
                _ => continue,
            };

            match key {
                0 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        item_type = i.try_into().unwrap_or(0);
                    }
                }
                1 => {
                    if let ciborium::value::Value::Text(t) = v {
                        name = t;
                    }
                }
                2 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        prev = Some(i.try_into().unwrap_or(0i64));
                    }
                }
                3 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        asize = i.try_into().unwrap_or(0);
                    }
                }
                4 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        dsize = i.try_into().unwrap_or(0);
                    }
                }
                5 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        dev = i.try_into().unwrap_or(0);
                    }
                }
                6 => {
                    if let ciborium::value::Value::Bool(b) = v {
                        rderr = b;
                    }
                }
                12 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        sub = Some(i.try_into().unwrap_or(0u64));
                    }
                }
                13 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        ino = i.try_into().unwrap_or(0);
                    }
                }
                14 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        nlink = i.try_into().unwrap_or(1);
                    }
                }
                15 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        uid = Some(i.try_into().unwrap_or(0));
                    }
                }
                16 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        gid = Some(i.try_into().unwrap_or(0));
                    }
                }
                17 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        mode = Some(i.try_into().unwrap_or(0));
                    }
                }
                18 => {
                    if let ciborium::value::Value::Integer(i) = v {
                        mtime = Some(i.try_into().unwrap_or(0));
                    }
                }
                _ => {}
            }
        }

        let mut flags = if item_type == 1 {
            EntryFlags::IS_DIR
        } else {
            EntryFlags::empty()
        };
        if rderr {
            flags.insert(EntryFlags::READ_ERROR);
        }
        if nlink > 1 && item_type != 1 {
            flags.insert(EntryFlags::HARD_LINK);
        }

        let has_extended = uid.is_some() || gid.is_some() || mode.is_some() || mtime.is_some();
        let extended = if has_extended {
            Some(ExtendedInfo {
                mtime: mtime.unwrap_or(0),
                uid: uid.unwrap_or(0),
                gid: gid.unwrap_or(0),
                mode: mode.unwrap_or(0) as u32,
            })
        } else {
            None
        };

        let node = if item_type == 1 {
            TreeNode::new_dir(name, dev, ino, flags, extended)
        } else {
            TreeNode::new_file(name, asize, dsize, dev, ino, nlink, flags, extended)
        };

        let node_idx = node_list.len();
        node_list.push(node);
        id_map.insert((root_block_num, item_offset), node_idx);

        // Store sub link (first child)
        if let Some(sub_itemref) = sub {
            let child_block = (sub_itemref >> 24) as u32;
            let child_offset = (sub_itemref & 0xFFFFFF) as usize;
            parent_child_links.push((node_idx, child_block, child_offset));
        }

        // Store prev sibling link (relative offset)
        if let Some(rel_prev) = prev {
            let prev_offset = (item_offset as i64 + rel_prev) as usize;
            prev_sibling_links.push((node_idx, prev_offset));
        }
    }

    if node_list.is_empty() {
        return Err(anyhow!("No items decoded from binary stream"));
    }

    // Now construct the TreeArena
    // Find the root node index
    let root_node_idx = *id_map
        .get(&(root_block_num, root_offset))
        .ok_or_else(|| anyhow!("Root node not found at offset {}", root_offset))?;

    // Initialize arena with the root node
    let mut arena = TreeArena::new(node_list[root_node_idx].clone());

    // Map list indices to NodeId in the arena
    let mut idx_to_node_id = HashMap::new();
    idx_to_node_id.insert(root_node_idx, arena.root);

    // Add remaining nodes to arena in a topological/orderly fashion.
    // To do this simply, we can use a queue starting from the root.
    // We resolve links:
    // parent -> first_child (parent_child_links)
    // sibling -> prev_sibling (prev_sibling_links)
    // Actually, since all nodes are in `node_list`, let's wire them up directly:
    // Define parents and children lists for all nodes based on the links.
    let mut parents = vec![None; node_list.len()];
    let mut children_lists = vec![Vec::new(); node_list.len()];

    // 1. Build sibling chains using maps of node_idx to sibling node_idx
    let mut prev_sibling = vec![None; node_list.len()];
    let mut next_sibling = vec![None; node_list.len()];
    for (node_idx, prev_offset) in prev_sibling_links {
        if let Some(&prev_idx) = id_map.get(&(root_block_num, prev_offset)) {
            prev_sibling[node_idx] = Some(prev_idx);
            next_sibling[prev_idx] = Some(node_idx);
        }
    }

    // 2. Resolve parent-child (sub) links and propagate parent indices through sibling chains
    for (parent_idx, child_block, child_offset) in parent_child_links {
        if let Some(&child_idx) = id_map.get(&(child_block, child_offset)) {
            // Propagate parent_idx to the entire sibling group
            // First, propagate backward (prev)
            let mut curr = Some(child_idx);
            while let Some(idx) = curr {
                parents[idx] = Some(parent_idx);
                if !children_lists[parent_idx].contains(&idx) {
                    children_lists[parent_idx].push(idx);
                }
                curr = prev_sibling[idx];
            }
            // Next, propagate forward (next)
            let mut curr = next_sibling[child_idx];
            while let Some(idx) = curr {
                parents[idx] = Some(parent_idx);
                if !children_lists[parent_idx].contains(&idx) {
                    children_lists[parent_idx].push(idx);
                }
                curr = next_sibling[idx];
            }
        }
    }

    // Now let's recursively build the arena from root_node_idx
    let root_id = arena.root;
    build_arena_recursive(
        &mut arena,
        root_id,
        root_node_idx,
        &node_list,
        &children_lists,
    );

    // Recalculate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    Ok(arena)
}

fn build_arena_recursive(
    arena: &mut TreeArena,
    parent_id: NodeId,
    parent_idx: usize,
    node_list: &[TreeNode],
    children_lists: &[Vec<usize>],
) {
    let mut children_indices = children_lists[parent_idx].clone();

    // Sort children based on their order in the node_list/offsets to maintain order
    children_indices.sort();

    for child_idx in children_indices {
        let child_node = node_list[child_idx].clone();
        let child_id = arena.add_child(parent_id, child_node);
        build_arena_recursive(arena, child_id, child_idx, node_list, children_lists);
    }
}
