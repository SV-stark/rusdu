use crate::tree::{EntryFlags, ExtendedInfo, NodeId, TreeArena, TreeNode};
use anyhow::{Result, anyhow};
use rustc_hash::FxHashMap;

pub fn import_bin(file_bytes: &[u8]) -> Result<TreeArena> {
    if file_bytes.len() < 8 || &file_bytes[0..8] != b"\xbfncduEX1" {
        return Err(anyhow!("Invalid binary file signature"));
    }

    // Parse blocks from start to find index block at the end
    let mut offset = 8;
    let mut data_blocks = FxHashMap::default();
    let mut index_content = None;

    while offset < file_bytes.len() {
        if offset + 8 > file_bytes.len() {
            break;
        }

        let typelen = u32::from_be_bytes(file_bytes[offset..offset + 4].try_into()?);
        let block_type = (typelen >> 28) & 0xF;
        let block_len = typelen & 0x0FFFFFFF;

        if block_len < 8 || offset + block_len as usize > file_bytes.len() {
            return Err(anyhow!("Malformed block length"));
        }

        // Validate footer TypeLen
        let footer_offset = offset + block_len as usize - 4;
        let footer_typelen =
            u32::from_be_bytes(file_bytes[footer_offset..footer_offset + 4].try_into()?);
        if footer_typelen != typelen {
            return Err(anyhow!("Block header and footer TypeLen mismatch"));
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

    // Decode all items across all data blocks
    let mut id_map = FxHashMap::default();
    let mut node_list = Vec::new();
    let mut parent_child_links = Vec::new();
    let mut prev_sibling_links = Vec::new();

    // Sort block numbers for deterministic decoding order
    let mut block_nums: Vec<u32> = data_blocks.keys().cloned().collect();
    block_nums.sort_unstable();

    for block_num in block_nums {
        let block_data = &data_blocks[&block_num];
        let mut cursor = CborCursor::new(block_data);

        while !cursor.is_eof() {
            let item_offset = cursor.pos;

            let map_len = match cursor.read_map_len() {
                Ok(l) => l,
                Err(_) => break, // Reached end of data in block
            };

            // Parse fields according to exact ncdu 2.x specification
            let mut item_type = 0i64;
            let mut name = String::new();
            let mut prev_sibling_target = None;
            let mut asize = 0i64;
            let mut dsize = 0i64;
            let mut dev = 0u64;
            let mut rderr_self = false;
            let mut rderr_sub = false;
            let mut sub_child = None;
            let mut ino = 0u64;
            let mut nlink = 1u32;
            let mut uid = None;
            let mut gid = None;
            let mut mode = None;
            let mut mtime = None;

            for _ in 0..map_len {
                let key = cursor.read_int()?;
                match key {
                    0 => item_type = cursor.read_int()?,
                    1 => name = cursor.read_text()?.to_string(),
                    2 => {
                        let val = cursor.read_int()?;
                        if val < 0 {
                            let prev_offset = (item_offset as i64 + val) as usize;
                            prev_sibling_target = Some((block_num, prev_offset));
                        } else {
                            let u = val as u64;
                            let prev_block = (u >> 24) as u32;
                            let prev_offset = (u & 0xFFFFFF) as usize;
                            prev_sibling_target = Some((prev_block, prev_offset));
                        }
                    }
                    3 => asize = cursor.read_int()?,
                    4 => dsize = cursor.read_int()?,
                    5 => dev = cursor.read_int()? as u64,
                    6 => {
                        let b = cursor.read_bool()?;
                        if b {
                            rderr_self = true;
                        } else {
                            rderr_sub = true;
                        }
                    }
                    7 => {
                        let _cumasize = cursor.read_int()?;
                    }
                    8 => {
                        let _cumdsize = cursor.read_int()?;
                    }
                    9 => {
                        let _shrasize = cursor.read_int()?;
                    }
                    10 => {
                        let _shrdsize = cursor.read_int()?;
                    }
                    11 => {
                        let _items = cursor.read_int()?;
                    }
                    12 => {
                        let val = cursor.read_int()?;
                        if val < 0 {
                            let abs_offset = (item_offset as i64 + val) as usize;
                            sub_child = Some((block_num, abs_offset));
                        } else {
                            let u = val as u64;
                            let child_block = (u >> 24) as u32;
                            let child_offset = (u & 0xFFFFFF) as usize;
                            sub_child = Some((child_block, child_offset));
                        }
                    }
                    13 => ino = cursor.read_int()? as u64,
                    14 => nlink = cursor.read_int()? as u32,
                    15 => uid = Some(cursor.read_int()? as u32),
                    16 => gid = Some(cursor.read_int()? as u32),
                    17 => mode = Some(cursor.read_int()? as u32),
                    18 => mtime = Some(cursor.read_int()?),
                    _ => cursor.skip_value()?,
                }
            }

            let mut flags = match item_type {
                1 => EntryFlags::IS_DIR,
                2 => EntryFlags::NOT_REG,
                3 => EntryFlags::HARD_LINK,
                -1 => EntryFlags::READ_ERROR,
                -2 => EntryFlags::EXCLUDED,
                -3 => EntryFlags::OTHER_FS,
                -4 => EntryFlags::KERNFS | EntryFlags::EXCLUDED,
                _ => EntryFlags::empty(),
            };

            if rderr_self {
                flags.insert(EntryFlags::READ_ERROR);
            }
            if rderr_sub {
                flags.insert(EntryFlags::SUB_ERROR);
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
                    mode: mode.unwrap_or(0),
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
            id_map.insert((block_num, item_offset), node_idx);

            // Store sub link (last child)
            if let Some((child_block, child_offset)) = sub_child {
                parent_child_links.push((node_idx, child_block, child_offset));
            }

            // Store prev sibling link
            if let Some((prev_block, prev_offset)) = prev_sibling_target {
                prev_sibling_links.push((node_idx, prev_block, prev_offset));
            }
        }
    }

    if node_list.is_empty() {
        return Err(anyhow!("No items decoded from binary stream"));
    }

    // Find the root node index
    let root_node_idx = *id_map
        .get(&(root_block_num, root_offset))
        .ok_or_else(|| anyhow!("Root node not found at offset {}", root_offset))?;

    // Initialize arena with the root node
    let mut arena = TreeArena::new(std::mem::take(&mut node_list[root_node_idx]));

    let mut children_lists = vec![Vec::new(); node_list.len()];

    // 1. Build sibling chains
    let mut prev_sibling = vec![None; node_list.len()];
    for (node_idx, block_num, prev_offset) in prev_sibling_links {
        if let Some(&prev_idx) = id_map.get(&(block_num, prev_offset)) {
            prev_sibling[node_idx] = Some(prev_idx);
        }
    }

    // 2. Resolve parent-child (sub) links: sub points to the LAST child.
    // Walking prev_sibling backward from last_child visits all siblings right-to-left.
    // Reversing the list gives the natural left-to-right order.
    for (parent_idx, child_block, child_offset) in parent_child_links {
        if let Some(&last_child_idx) = id_map.get(&(child_block, child_offset)) {
            let mut list = Vec::new();
            let mut curr = Some(last_child_idx);
            while let Some(idx) = curr {
                list.push(idx);
                curr = prev_sibling[idx];
            }
            list.reverse();
            children_lists[parent_idx] = list;
        }
    }

    // Now let's recursively build the arena from root_node_idx
    let root_id = arena.root;
    build_arena_recursive(
        &mut arena,
        root_id,
        root_node_idx,
        &mut node_list,
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
    node_list: &mut [TreeNode],
    children_lists: &[Vec<usize>],
) {
    let children_indices = &children_lists[parent_idx];

    for &child_idx in children_indices {
        let child_node = std::mem::take(&mut node_list[child_idx]);
        let child_id = arena.add_child(parent_id, child_node);
        build_arena_recursive(arena, child_id, child_idx, node_list, children_lists);
    }
}

struct CborCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> CborCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_byte(&mut self) -> Result<u8> {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            Ok(b)
        } else {
            Err(anyhow!("Unexpected EOF reading CBOR"))
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.pos + len <= self.data.len() {
            let slice = &self.data[self.pos..self.pos + len];
            self.pos += len;
            Ok(slice)
        } else {
            Err(anyhow!("Unexpected EOF reading CBOR bytes"))
        }
    }

    fn read_uint_val(&mut self, info: u8) -> Result<u64> {
        match info {
            0..=23 => Ok(info as u64),
            24 => Ok(self.read_byte()? as u64),
            25 => {
                let bytes = self.read_exact(2)?;
                Ok(u16::from_be_bytes(bytes.try_into()?) as u64)
            }
            26 => {
                let bytes = self.read_exact(4)?;
                Ok(u32::from_be_bytes(bytes.try_into()?) as u64)
            }
            27 => {
                let bytes = self.read_exact(8)?;
                Ok(u64::from_be_bytes(bytes.try_into()?))
            }
            _ => Err(anyhow!("Invalid CBOR integer additional info: {}", info)),
        }
    }

    fn read_int(&mut self) -> Result<i64> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        match major {
            0 => {
                let u = self.read_uint_val(info)?;
                if u > (i64::MAX as u64) {
                    return Err(anyhow!("CBOR positive integer exceeds i64::MAX"));
                }
                Ok(u as i64)
            }
            1 => {
                let u = self.read_uint_val(info)?;
                if u > (i64::MAX as u64) {
                    return Err(anyhow!("CBOR negative integer exceeds i64::MIN"));
                }
                Ok(-1 - (u as i64))
            }
            _ => Err(anyhow!("Expected CBOR integer, got major type {}", major)),
        }
    }

    fn read_text(&mut self) -> Result<&'a str> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        if major != 3 {
            return Err(anyhow!(
                "Expected CBOR text string, got major type {}",
                major
            ));
        }
        let len = self.read_uint_val(info)? as usize;
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes).map_err(|e| anyhow!("Invalid UTF-8 in CBOR string: {}", e))
    }

    fn read_bool(&mut self) -> Result<bool> {
        let b = self.read_byte()?;
        match b {
            0xF4 => Ok(false),
            0xF5 => Ok(true),
            _ => Err(anyhow!("Expected CBOR bool, got 0x{:02x}", b)),
        }
    }

    fn read_map_len(&mut self) -> Result<usize> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        if major != 5 {
            return Err(anyhow!("Expected CBOR map, got major type {}", major));
        }
        Ok(self.read_uint_val(info)? as usize)
    }

    fn skip_value(&mut self) -> Result<()> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        match major {
            0 | 1 | 7 => {
                let _ = self.read_uint_val(info)?;
                Ok(())
            }
            2 | 3 => {
                let len = self.read_uint_val(info)? as usize;
                let _ = self.read_exact(len)?;
                Ok(())
            }
            4 => {
                let len = self.read_uint_val(info)? as usize;
                for _ in 0..len {
                    self.skip_value()?;
                }
                Ok(())
            }
            5 => {
                let len = self.read_uint_val(info)? as usize;
                for _ in 0..len {
                    self.skip_value()?;
                    self.skip_value()?;
                }
                Ok(())
            }
            6 => {
                let _ = self.read_uint_val(info)?;
                self.skip_value()
            }
            _ => Err(anyhow!("Unsupported CBOR major type: {}", major)),
        }
    }
}
