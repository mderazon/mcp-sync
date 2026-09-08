use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomically write contents to `dest_path` by first writing to a temporary file
/// in the same directory and renaming it into place.
pub fn atomic_write(dest_path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = process::id();
    let file_name = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let temp_name = format!(".{}.tmp.{}.{}", file_name, pid, timestamp);
    let temp_path = dest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(temp_name);

    {
        let mut f = File::create(&temp_path)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }

    // Atomic replace on Unix; handle existing file replace on Windows
    #[cfg(windows)]
    {
        if dest_path.exists() {
            let _ = fs::remove_file(dest_path);
        }
    }
    if let Err(err) = fs::rename(&temp_path, dest_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }

    Ok(())
}
