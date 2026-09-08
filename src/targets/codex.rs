use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::Target;
use crate::config::CanonicalConfig;
use crate::fs_utils::atomic_write;

pub struct CodexTarget {
    path: PathBuf,
}

impl CodexTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            std::env::var("CODEX_HOME")
                .ok()
                .map(|dir| PathBuf::from(dir).join("config.toml"))
                .or_else(|| dirs::home_dir().map(|h| h.join(".codex/config.toml")))
                .unwrap_or_else(|| PathBuf::from(".codex/config.toml"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read existing servers and their enabled states from ~/.codex/config.toml
    pub fn parse_existing_states(&self) -> (BTreeMap<String, bool>, Option<DocumentMut>) {
        let mut states = BTreeMap::new();
        if self.path.exists()
            && let Ok(content) = fs::read_to_string(&self.path)
            && let Ok(doc) = content.parse::<DocumentMut>()
        {
            if let Some(mcp) = doc.get("mcp_servers").and_then(|m| m.as_table_like()) {
                for (name, item) in mcp.iter() {
                    let enabled = item
                        .get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    states.insert(name.to_string(), enabled);
                }
            }
            return (states, Some(doc));
        }
        (states, None)
    }

    /// Update or build the DocumentMut for ~/.codex/config.toml
    /// Safe mode: only touches canonical servers; unmanaged servers (e.g. node_repl, cua_repl)
    /// and other configuration blocks (approvals, models, plugins, etc.) are preserved intact.
    pub fn update_doc(&self, config: &CanonicalConfig) -> Result<DocumentMut, String> {
        let (existing_states, parsed_doc) = self.parse_existing_states();
        let mut doc = parsed_doc.unwrap_or_default();

        if !doc.contains_key("mcp_servers") {
            doc.insert("mcp_servers", Item::Table(Table::new()));
        }

        let mcp_servers = doc["mcp_servers"]
            .as_table_like_mut()
            .ok_or_else(|| "mcp_servers in config.toml is not a table".to_string())?;

        for (name, server) in &config.servers {
            // Smart merge: preserve existing enabled state, default true
            let enabled = existing_states
                .get(name)
                .copied()
                .or(server.enabled)
                .unwrap_or(true);

            if !mcp_servers.contains_key(name) {
                mcp_servers.insert(name, Item::Table(Table::new()));
            }

            let tbl = match mcp_servers.get_mut(name) {
                Some(Item::Table(t)) => t,
                _ => continue,
            };

            if let Some(url) = server.get_url() {
                tbl.remove("command");
                tbl.remove("args");
                tbl.remove("env");
                tbl.insert("url", Item::Value(url.into()));

                if let Some(headers) = server.get_headers() {
                    let mut h_tbl = Table::new();
                    for (k, v) in headers {
                        h_tbl.insert(k, Item::Value(v.as_str().into()));
                    }
                    tbl.insert("http_headers", Item::Table(h_tbl));
                }
            } else {
                tbl.remove("url");
                tbl.remove("http_headers");

                if let Some(ref cmd) = server.command {
                    tbl.insert("command", Item::Value(cmd.as_str().into()));
                }
                if let Some(ref args) = server.args {
                    let mut arr = Array::new();
                    for a in args {
                        arr.push(a.as_str());
                    }
                    tbl.insert("args", Item::Value(Value::Array(arr)));
                }
                if let Some(ref env) = server.env {
                    let mut env_tbl = Table::new();
                    for (k, v) in env {
                        env_tbl.insert(k, Item::Value(v.as_str().into()));
                    }
                    tbl.insert("env", Item::Table(env_tbl));
                }
            }

            if let Some(ref env_vars) = server.env_vars {
                let mut arr = Array::new();
                for ev in env_vars {
                    arr.push(ev.as_str());
                }
                tbl.insert("env_vars", Item::Value(Value::Array(arr)));
            }

            if !enabled {
                tbl.insert("enabled", Item::Value(false.into()));
            } else {
                tbl.remove("enabled");
            }
        }

        Ok(doc)
    }

    /// Sync canonical config into Codex's config.toml
    pub fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        let doc = self.update_doc(config)?;
        let formatted = doc.to_string();

        if dry_run {
            return Ok(format!("[codex] Would update {}", self.path.display()));
        }

        atomic_write(&self.path, &formatted)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        Ok(format!("[codex] Updated {}", self.path.display()))
    }
}

impl Target for CodexTarget {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        self.sync(config, dry_run)
    }
}
