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
    let mut seen_links = HashSet::new();
    let root = arena.root;
    aggregate_node_stats(arena, root, &mut seen_links);
}

fn aggregate_node_stats(
    arena: &mut TreeArena,
    node_id: NodeId,
    seen_links: &mut HashSet<(u64, u64)>,
) -> AggregateStats {
    // If it's a file, calculate and return its basic stats
    let is_dir = arena.get(node_id).is_dir();
    if !is_dir {
        let node = arena.get(node_id);
        let is_hard_link = node.flags.contains(EntryFlags::HARD_LINK);
        let link_key = (node.dev, node.ino);

        let (asize, dsize) = if is_hard_link {
            if seen_links.contains(&link_key) {
                // Already counted, don't count towards normal sizes
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
        arena.get_mut(node_id).stats = stats.clone();
        return stats;
    }

    // It's a directory
    let children = arena.get(node_id).children.clone();
    let mut stats = AggregateStats::default();
    stats.dir_count = 1; // Count itself as a directory

    for child_id in children {
        let child_stats = aggregate_node_stats(arena, child_id, seen_links);

        stats.total_asize += child_stats.total_asize;
        stats.total_dsize += child_stats.total_dsize;
        stats.item_count += child_stats.item_count;
        stats.dir_count += child_stats.dir_count;
        stats.file_count += child_stats.file_count;
        stats.latest_mtime = stats.latest_mtime.max(child_stats.latest_mtime);
        stats.shared_size += child_stats.shared_size;
    }

    // Include directory's own apparent/disk size if any (usually directories consume a block on disk)
    let node = arena.get(node_id);
    stats.total_asize += node.asize;
    stats.total_dsize += node.dsize;
    stats.item_count += 1; // Include itself in item count

    let own_mtime = node.extended.as_ref().map(|e| e.mtime).unwrap_or(0);
    stats.latest_mtime = stats.latest_mtime.max(own_mtime);

    arena.get_mut(node_id).stats = stats.clone();
    stats
}
