use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub fn load_custom_actions() -> HashMap<char, String> {
    let mut actions = HashMap::new();

    // Default actions
    actions.insert('c', "copy".to_string());
    actions.insert('o', "open".to_string());
    actions.insert('v', "editor".to_string());

    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("rusdu");
        config_path.push("actions.conf");
        if config_path.exists() {
            if let Ok(file) = std::fs::File::open(&config_path) {
                let reader = std::io::BufReader::new(file);
                use std::io::BufRead;
                for line in reader.lines() {
                    if let Ok(l) = line {
                        let trimmed = l.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }
                        if let Some(pos) = trimmed.find('=') {
                            let key_part = trimmed[..pos].trim();
                            let cmd_part = trimmed[pos + 1..].trim();
                            if key_part.len() == 1 {
                                if let Some(key_char) = key_part.chars().next() {
                                    actions.insert(key_char, cmd_part.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    actions
}

pub fn execute_custom_action(cmd: &str, path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy().to_string();

    match cmd {
        "copy" => {
            #[cfg(target_os = "windows")]
            {
                let cmd_str = format!("Set-Clipboard -Value '{}'", path_str.replace("'", "''"));
                Command::new("powershell")
                    .args(&["-Command", &cmd_str])
                    .output()?;
            }
            #[cfg(target_os = "macos")]
            {
                let mut child = Command::new("pbcopy")
                    .stdin(std::process::Stdio::piped())
                    .spawn()?;
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    stdin.write_all(path_str.as_bytes())?;
                }
                child.wait()?;
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let has_xclip = Command::new("which")
                    .arg("xclip")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if has_xclip {
                    let mut child = Command::new("xclip")
                        .args(&["-selection", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()?;
                    if let Some(mut stdin) = child.stdin.take() {
                        use std::io::Write;
                        stdin.write_all(path_str.as_bytes())?;
                    }
                    child.wait()?;
                } else {
                    let mut child = Command::new("xsel")
                        .args(&["--clipboard", "--input"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()?;
                    if let Some(mut stdin) = child.stdin.take() {
                        use std::io::Write;
                        stdin.write_all(path_str.as_bytes())?;
                    }
                    child.wait()?;
                }
            }
        }
        "open" => {
            #[cfg(target_os = "windows")]
            {
                if path.is_file() {
                    Command::new("explorer")
                        .arg(format!("/select,{}", path_str))
                        .spawn()?;
                } else {
                    Command::new("explorer").arg(&path_str).spawn()?;
                }
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("open").args(&["-R", &path_str]).spawn()?;
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let parent = path.parent().unwrap_or(path);
                Command::new("xdg-open")
                    .arg(parent.to_string_lossy().to_string())
                    .spawn()?;
            }
        }
        "editor" => {
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or_else(|_| {
                    if cfg!(target_os = "windows") {
                        "notepad".to_string()
                    } else {
                        "vi".to_string()
                    }
                });

            // Suspend raw mode
            crossterm::terminal::disable_raw_mode()?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show,
                crossterm::event::DisableMouseCapture
            )?;

            // Run editor (simple shell split)
            let mut parts = editor
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            let binary = if parts.is_empty() {
                editor
            } else {
                parts.remove(0)
            };
            parts.push(path_str);

            let mut child = Command::new(binary).args(&parts).spawn()?;
            child.wait()?;

            // Restore TUI
            crossterm::terminal::enable_raw_mode()?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::EnterAlternateScreen,
                crossterm::cursor::Hide,
                crossterm::event::EnableMouseCapture
            )?;
        }
        custom_cmd => {
            let shell_cmd = if custom_cmd.contains("{path}") {
                custom_cmd.replace("{path}", &path_str)
            } else {
                format!("{} \"{}\"", custom_cmd, path_str)
            };

            // Suspend TUI
            crossterm::terminal::disable_raw_mode()?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show,
                crossterm::event::DisableMouseCapture
            )?;

            #[cfg(target_os = "windows")]
            {
                let mut child = Command::new("powershell")
                    .args(&["-Command", &shell_cmd])
                    .spawn()?;
                child.wait()?;
            }
            #[cfg(not(target_os = "windows"))]
            {
                let mut child = Command::new("sh").args(&["-c", &shell_cmd]).spawn()?;
                child.wait()?;
            }

            // Restore TUI
            crossterm::terminal::enable_raw_mode()?;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::EnterAlternateScreen,
                crossterm::cursor::Hide,
                crossterm::event::EnableMouseCapture
            )?;
        }
    }
    Ok(())
}
