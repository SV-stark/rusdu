use crate::scan::filter::Filter;
use crate::scan::platform::get_metadata;
use crate::scan::{ProgressMode, ScanOptions, ScanStats, update_progress};
use crate::tree::{EntryFlags, NodeId, TreeArena, TreeNode};
use anyhow::Result;
use jwalk::{Parallelism, WalkDirGeneric};
use rustc_hash::FxHashMap;
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
    // Scanning a plain file previously succeeded with a bogus one-node tree
    // (and no error at all in parallel mode). Fail like the walker does.
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

    // Use a HashMap to map paths to NodeId in the arena
    let mut path_to_id = FxHashMap::default();
    path_to_id.insert(root_path.to_path_buf(), arena.root);

    // Build the WalkDir with the specified number of threads and sort by depth
    // to guarantee parent directories are added to arena/path_to_id before child items.
    let walk = WalkDirGeneric::<((), Option<NodeId>)>::new(root_path)
        .follow_links(opts.follow_symlinks)
        .parallelism(Parallelism::RayonNewPool(opts.threads))
        // jwalk defaults `skip_hidden` to true, which silently drops every
        // dotfile/dot-directory. The single-threaded walker reads them, so the
        // two backends disagreed on totals. Must be disabled for parity.
        .skip_hidden(false);

    // Collect entries and walk errors separately. Errors used to be discarded by
    // `filter_map(..ok())`, which turned unreadable directories into entries
    // that were later flagged EMPTY_DIR instead of READ_ERROR -- silent
    // under-reporting that the user cannot detect.
    let mut entries = Vec::new();
    let mut walk_errors: Vec<std::path::PathBuf> = Vec::new();
    for res in walk {
        match res {
            Ok(e) => entries.push(e),
            Err(e) => {
                if let Some(p) = e.path() {
                    walk_errors.push(p.to_path_buf());
                } else {
                    walk_errors.push(root_path.to_path_buf());
                }
            }
        }
    }

    entries.sort_by_key(|e| e.depth());

    for entry in entries {
        // Honour an interactive abort instead of running the whole scan to
        // completion in silence.
        if stats.aborted {
            break;
        }
        let path = entry.path();
        if path == root_path {
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

        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let is_dir_entry = entry.file_type.is_dir();

        // Exclude check
        if filter.is_kernfs_path(&path) {
            let child_node = if is_dir_entry {
                TreeNode::new_dir(
                    file_name,
                    arena.get(parent_id).dev,
                    0,
                    EntryFlags::KERNFS | EntryFlags::EXCLUDED,
                    None,
                )
            } else {
                TreeNode::new_file(
                    file_name,
                    0,
                    0,
                    arena.get(parent_id).dev,
                    0,
                    1,
                    EntryFlags::KERNFS | EntryFlags::EXCLUDED,
                    None,
                )
            };
            arena.add_child(parent_id, child_node);
            continue;
        }

        if filter.is_glob_match_relative(&path, Some(root_path)) {
            let child_node = if is_dir_entry {
                TreeNode::new_dir(
                    file_name,
                    arena.get(parent_id).dev,
                    0,
                    EntryFlags::EXCLUDED,
                    None,
                )
            } else {
                TreeNode::new_file(
                    file_name,
                    0,
                    0,
                    arena.get(parent_id).dev,
                    0,
                    1,
                    EntryFlags::EXCLUDED,
                    None,
                )
            };
            arena.add_child(parent_id, child_node);
            continue;
        }

        // `entry.file_type` is the type of the *target* once `follow_links` is set, so
        // `is_symlink()` is always false under `-L` and a symlinked directory was
        // registered as a real directory and recursed into -- duplicating the
        // whole subtree (unboundedly, on a junction). `path_is_symlink()`
        // reports whether the *path* is a link in both modes.
        let is_symlink = entry.path_is_symlink();
        let (meta, is_dir) = if is_symlink {
            if opts.follow_symlinks {
                match std::fs::metadata(&path) {
                    Ok(target_meta) => (target_meta, false),
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
                Ok(m) => (m, entry.file_type.is_dir()),
                Err(_) => continue,
            }
        };

        let plat = get_metadata(&path, &meta, opts.extended);

        // Cache dir check
        if is_dir && filter.has_cachedir_tag(&path) {
            let child_node = TreeNode::new_dir(
                file_name,
                plat.dev,
                plat.ino,
                EntryFlags::EXCLUDED,
                plat.extended,
            );
            arena.add_child(parent_id, child_node);
            continue;
        }

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

        if is_dir {
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
            if is_symlink || !entry.file_type.is_file() {
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

    // Mark directories that could not be read as READ_ERROR rather than letting
    // them fall through to EMPTY_DIR below (which would claim they are empty).
    for err_path in &walk_errors {
        if let Some(&parent_id) = path_to_id.get(err_path) {
            arena
                .get_mut(parent_id)
                .flags
                .insert(EntryFlags::READ_ERROR);
        }
    }

    if !walk_errors.is_empty() {
        eprintln!(
            "Warning: {} director{} could not be read",
            walk_errors.len(),
            if walk_errors.len() == 1 { "y" } else { "ies" }
        );
    }

    for i in 0..arena.nodes.len() {
        if arena.nodes[i].is_dir()
            && arena.nodes[i].children.is_empty()
            && !arena.nodes[i].flags.contains(EntryFlags::READ_ERROR)
            && !arena.nodes[i].flags.contains(EntryFlags::EXCLUDED)
        {
            arena.nodes[i].flags.insert(EntryFlags::EMPTY_DIR);
        }
    }

    // Recalculate stats bottom-up
    crate::tree::stats::recalculate_stats(&mut arena);

    if progress_mode == ProgressMode::Line {
        eprintln!("\nScan complete. Scanned {} items.", stats.items_scanned);
    }

    // A partial tree looks complete and would be silently wrong, so surface
    // the abort instead of returning it as a normal result.
    if stats.aborted {
        anyhow::bail!("Scan aborted by user");
    }

    Ok(arena)
}
