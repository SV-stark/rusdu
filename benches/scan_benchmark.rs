use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rusdu::scan::{ProgressMode, ScanOptions, scan_directory};
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

fn generate_test_directory(total_files: usize, depth: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir for benchmark");
    let base_path = temp_dir.path();

    let mut current_dir = base_path.to_path_buf();
    for d in 0..depth {
        current_dir = current_dir.join(format!("subdir_{}", d));
        fs::create_dir_all(&current_dir).unwrap();
    }

    let files_per_dir = (total_files / (depth + 1)).max(1);

    // Create files at root and inside each subdirectory
    let mut dirs_to_populate = vec![base_path.to_path_buf()];
    let mut curr = base_path.to_path_buf();
    for d in 0..depth {
        curr = curr.join(format!("subdir_{}", d));
        dirs_to_populate.push(curr.clone());
    }

    let mut file_counter = 0;
    for dir in &dirs_to_populate {
        for i in 0..files_per_dir {
            file_counter += 1;
            let file_path = dir.join(format!("file_{}_{}.dat", i, if i % 5 == 0 { "tmp" } else { "txt" }));
            let mut f = File::create(&file_path).unwrap();
            let dummy_data = vec![(file_counter % 255) as u8; 256];
            f.write_all(&dummy_data).unwrap();
        }
    }

    temp_dir
}

fn bench_scan_directory(c: &mut Criterion) {
    let temp_dir = generate_test_directory(200, 3);
    let target_path = temp_dir.path();

    let mut group = c.benchmark_group("directory_scanning");

    group.bench_function("single_thread_200_files", |b| {
        b.iter(|| {
            let opts = ScanOptions {
                one_file_system: false,
                exclude_patterns: Vec::new(),
                exclude_from: None,
                exclude_caches: false,
                exclude_kernfs: false,
                follow_symlinks: false,
                threads: 1,
                extended: false,
            };
            let arena = scan_directory(black_box(target_path), opts, ProgressMode::Silent)
                .expect("Scan failed");
            black_box(arena);
        });
    });

    group.bench_function("multi_thread_200_files", |b| {
        b.iter(|| {
            let opts = ScanOptions {
                one_file_system: false,
                exclude_patterns: Vec::new(),
                exclude_from: None,
                exclude_caches: false,
                exclude_kernfs: false,
                follow_symlinks: false,
                threads: 4,
                extended: false,
            };
            let arena = scan_directory(black_box(target_path), opts, ProgressMode::Silent)
                .expect("Scan failed");
            black_box(arena);
        });
    });

    group.bench_function("scan_with_glob_exclusions", |b| {
        b.iter(|| {
            let opts = ScanOptions {
                one_file_system: false,
                exclude_patterns: vec!["*.tmp".to_string()],
                exclude_from: None,
                exclude_caches: false,
                exclude_kernfs: false,
                follow_symlinks: false,
                threads: 2,
                extended: false,
            };
            let arena = scan_directory(black_box(target_path), opts, ProgressMode::Silent)
                .expect("Scan failed");
            black_box(arena);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_scan_directory);
criterion_main!(benches);
