use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CanonicalConfig;
use crate::fs_utils::atomic_write;

pub struct AntigravityTarget {
    path: PathBuf,
}

impl AntigravityTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".gemini/config/mcp_config.json"))
                .unwrap_or_else(|| PathBuf::from(".gemini/config/mcp_config.json"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read existing servers and disabled states from ~/.gemini/config/mcp_config.json
    pub fn parse_existing(&self) -> (BTreeMap<String, Value>, BTreeMap<String, bool>) {
        let mut servers_map = BTreeMap::new();
        let mut states = BTreeMap::new();
        if self.path.exists()
            && let Ok(content) = fs::read_to_string(&self.path)
            && let Ok(v) = serde_json::from_str::<Value>(&content)
            && let Some(servers) = v.get("mcpServers").and_then(|ms| ms.as_object())
        {
            for (name, s_val) in servers {
                let is_disabled = s_val
                    .get("disabled")
                    .and_then(|d| d.as_bool())
                    .unwrap_or(false);
                states.insert(name.clone(), is_disabled);
                servers_map.insert(name.clone(), s_val.clone());
            }
        }
        (servers_map, states)
    }

    /// Build the mcpServers JSON document for Antigravity
    /// Safe mode: only touches canonical servers; unmanaged servers in Antigravity are preserved intact.
    pub fn build_doc(&self, config: &CanonicalConfig) -> Value {
        let (existing_servers, existing_disabled) = self.parse_existing();
        let mut servers_map = Map::new();

        // 1. Add/update servers defined in canonical config
        for (name, server) in &config.servers {
            let mut obj = Map::new();

            if let Some(url) = server.get_url() {
                obj.insert("serverUrl".to_string(), Value::String(url.to_string()));
            } else {
                if let Some(ref cmd) = server.command {
                    obj.insert("command".to_string(), Value::String(cmd.clone()));
                }
                if let Some(ref args) = server.args {
                    obj.insert("args".to_string(), serde_json::to_value(args).unwrap());
                }
                if let Some(ref env) = server.env {
                    obj.insert("env".to_string(), serde_json::to_value(env).unwrap());
                }
                if let Some(ref stype) = server.server_type
                    && stype == "stdio"
                {
                    obj.insert("type".to_string(), Value::String("stdio".to_string()));
                }
            }

            // Smart merge: preserve existing disabled state in Antigravity
            let is_disabled = existing_disabled
                .get(name)
                .copied()
                .or_else(|| server.enabled.map(|e| !e))
                .unwrap_or(false);

            if is_disabled {
                obj.insert("disabled".to_string(), Value::Bool(true));
            }

            servers_map.insert(name.clone(), Value::Object(obj));
        }

        // 2. Safe mode: preserve any unmanaged servers already in Antigravity
        for (name, raw_val) in existing_servers {
            if !config.servers.contains_key(&name) {
                servers_map.insert(name, raw_val);
            }
        }

        let mut root = Map::new();
        root.insert("mcpServers".to_string(), Value::Object(servers_map));

        Value::Object(root)
    }

    /// Sync canonical config into Antigravity's mcp_config.json
    pub fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        let doc = self.build_doc(config);
        let formatted = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("Failed to serialize Antigravity config: {}", e))?;

        if dry_run {
            return Ok(format!("[antigravity] Would write {}", self.path.display()));
        }

        atomic_write(&self.path, &formatted)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        Ok(format!("[antigravity] Updated {}", self.path.display()))
    }
}

impl super::Target for AntigravityTarget {
    fn name(&self) -> &'static str {
        "antigravity"
    }

    fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        self.sync(config, dry_run)
    }
}
