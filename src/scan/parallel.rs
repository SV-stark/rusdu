use crate::scan::filter::Filter;
use crate::scan::platform::get_metadata;
use crate::scan::{ProgressMode, ScanOptions, ScanStats, update_progress};
use crate::tree::{EntryFlags, NodeId, TreeArena, TreeNode};
use anyhow::Result;
use jwalk::{Parallelism, WalkDirGeneric};
use std::collections::HashMap;
use std::path::Path;

pub fn scan_parallel(
    root_path: &Path,
    opts: ScanOptions,
    progress_mode: ProgressMode,
) -> Result<TreeArena> {
    let filter = Filter::new(
        &opts.exclude_patterns,
        opts.exclude_from.as_deref(),
        opts.exclude_caches,
        opts.exclude_kernfs,
    )?;

    if progress_mode == ProgressMode::Line {
        eprintln!(
            "Scanning parallelly ({} threads) {:?}",
            opts.threads, root_path
        );
    }

    let root_meta = std::fs::symlink_metadata(root_path)?;
    let root_plat = get_metadata(&root_meta, opts.extended);

    let root_node = TreeNode::new_dir(
        root_path.to_string_lossy().into_owned(),
        root_plat.dev,
        root_plat.ino,
        EntryFlags::empty(),
        root_plat.extended,
    );

    let mut arena = TreeArena::new(root_node);
    let mut stats = ScanStats::default();

    // Use a HashMap to map paths to NodeId in the arena
    let mut path_to_id = HashMap::new();
    path_to_id.insert(root_path.to_path_buf(), arena.root);

    // Build the WalkDir with the specified number of threads
    let walker = WalkDirGeneric::<((), Option<NodeId>)>::new(root_path)
        .follow_links(opts.follow_symlinks)
        .parallelism(Parallelism::RayonNewPool(opts.threads));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path == root_path {
            continue;
        }

        if filter.should_exclude_path(&path) {
            continue;
        }

        let parent_path = match path.parent() {
            Some(p) => p,
            None => continue,
        };

        let parent_id = match path_to_id.get(parent_path) {
            Some(&id) => id,
            None => continue, // Parent was not added/processed or was excluded
        };

        // Cache dir check
        if entry.file_type.is_dir() && filter.has_cachedir_tag(&path) {
            arena.get_mut(parent_id).flags.insert(EntryFlags::EXCLUDED);
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let plat = get_metadata(&meta, opts.extended);

        // Check filesystem boundary
        let parent_dev = arena.get(parent_id).dev;
        if opts.one_file_system && plat.dev != parent_dev {
            let child = TreeNode::new_dir(
                file_name,
                plat.dev,
                plat.ino,
                EntryFlags::OTHER_FS,
                plat.extended,
            );
            arena.add_child(parent_id, child);
            continue;
        }

        stats.items_scanned += 1;
        stats.size_scanned += plat.dsize;
        update_progress(&path, &mut stats, progress_mode);

        if entry.file_type.is_dir() {
            let child_node = TreeNode::new_dir(
                file_name,
                plat.dev,
                plat.ino,
                EntryFlags::empty(),
                plat.extended,
            );
            let child_id = arena.add_child(parent_id, child_node);
            path_to_id.insert(path.clone(), child_id);
        } else {
            let mut flags = EntryFlags::empty();
            if !entry.file_type.is_file() {
                flags.insert(EntryFlags::NOT_REG);
            }
            if plat.nlink > 1 {
                flags.insert(EntryFlags::HARD_LINK);
            }

            let child_node = TreeNode::new_file(
                file_name,
                plat.asize,
                plat.dsize,
                plat.dev,
                plat.ino,
                plat.nlink,
                flags,
                plat.extended,
            );
            arena.add_child(parent_id, child_node);
        }
    }

    // Recalculate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    if progress_mode == ProgressMode::Line {
        eprintln!("\nScan complete. Scanned {} items.", stats.items_scanned);
    }

    Ok(arena)
}
