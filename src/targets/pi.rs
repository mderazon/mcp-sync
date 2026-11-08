use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use super::Target;
use crate::config::{CanonicalConfig, ServerDefinition};
use crate::fs_utils::atomic_write;

/// Fields owned by the canonical config. Everything else already present in
/// Pi's entry (`exposure`, `toolExposure`, `oauth`, `timeout`, `description`,
/// `cwd`, ...) is preserved across syncs.
const MANAGED_FIELDS: [&str; 7] = [
    "command", "args", "env", "url", "headers", "type", "enabled",
];

pub struct PiTarget {
    path: PathBuf,
}

impl PiTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            std::env::var("PI_CODING_AGENT_DIR")
                .ok()
                .map(|dir| PathBuf::from(dir).join("mcp.json"))
                .or_else(|| dirs::home_dir().map(|h| h.join(".pi/agent/mcp.json")))
                .unwrap_or_else(|| PathBuf::from(".pi/agent/mcp.json"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_existing_root(&self) -> Map<String, Value> {
        if self.path.exists()
            && let Ok(content) = fs::read_to_string(&self.path)
            && let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&content)
        {
            map
        } else {
            Map::new()
        }
    }

    /// Build the full mcp.json document for Pi.
    /// Safe mode: only touches canonical servers; unmanaged servers and
    /// top-level options (e.g. `autoEnableCodemode`) are preserved intact.
    pub fn build_doc(&self, config: &CanonicalConfig) -> Value {
        let mut root = self.read_existing_root();
        let existing_servers = root
            .get("mcpServers")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let mut servers = Map::new();

        // 1. Add/update servers defined in canonical config
        for (name, server) in &config.servers {
            let existing = existing_servers.get(name).and_then(Value::as_object);
            let mut entry = existing.cloned().unwrap_or_default();

            for field in MANAGED_FIELDS {
                entry.remove(field);
            }

            if let Some(url) = server.get_url() {
                entry.remove("cwd");
                entry.insert("url".to_string(), Value::String(url.to_string()));
                if let Some(headers) = server.get_headers() {
                    entry.insert(
                        "headers".to_string(),
                        serde_json::to_value(headers).unwrap(),
                    );
                }
            } else {
                // OAuth only applies to HTTP servers.
                entry.remove("oauth");

                if let Some(command) = &server.command {
                    entry.insert("command".to_string(), Value::String(command.clone()));
                }
                if let Some(args) = &server.args {
                    entry.insert("args".to_string(), serde_json::to_value(args).unwrap());
                }
                if let Some(env) = build_env(server) {
                    entry.insert("env".to_string(), env);
                }
            }

            // `type` is optional in Pi. Only pass through values Pi understands
            // (stdio, http, streamable-http); legacy SSE endpoints fall back to
            // transport auto-detection.
            if let Some(server_type) = server.server_type.as_deref() {
                let valid = matches!(
                    (server.is_remote(), server_type),
                    (true, "http" | "streamable-http") | (false, "stdio")
                );
                if valid {
                    entry.insert("type".to_string(), Value::String(server_type.to_string()));
                }
            }

            // Smart merge: canonical explicit enabled takes precedence, otherwise preserve existing state
            let enabled = server
                .enabled
                .or_else(|| {
                    existing
                        .and_then(|e| e.get("enabled"))
                        .and_then(Value::as_bool)
                })
                .unwrap_or(true);

            if !enabled {
                entry.insert("enabled".to_string(), Value::Bool(false));
            }

            servers.insert(name.clone(), Value::Object(entry));
        }

        // 2. Safe mode: preserve any unmanaged servers already in Pi
        for (name, raw_val) in existing_servers {
            if !config.servers.contains_key(&name) {
                servers.insert(name, raw_val);
            }
        }

        root.insert("mcpServers".to_string(), Value::Object(servers));
        Value::Object(root)
    }

    /// Sync canonical config into Pi's ~/.pi/agent/mcp.json
    pub fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        let doc = self.build_doc(config);
        let formatted = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("Failed to serialize Pi config: {}", e))?;

        if dry_run {
            return Ok(format!("[pi] Would write {}", self.path.display()));
        }

        let updated = atomic_write(&self.path, &formatted)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        if updated {
            Ok(format!("[pi] Updated {}", self.path.display()))
        } else {
            Ok(format!("[pi] Unchanged {}", self.path.display()))
        }
    }
}

/// Pi has no `env_vars` field, but `env` values support `${VAR}` expansion,
/// so pass-through variables from the canonical config are written as such.
fn build_env(server: &ServerDefinition) -> Option<Value> {
    let mut env = Map::new();

    if let Some(env_vars) = &server.env_vars {
        for name in env_vars {
            env.insert(name.clone(), Value::String(format!("${{{name}}}")));
        }
    }

    if let Some(values) = &server.env {
        for (key, value) in values {
            env.insert(key.clone(), Value::String(value.clone()));
        }
    }

    if env.is_empty() {
        None
    } else {
        Some(Value::Object(env))
    }
}

impl Target for PiTarget {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        self.sync(config, dry_run)
    }
}
