use anyhow::{Result, anyhow};
use std::path::Path;
use std::process::Command;

pub fn spawn_shell(dir_path: &Path, read_only: bool) -> Result<()> {
    if read_only {
        return Err(anyhow!("Cannot spawn shell in read-only mode"));
    }

    // Determine shell executable
    let shell_exe = if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
    } else {
        std::env::var("NCDU_SHELL")
            .or_else(|_| std::env::var("SHELL"))
            .unwrap_or_else(|_| "/bin/sh".to_string())
    };

    // Increment NCDU_LEVEL env variable
    let ncdu_level = std::env::var("NCDU_LEVEL")
        .ok()
        .and_then(|val| val.parse::<i32>().ok())
        .unwrap_or(0)
        + 1;

    // Suspend crossterm TUI raw mode before launching shell
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
        crossterm::event::DisableMouseCapture
    )?;

    let mut cmd = Command::new(&shell_exe);
    cmd.current_dir(dir_path)
        .env("NCDU_LEVEL", ncdu_level.to_string());

    let status = cmd.status();

    // Re-enable TUI raw mode
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
        crossterm::event::EnableMouseCapture
    )?;

    match status {
        Ok(s) => {
            if s.success() {
                Ok(())
            } else {
                Err(anyhow!("Shell exited with error status"))
            }
        }
        Err(e) => Err(anyhow!("Failed to spawn shell {:?}: {}", shell_exe, e)),
    }
}
