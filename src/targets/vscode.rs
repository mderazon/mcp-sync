use std::fs;
use std::path::{Path, PathBuf};
use serde_json::{Map, Value};

use crate::config::CanonicalConfig;
use crate::fs_utils::atomic_write;

pub struct VSCodeTarget {
    path: PathBuf,
}

impl VSCodeTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            dirs::config_dir()
                .map(|d| d.join("Code/User/mcp.json"))
                .or_else(|| dirs::home_dir().map(|h| h.join(".config/Code/User/mcp.json")))
                .unwrap_or_else(|| PathBuf::from(".config/Code/User/mcp.json"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Build the full mcp.json document for VSCode
    /// Safe mode: only touches canonical servers; unmanaged servers in VSCode are preserved intact.
    pub fn build_doc(&self, config: &CanonicalConfig) -> Value {
        let mut servers_map = Map::new();

        // 1. Add/update servers defined in canonical config
        for (name, server) in &config.servers {
            let mut obj = Map::new();

            if let Some(url) = server.get_url() {
                let stype = server.server_type.as_deref().unwrap_or("http");
                obj.insert("type".to_string(), Value::String(stype.to_string()));
                obj.insert("url".to_string(), Value::String(url.to_string()));
            } else {
                let stype = server.server_type.as_deref().unwrap_or("stdio");
                obj.insert("type".to_string(), Value::String(stype.to_string()));
                if let Some(ref cmd) = server.command {
                    obj.insert("command".to_string(), Value::String(cmd.clone()));
                }
                if let Some(ref args) = server.args {
                    obj.insert("args".to_string(), serde_json::to_value(args).unwrap());
                }
                if let Some(ref env) = server.env {
                    obj.insert("env".to_string(), serde_json::to_value(env).unwrap());
                }
            }

            servers_map.insert(name.clone(), Value::Object(obj));
        }

        // 2. Safe mode: preserve any unmanaged servers already in VSCode
        let mut inputs_val = config.inputs.clone();
        if self.path.exists()
            && let Ok(text) = fs::read_to_string(&self.path)
            && let Ok(existing_json) = serde_json::from_str::<Value>(&text)
        {
            if let Some(existing_servers) = existing_json.get("servers").and_then(|s| s.as_object()) {
                for (name, s_val) in existing_servers {
                    if !config.servers.contains_key(name) {
                        servers_map.insert(name.clone(), s_val.clone());
                    }
                }
            }
            if inputs_val.is_none() && let Some(existing_inputs) = existing_json.get("inputs") {
                inputs_val = Some(existing_inputs.clone());
            }
        }

        let mut root = Map::new();
        root.insert("servers".to_string(), Value::Object(servers_map));
        root.insert(
            "inputs".to_string(),
            inputs_val.unwrap_or_else(|| Value::Array(Vec::new())),
        );

        Value::Object(root)
    }

    /// Sync the canonical config into VSCode's User mcp.json
    pub fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        let doc = self.build_doc(config);
        let formatted = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("Failed to serialize VSCode config: {}", e))?;

        if dry_run {
            return Ok(format!("[vscode] Would write {}", self.path.display()));
        }

        atomic_write(&self.path, &formatted)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        Ok(format!("[vscode] Updated {}", self.path.display()))
    }
}
