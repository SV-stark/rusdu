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
        let mut abs_path_str = abs_path.to_string_lossy().into_owned();
        #[cfg(windows)]
        if let Some(stripped) = abs_path_str.strip_prefix(r"\\?\") {
            abs_path_str = stripped.to_string();
        }

        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg(format!("{} \"%NCDU_DELETE_PATH%\"", cmd_str))
                .env("NCDU_DELETE_PATH", &abs_path_str)
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
                .env("NCDU_DELETE_PATH", &abs_path_str)
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
            remove_file_with_retry(path)?;
        } else if metadata.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            remove_file_with_retry(path)?;
        }
    }

    Ok(())
}

#[allow(clippy::permissions_set_readonly_false)]
fn remove_file_with_retry(path: &Path) -> Result<()> {
    if let Err(err) = std::fs::remove_file(path) {
        #[cfg(windows)]
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            if let Ok(mut perms) = std::fs::metadata(path).map(|m| m.permissions()) {
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(path, perms);
                    return std::fs::remove_file(path).map_err(Into::into);
                }
            }
        }
        return Err(err.into());
    }
    Ok(())
}
