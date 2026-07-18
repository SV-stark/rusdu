use crate::tree::{EntryFlags, NodeId, TreeArena};
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct AggregateStats {
    pub total_asize: i64,
    pub total_dsize: i64,
    pub item_count: u32,
    pub dir_count: u32,
    pub file_count: u32,
    pub latest_mtime: i64,
    pub shared_size: i64,
}

pub fn recalculate_stats(arena: &mut TreeArena) {
    let mut post_order = Vec::new();
    let root = arena.root;
    get_post_order(arena, root, &mut post_order);

    let mut seen_links = HashSet::new();

    for &node_id in &post_order {
        let is_dir = arena.get(node_id).is_dir();
        if !is_dir {
            // It's a file
            let node = arena.get(node_id);
            let is_hard_link = node.flags.contains(EntryFlags::HARD_LINK);
            let link_key = (node.dev, node.ino);

            let (asize, dsize) = if is_hard_link {
                if seen_links.contains(&link_key) {
                    (0, 0)
                } else {
                    seen_links.insert(link_key);
                    (node.asize, node.dsize)
                }
            } else {
                (node.asize, node.dsize)
            };

            let mtime = node.extended.as_ref().map(|e| e.mtime).unwrap_or(0);

            let stats = AggregateStats {
                total_asize: asize,
                total_dsize: dsize,
                item_count: 1,
                dir_count: 0,
                file_count: 1,
                latest_mtime: mtime,
                shared_size: if is_hard_link { node.dsize } else { 0 },
            };
            arena.get_mut(node_id).stats = stats;
        } else {
            // It's a directory. Its children stats are already calculated!
            let mut stats = AggregateStats {
                dir_count: 1,
                ..Default::default()
            };

            let node = arena.get(node_id);
            let children = &node.children;

            for &child_id in children {
                let child_stats = &arena.get(child_id).stats;
                stats.total_asize += child_stats.total_asize;
                stats.total_dsize += child_stats.total_dsize;
                stats.item_count += child_stats.item_count;
                stats.dir_count += child_stats.dir_count;
                stats.file_count += child_stats.file_count;
                stats.latest_mtime = stats.latest_mtime.max(child_stats.latest_mtime);
                stats.shared_size += child_stats.shared_size;
            }

            // Include directory's own apparent/disk size if any
            let node = arena.get(node_id);
            stats.total_asize += node.asize;
            stats.total_dsize += node.dsize;
            stats.item_count += 1;

            let own_mtime = node.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
            stats.latest_mtime = stats.latest_mtime.max(own_mtime);

            arena.get_mut(node_id).stats = stats;
        }
    }
}

fn get_post_order(arena: &TreeArena, root: NodeId, post_order: &mut Vec<NodeId>) {
    let mut stack = vec![(root, 0)];

    while let Some((node_id, child_idx)) = stack.last_mut() {
        let node = arena.get(*node_id);
        if *child_idx < node.children.len() {
            let next_child = node.children[*child_idx];
            *child_idx += 1;
            stack.push((next_child, 0));
        } else {
            post_order.push(*node_id);
            stack.pop();
        }
    }
}
