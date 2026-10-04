use crate::scan::filter::Filter;
use crate::scan::platform::get_metadata;
use crate::scan::{ProgressMode, ScanOptions, ScanStats, update_progress};
use crate::tree::{EntryFlags, NodeId, TreeArena, TreeNode};
use anyhow::Result;
use std::fs;
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
    // Scanning a plain file used to return a bogus one-node tree flagged as a
    // read error, and exit 0.
    if !root_meta.is_dir() {
        anyhow::bail!("{} is not a directory", root_path.display());
    }
    let root_plat = get_metadata(root_path, &root_meta, opts.extended);

    let root_node = TreeNode::new_dir(
        root_path.to_string_lossy().into_owned(),
        root_plat.dev,
        root_plat.ino,
        EntryFlags::empty(),
        root_plat.extended,
    );

    let mut arena = TreeArena::new(root_node);
    let mut stats = ScanStats {
        update_interval_ms: opts.update_interval_ms,
        ..Default::default()
    };

    let root_id = arena.root;
    walk_dir_recursive(
        &mut arena,
        root_id,
        root_path,
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

    // `aborted` was set but never read, so an interrupted scan returned a
    // truncated tree that looked complete.
    if stats.aborted {
        anyhow::bail!("Scan aborted by user");
    }

    Ok(arena)
}

#[allow(clippy::too_many_arguments)]
fn walk_dir_recursive(
    arena: &mut TreeArena,
    parent_id: NodeId,
    dir_path: &Path,
    root_path: &Path,
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
    for e in entries.flatten() {
        if opts.exclude_caches && e.file_name() == "CACHEDIR.TAG" {
            tag_path = Some(e.path());
        }
        collected.push(e);
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
        let file_name = entry.file_name().to_string_lossy().into_owned();

        let is_dir_entry = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        // Exclude check
        if filter.is_kernfs_path(&path) {
            let child = if is_dir_entry {
                TreeNode::new_dir(
                    file_name,
                    parent_dev,
                    0,
                    EntryFlags::KERNFS | EntryFlags::EXCLUDED,
                    None,
                )
            } else {
                TreeNode::new_file(
                    file_name,
                    0,
                    0,
                    parent_dev,
                    0,
                    1,
                    EntryFlags::KERNFS | EntryFlags::EXCLUDED,
                    None,
                )
            };
            arena.add_child(parent_id, child);
            continue;
        }

        if stats.aborted {
            break;
        }

        if filter.is_glob_match_relative(&path, Some(root_path)) {
            let child = if is_dir_entry {
                TreeNode::new_dir(file_name, parent_dev, 0, EntryFlags::EXCLUDED, None)
            } else {
                TreeNode::new_file(
                    file_name,
                    0,
                    0,
                    parent_dev,
                    0,
                    1,
                    EntryFlags::EXCLUDED,
                    None,
                )
            };
            arena.add_child(parent_id, child);
            continue;
        }

        let is_symlink = match entry.file_type() {
            Ok(ft) => ft.is_symlink(),
            Err(_) => false,
        };

        let (meta, is_dir_to_recurse) = if is_symlink {
            if opts.follow_symlinks {
                match fs::metadata(&path) {
                    Ok(target_meta) => {
                        // ncdu follows symlinks to files only, NEVER directories
                        (target_meta, false)
                    }
                    Err(_) => match entry.metadata() {
                        Ok(m) => (m, false),
                        Err(_) => continue,
                    },
                }
            } else {
                match entry.metadata() {
                    Ok(m) => (m, false),
                    Err(_) => continue,
                }
            }
        } else {
            match entry.metadata() {
                Ok(m) => {
                    let is_dir = m.is_dir();
                    (m, is_dir)
                }
                Err(_) => continue,
            }
        };

        let plat = get_metadata(&path, &meta, opts.extended);

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

        if is_dir_to_recurse {
            let child_node = TreeNode::new_dir(
                file_name,
                plat.dev,
                plat.ino,
                EntryFlags::empty(),
                plat.extended,
            );
            let child_id = arena.add_child(parent_id, child_node);

            if walk_dir_recursive(
                arena,
                child_id,
                &path,
                root_path,
                opts,
                filter,
                progress_mode,
                stats,
            )
            .is_err()
            {
                arena.get_mut(child_id).flags.insert(EntryFlags::READ_ERROR);
            }
        } else {
            let mut flags = EntryFlags::empty();
            if is_symlink || !meta.is_file() {
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

    // Guard on `aborted`: an interrupted scan leaves this directory partially
    // populated, and marking it EMPTY_DIR would claim it is legitimately empty.
    if !stats.aborted && arena.get(parent_id).children.is_empty() {
        arena.get_mut(parent_id).flags.insert(EntryFlags::EMPTY_DIR);
    }

    Ok(())
}
