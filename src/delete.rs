use anyhow::{Result, anyhow};
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
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg(format!("{} \"%NCDU_DELETE_PATH%\"", cmd_str))
                .env("NCDU_DELETE_PATH", &*abs_path_str)
                .env("NCDU_LEVEL", "1");
            let mut child = cmd.spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(anyhow!("Custom delete command failed"));
            }
        }

        #[cfg(unix)]
        {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg(format!("{} \"$NCDU_DELETE_PATH\"", cmd_str))
                .env("NCDU_DELETE_PATH", &*abs_path_str)
                .env("NCDU_LEVEL", "1");
            let mut child = cmd.spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(anyhow!("Custom delete command failed"));
            }
        }
    } else {
        // Built-in deletion
        let metadata = path.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            std::fs::remove_file(path)?;
        } else if metadata.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}
