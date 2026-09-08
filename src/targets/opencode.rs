use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::Target;
use crate::config::CanonicalConfig;
use crate::fs_utils::atomic_write;

pub struct OpenCodeTarget {
    path: PathBuf,
}

impl OpenCodeTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            dirs::config_dir()
                .map(|d| d.join("opencode/opencode.json"))
                .or_else(|| dirs::home_dir().map(|h| h.join(".config/opencode/opencode.json")))
                .unwrap_or_else(|| PathBuf::from(".config/opencode/opencode.json"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read existing servers and enabled states from ~/.config/opencode/opencode.json
    pub fn parse_existing(&self) -> (BTreeMap<String, Value>, BTreeMap<String, bool>) {
        let mut servers_map = BTreeMap::new();
        let mut states = BTreeMap::new();
        if self.path.exists()
            && let Ok(content) = fs::read_to_string(&self.path)
            && let Ok(v) = serde_json::from_str::<Value>(&content)
            && let Some(servers) = v.get("mcp").and_then(|m| m.as_object())
        {
            for (name, s_val) in servers {
                let enabled = s_val
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                states.insert(name.clone(), enabled);
                servers_map.insert(name.clone(), s_val.clone());
            }
        }
        (servers_map, states)
    }

    /// Build updated mcp object for OpenCode
    /// Safe mode: only touches canonical servers; unmanaged servers in OpenCode are preserved intact.
    pub fn build_mcp_map(&self, config: &CanonicalConfig) -> Map<String, Value> {
        let (existing_servers, existing_states) = self.parse_existing();
        let mut mcp_map = Map::new();

        // 1. Add/update servers defined in canonical config
        for (name, server) in &config.servers {
            let mut obj = Map::new();

            // Smart merge: preserve existing enabled state, default true
            let enabled = existing_states
                .get(name)
                .copied()
                .or(server.enabled)
                .unwrap_or(true);
            obj.insert("enabled".to_string(), Value::Bool(enabled));

            if let Some(url) = server.get_url() {
                obj.insert("type".to_string(), Value::String("remote".to_string()));
                obj.insert("url".to_string(), Value::String(url.to_string()));
                if let Some(headers) = server.get_headers() {
                    obj.insert(
                        "headers".to_string(),
                        serde_json::to_value(headers).unwrap(),
                    );
                }
                if let Some(ref env) = server.env {
                    obj.insert(
                        "environment".to_string(),
                        serde_json::to_value(env).unwrap(),
                    );
                }
            } else {
                obj.insert("type".to_string(), Value::String("local".to_string()));

                // OpenCode uses command: ["cmd", "arg1", "arg2", ...]
                let mut cmd_vec = Vec::new();
                if let Some(ref cmd) = server.command {
                    cmd_vec.push(Value::String(cmd.clone()));
                }
                if let Some(ref args) = server.args {
                    for arg in args {
                        cmd_vec.push(Value::String(arg.clone()));
                    }
                }
                obj.insert("command".to_string(), Value::Array(cmd_vec));

                // OpenCode uses "environment" key
                if let Some(ref env) = server.env {
                    obj.insert(
                        "environment".to_string(),
                        serde_json::to_value(env).unwrap(),
                    );
                }
            }

            mcp_map.insert(name.clone(), Value::Object(obj));
        }

        // 2. Safe mode: preserve any unmanaged servers already in OpenCode
        for (name, raw_val) in existing_servers {
            if !config.servers.contains_key(&name) {
                mcp_map.insert(name, raw_val);
            }
        }

        mcp_map
    }
}

impl Target for OpenCodeTarget {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        let mcp_map = self.build_mcp_map(config);

        let mut root: Value = if self.path.exists() {
            let content = fs::read_to_string(&self.path)
                .map_err(|e| format!("Failed to read {}: {}", self.path.display(), e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON in {}: {}", self.path.display(), e))?
        } else {
            Value::Object(Map::new())
        };

        if let Some(root_obj) = root.as_object_mut() {
            root_obj.insert("mcp".to_string(), Value::Object(mcp_map));
        } else {
            let mut obj = Map::new();
            obj.insert("mcp".to_string(), Value::Object(mcp_map));
            root = Value::Object(obj);
        }

        let formatted = serde_json::to_string_pretty(&root)
            .map_err(|e| format!("Failed to serialize OpenCode config: {}", e))?;

        if dry_run {
            return Ok(format!("[opencode] Would write {}", self.path.display()));
        }

        atomic_write(&self.path, &formatted)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        Ok(format!("[opencode] Updated {}", self.path.display()))
    }
}
