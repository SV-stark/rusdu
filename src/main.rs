mod cli;
mod config;
mod delete;
mod export;
mod format;
mod scan;
mod shell;
mod tree;
mod ui;

use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = match cli::Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Load configuration files unless ignored
    if !args.ignore_config {
        if let Err(e) = config::load_config(&mut args) {
            eprintln!("Warning: failed to load configuration: {}", e);
        }
    }

    // Initialize logging
    if let Some(ref log_path) = args.log_file {
        match std::fs::File::create(log_path) {
            Ok(file) => {
                let mut builder = env_logger::Builder::new();
                builder.target(env_logger::Target::Pipe(Box::new(file)));
                builder.filter_level(log::LevelFilter::Info);
                builder.init();
            }
            Err(e) => {
                eprintln!("Warning: failed to create log file {:?}: {}", log_path, e);
                env_logger::init();
            }
        }
    } else {
        env_logger::init();
    }

    // Determine target directory / path
    let scan_path = args.path.clone().unwrap_or_else(|| PathBuf::from("."));

    // If an import file was specified
    let arena = if let Some(ref import_path) = args.import_file {
        if import_path == std::path::Path::new("-") {
            eprintln!("Importing from stdin...");
        } else {
            eprintln!("Importing from {}...", import_path.display());
        }
        export::import_file(import_path)?
    } else {
        // Otherwise scan the directory
        let progress_mode = if args.silent {
            scan::ProgressMode::Silent
        } else if args.line_progress {
            scan::ProgressMode::Line
        } else if args.fullscreen_progress {
            scan::ProgressMode::Fullscreen
        } else if args.export_json.is_some() || args.export_bin.is_some() {
            let is_stdout = args
                .export_json
                .as_ref()
                .is_some_and(|p| p == std::path::Path::new("-"))
                || args
                    .export_bin
                    .as_ref()
                    .is_some_and(|p| p == std::path::Path::new("-"));
            if is_stdout {
                scan::ProgressMode::Silent
            } else {
                scan::ProgressMode::Line
            }
        } else {
            scan::ProgressMode::Fullscreen
        };

        let scan_opts = scan::ScanOptions::from_args(&args);
        scan::scan_directory(&scan_path, scan_opts, progress_mode)?
    };

    // If exporting, perform export and exit
    if let Some(ref export_json_path) = args.export_json {
        let compress = args.compress;
        let compress_level = args.compress_level.unwrap_or(scan::DEFAULT_COMPRESS_LEVEL);
        export::export_json(&arena, export_json_path, compress, compress_level)?;
        return Ok(());
    }

    if let Some(ref export_bin_path) = args.export_bin {
        let block_size = args
            .export_block_size
            .unwrap_or(scan::DEFAULT_BLOCK_SIZE_KB);
        let compress_level = args.compress_level.unwrap_or(scan::DEFAULT_COMPRESS_LEVEL);
        export::export_bin(&arena, export_bin_path, block_size, compress_level)?;
        return Ok(());
    }

    // Start interactive TUI browser
    ui::run_tui(arena, args)?;

    Ok(())
}
