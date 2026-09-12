use anyhow::{Context, Result, anyhow};
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

    // 2. Load user config (search ~/.config/rusdu/config then ~/.config/ncdu/config)
    if let Some(config_dir) = dirs::config_dir() {
        let rusdu_config = config_dir.join("rusdu").join("config");
        let ncdu_config = config_dir.join("ncdu").join("config");
        if rusdu_config.exists() {
            append_config_args(&rusdu_config, &mut config_args)?;
        } else if ncdu_config.exists() {
            append_config_args(&ncdu_config, &mut config_args)?;
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
                return Err(anyhow!("Configuration parsing error:\n{}", e));
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

        // Handle '@' prefix to suppress errors for unsupported/invalid options
        let is_suppressed = trimmed.starts_with('@');
        let clean_line = trimmed.strip_prefix('@').unwrap_or(trimmed);

        // Split by whitespace / shell words to extract options and values
        let parts = shell_words::split(clean_line).unwrap_or_else(|_| {
            clean_line
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        });

        let mut line_tokens = Vec::new();
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
                line_tokens.push(expanded_part);
            }
        }

        if !line_tokens.is_empty() {
            if is_suppressed {
                let mut test_args = vec!["rusdu".to_string()];
                test_args.extend(line_tokens.clone());
                if crate::cli::Args::try_parse_from(&test_args).is_ok() {
                    args.extend(line_tokens);
                }
            } else {
                args.extend(line_tokens);
            }
        }
    }

    Ok(())
}

/// Helper module for tokenizing configuration file lines into command-line arguments.
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
                    words.push(std::mem::take(&mut word));
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

#[cfg(test)]
mod tests {
    use super::shell_words;

    #[test]
    fn test_shell_words_split() {
        let input = "--exclude \"*.tmp\" --threads 4 'single arg'";
        let res = shell_words::split(input).unwrap();
        assert_eq!(
            res,
            vec!["--exclude", "*.tmp", "--threads", "4", "single arg"]
        );
    }
}
