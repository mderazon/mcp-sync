use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Logger {
    log_file: PathBuf,
    quiet: bool,
}

impl Logger {
    pub fn new(custom_path: Option<&Path>, quiet: bool) -> Self {
        let log_file = if let Some(p) = custom_path {
            p.to_path_buf()
        } else {
            Self::default_log_path()
        };

        Self { log_file, quiet }
    }

    fn default_log_path() -> PathBuf {
        if let Some(state_dir) = dirs::state_dir() {
            state_dir.join("mcp-sync/mcp-sync.log")
        } else if let Some(home) = dirs::home_dir() {
            home.join(".local/state/mcp-sync/mcp-sync.log")
        } else {
            PathBuf::from("/tmp/mcp-sync.log")
        }
    }

    fn format_now() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Format timestamp safely via libc localtime_r on unix
        #[cfg(unix)]
        {
            let mut tm: libc::tm = unsafe { std::mem::zeroed() };
            let t = now as libc::time_t;
            unsafe {
                libc::localtime_r(&t, &mut tm);
            }
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min,
                tm.tm_sec
            )
        }
        #[cfg(not(unix))]
        {
            format!("unix:{}", now)
        }
    }

    pub fn log(&self, level: &str, msg: &str) {
        let timestamp = Self::format_now();
        let formatted = format!("{} [{}] {}\n", timestamp, level, msg);

        if !self.quiet {
            if level == "ERROR" {
                eprint!("[mcp-sync] {}", formatted);
            } else {
                print!("[mcp-sync] {}", formatted);
            }
        }

        if let Some(parent) = self.log_file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let _ = file.write_all(formatted.as_bytes());
        }
    }

    pub fn info(&self, msg: &str) {
        self.log("INFO", msg);
    }

    pub fn warn(&self, msg: &str) {
        self.log("WARN", msg);
    }

    pub fn error(&self, msg: &str) {
        self.log("ERROR", msg);
    }

    pub fn log_path(&self) -> &Path {
        &self.log_file
    }
}
