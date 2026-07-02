use crate::scan::filter::Filter;
use crate::scan::platform::get_metadata;
use crate::scan::{ProgressMode, ScanOptions, ScanStats, update_progress};
use crate::tree::{EntryFlags, NodeId, TreeArena, TreeNode};
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn scan_single_threaded(
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

    // Start UI/Console updates
    if progress_mode == ProgressMode::Line {
        eprintln!("Scanning {:?}", root_path);
    }

    let root_meta = fs::symlink_metadata(root_path)?;
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

    let root_id = arena.root;
    walk_dir_recursive(
        &mut arena,
        root_id,
        root_path,
        &opts,
        &filter,
        progress_mode,
        &mut stats,
    )?;

    // Aggregate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    if progress_mode == ProgressMode::Line {
        eprintln!("\nScan complete. Scanned {} items.", stats.items_scanned);
    }

    Ok(arena)
}

fn walk_dir_recursive(
    arena: &mut TreeArena,
    parent_id: NodeId,
    dir_path: &Path,
    opts: &ScanOptions,
    filter: &Filter,
    progress_mode: ProgressMode,
    stats: &mut ScanStats,
) -> Result<()> {
    let entries = match fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => {
            arena
                .get_mut(parent_id)
                .flags
                .insert(EntryFlags::READ_ERROR);
            return Ok(());
        }
    };

    let mut collected = Vec::new();
    let mut tag_path = None;
    for entry in entries {
        if let Ok(e) = entry {
            if opts.exclude_caches && e.file_name() == "CACHEDIR.TAG" {
                tag_path = Some(e.path());
            }
            collected.push(e);
        }
    }

    if let Some(path) = tag_path {
        if filter.verify_cachedir_tag(&path) {
            arena.get_mut(parent_id).flags.insert(EntryFlags::EXCLUDED);
            return Ok(());
        }
    }

    let parent_dev = arena.get(parent_id).dev;

    for entry in collected {
        let path = entry.path();

        // Exclude check
        if filter.should_exclude_path(&path) {
            continue;
        }

        let meta = if opts.follow_symlinks {
            match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                },
            }
        } else {
            match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            }
        };

        let file_name = entry.file_name().to_string_lossy().into_owned();
        let plat = get_metadata(&meta, opts.extended);

        // Check filesystem boundary
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

        // Update progress UI if time elapsed
        update_progress(&path, stats, progress_mode);

        if meta.is_dir() {
            let child_node = TreeNode::new_dir(
                file_name,
                plat.dev,
                plat.ino,
                EntryFlags::empty(),
                plat.extended,
            );
            let child_id = arena.add_child(parent_id, child_node);

            if let Err(_) =
                walk_dir_recursive(arena, child_id, &path, opts, filter, progress_mode, stats)
            {
                arena.get_mut(child_id).flags.insert(EntryFlags::READ_ERROR);
            }
        } else {
            let mut flags = EntryFlags::empty();
            if !meta.is_file() {
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

    Ok(())
}
