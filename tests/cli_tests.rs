use rusdu::cli::{Args, CliError};
use std::path::PathBuf;

#[test]
fn test_default_args() {
    let args = Args::default();
    assert!(args.path.is_none());
    assert!(!args.extended);
    assert!(!args.one_file_system);
    assert!(!args.silent);
    assert_eq!(args.read_only, 0);
    assert_eq!(args.color, "off");
    assert_eq!(args.graph_style, "hash");
    assert_eq!(args.sort, "disk-usage");
}

#[test]
fn test_short_and_long_flags() {
    // Short flags
    let parsed = Args::try_parse_from(["rusdu", "-e", "-x", "-0", "-L", "-c"]).unwrap();
    assert!(parsed.extended);
    assert!(parsed.one_file_system);
    assert!(!parsed.cross_file_system);
    assert!(parsed.silent);
    assert!(parsed.follow_symlinks);
    assert!(parsed.compress);

    // Long flags equivalent
    let parsed_long = Args::try_parse_from([
        "rusdu",
        "--extended",
        "--one-file-system",
        "--silent",
        "--follow-symlinks",
        "--compress",
    ])
    .unwrap();
    assert!(parsed_long.extended);
    assert!(parsed_long.one_file_system);
    assert!(parsed_long.silent);
    assert!(parsed_long.follow_symlinks);
    assert!(parsed_long.compress);
}

#[test]
fn test_flag_precedence_and_overrides() {
    // one-file-system then cross-file-system
    let args = Args::try_parse_from(["rusdu", "--one-file-system", "--cross-file-system"]).unwrap();
    assert!(args.cross_file_system);
    assert!(!args.one_file_system);

    // cross-file-system then one-file-system
    let args2 = Args::try_parse_from(["rusdu", "--cross-file-system", "--one-file-system"]).unwrap();
    assert!(args2.one_file_system);
    assert!(!args2.cross_file_system);

    // natsort override
    let args3 = Args::try_parse_from(["rusdu", "--disable-natsort", "--enable-natsort"]).unwrap();
    assert!(args3.enable_natsort);
    assert!(!args3.disable_natsort);

    // confirm quit override
    let args4 = Args::try_parse_from(["rusdu", "--confirm-quit", "--no-confirm-quit"]).unwrap();
    assert!(args4.no_confirm_quit);
    assert!(!args4.confirm_quit);
}

#[test]
fn test_multi_occurrence_exclude() {
    let args = Args::try_parse_from([
        "rusdu",
        "--exclude",
        "*.tmp",
        "--exclude",
        "node_modules",
        "--exclude",
        ".git",
    ])
    .unwrap();

    assert_eq!(args.exclude, vec!["*.tmp", "node_modules", ".git"]);
}

#[test]
fn test_options_with_values() {
    let args = Args::try_parse_from([
        "rusdu",
        "/target/path",
        "-t",
        "16",
        "--compress-level",
        "9",
        "--delete-command",
        "trash-put",
        "--color",
        "dark",
        "--graph-style",
        "half-block",
        "--shared-column",
        "unique",
        "--sort",
        "name",
        "--log-file",
        "/var/log/rusdu.log",
    ])
    .unwrap();

    assert_eq!(args.path, Some(PathBuf::from("/target/path")));
    assert_eq!(args.threads, Some(16));
    assert_eq!(args.compress_level, Some(9));
    assert_eq!(args.delete_command, Some("trash-put".to_string()));
    assert_eq!(args.color, "dark");
    assert_eq!(args.graph_style, "half-block");
    assert_eq!(args.shared_column, "unique");
    assert_eq!(args.sort, "name");
    assert_eq!(args.log_file, Some(PathBuf::from("/var/log/rusdu.log")));
}

#[test]
fn test_read_only_stacking() {
    let args1 = Args::try_parse_from(["rusdu", "-r"]).unwrap();
    assert_eq!(args1.read_only, 1);

    let args2 = Args::try_parse_from(["rusdu", "-r", "-r"]).unwrap();
    assert_eq!(args2.read_only, 2);
}

#[test]
fn test_error_handling() {
    // Unknown option
    let err_unknown = Args::try_parse_from(["rusdu", "--nonexistent-option"]);
    assert!(matches!(err_unknown, Err(CliError::UnknownOption(_))));

    // Unexpected argument (multiple paths)
    let err_path = Args::try_parse_from(["rusdu", "/first/path", "/second/path"]);
    assert!(matches!(err_path, Err(CliError::UnexpectedArgument(_))));

    // Parse int error for threads wrapped in Lexopt error
    let err_threads = Args::try_parse_from(["rusdu", "-t", "not_a_number"]);
    assert!(matches!(err_threads, Err(CliError::Lexopt(_))));
}
