use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn delete_item(path: &Path, custom_command: Option<&str>, read_only: bool) -> Result<()> {
    if read_only {
        return Err(anyhow!("Cannot delete in read-only mode"));
    }

    if let Some(cmd_str) = custom_command {
        // Run custom command
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let abs_path_str = abs_path.to_string_lossy();

        #[cfg(windows)]
        {
            let full_cmd = format!("{} \"{}\"", cmd_str, abs_path_str);
            let mut child = Command::new("cmd")
                .arg("/C")
                .arg(&full_cmd)
                .env("NCDU_DELETE_PATH", &*abs_path_str)
                .env("NCDU_LEVEL", "1")
                .spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(anyhow!("Custom delete command failed"));
            }
        }

        #[cfg(unix)]
        {
            let full_cmd = format!("{} '{}'", cmd_str, abs_path_str.replace("'", "'\\''"));
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&full_cmd)
                .env("NCDU_DELETE_PATH", &*abs_path_str)
                .env("NCDU_LEVEL", "1")
                .spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(anyhow!("Custom delete command failed"));
            }
        }
    } else {
        // Built-in deletion
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}
