use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn load_config(args: &mut crate::cli::Args) -> Result<()> {
    let mut config_args = vec![
        std::env::args()
            .next()
            .unwrap_or_else(|| "rusdu".to_string()),
    ];

    // 1. Load system config on Unix
    #[cfg(unix)]
    {
        let system_config = Path::new("/etc/ncdu.conf");
        if system_config.exists() {
            if let Err(e) = append_config_args(system_config, &mut config_args) {
                log::warn!("Failed to read system config /etc/ncdu.conf: {}", e);
            }
        }
    }

    // 2. Load user config
    if let Some(mut user_config) = dirs::config_dir() {
        user_config.push("ncdu");
        user_config.push("config");
        if user_config.exists() {
            append_config_args(&user_config, &mut config_args)?;
        }
    }

    // If we have loaded configuration arguments, merge them with actual command line arguments
    if config_args.len() > 1 {
        // Appending the actual command line arguments (skipping the binary name)
        let actual_args = std::env::args().skip(1);
        config_args.extend(actual_args);

        // Reparse the combined args
        match crate::cli::Args::try_parse_from(&config_args) {
            Ok(parsed) => {
                *args = parsed;
            }
            Err(e) => {
                // If there's an error and we were parsing config files, print it unless it was suppressed
                // Actually, let's just print the error if not run in silent/headless mode
                eprintln!("Configuration parsing error:\n{}", e);
            }
        }
    }

    Ok(())
}

fn append_config_args(path: &Path, args: &mut Vec<String>) -> Result<()> {
    let file =
        File::open(path).with_context(|| format!("Failed to open config file {:?}", path))?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Handle '@' prefix to suppress errors (in our case, we just parse it anyway,
        // but if it fails we might handle it differently. For now, just strip it or process it)
        let clean_line = if trimmed.starts_with('@') {
            &trimmed[1..]
        } else {
            trimmed
        };

        // Split by whitespace to extract options and values (simplistic shell word splitting)
        // Note: ncdu expects one option per line. E.g.:
        // --exclude .git
        // or just:
        // -e
        // Let's split it into tokens
        let parts = shell_words::split(clean_line).unwrap_or_else(|_| {
            clean_line
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        });

        for part in parts {
            if !part.is_empty() {
                // Handle tilde expansion for paths (e.g. ~/excludes)
                let expanded_part = if part.starts_with("~/") || part == "~" {
                    if let Some(home) = dirs::home_dir() {
                        part.replacen('~', &home.to_string_lossy(), 1)
                    } else {
                        part
                    }
                } else {
                    part
                };
                args.push(expanded_part);
            }
        }
    }

    Ok(())
}

// Simple implementation of shell_words::split if the crate is not loaded, but since we have it,
// we can use standard splitting or implement a small parser. Let's write a simple token splitter
// to avoid extra external crates if possible, but since we put `shell_words`? Wait, is it in Cargo.toml?
// No, I did not put `shell_words` in Cargo.toml. Let's write a simple helper function.
mod shell_words {
    pub fn split(input: &str) -> Result<Vec<String>, ()> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut in_double_quote = false;
        let mut in_single_quote = false;
        let mut escaped = false;

        for c in input.chars() {
            if escaped {
                word.push(c);
                escaped = false;
            } else if c == '\\' && !in_single_quote {
                escaped = true;
            } else if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            } else if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            } else if c.is_whitespace() && !in_double_quote && !in_single_quote {
                if !word.is_empty() {
                    words.push(word.clone());
                    word.clear();
                }
            } else {
                word.push(c);
            }
        }

        if !word.is_empty() {
            words.push(word);
        }

        if in_double_quote || in_single_quote || escaped {
            Err(())
        } else {
            Ok(words)
        }
    }
}
