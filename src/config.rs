use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(rename = "serverUrl", skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,

    #[serde(rename = "http_headers", skip_serializing_if = "Option::is_none")]
    pub http_headers: Option<BTreeMap<String, String>>,

    #[serde(rename = "env_vars", skip_serializing_if = "Option::is_none")]
    pub env_vars: Option<Vec<String>>,
}

impl ServerDefinition {
    /// Return effective URL if this is a remote server
    pub fn get_url(&self) -> Option<&str> {
        self.url.as_deref().or(self.server_url.as_deref())
    }

    /// Return HTTP headers if configured
    pub fn get_headers(&self) -> Option<&BTreeMap<String, String>> {
        self.headers.as_ref().or(self.http_headers.as_ref())
    }

    /// True if server is configured as a remote URL endpoint
    pub fn is_remote(&self) -> bool {
        self.get_url().is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalConfig {
    pub servers: BTreeMap<String, ServerDefinition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
}

/// Find the canonical config path.
/// Looks at explicit path if passed, otherwise checks:
/// 1. ~/.config/mcp/servers.json
/// 2. ~/.config/mcp/config.json
/// 3. ~/.config/mcp/mcp.json
pub fn resolve_canonical_path(custom_path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = custom_path {
        return Ok(p.to_path_buf());
    }

    let base = dirs::config_dir()
        .map(|d| d.join("mcp"))
        .or_else(|| dirs::home_dir().map(|h| h.join(".config/mcp")))
        .ok_or_else(|| "Could not determine user config directory".to_string())?;

    let candidates = [
        base.join("servers.json"),
        base.join("config.json"),
        base.join("mcp.json"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // On macOS, dirs::config_dir() is ~/Library/Application Support, but users also frequently use ~/.config/mcp
    if let Some(home) = dirs::home_dir() {
        let home_mcp = home.join(".config/mcp");
        for name in &["servers.json", "config.json", "mcp.json"] {
            let candidate = home_mcp.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // Default to servers.json even if not existing yet
    Ok(candidates[0].clone())
}

/// Parse canonical config from string
pub fn parse_canonical_str(content: &str) -> Result<CanonicalConfig, String> {
    let root: Value =
        serde_json::from_str(content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let root_obj = root
        .as_object()
        .ok_or_else(|| "Root of config must be a JSON object".to_string())?;

    let servers_val = root_obj
        .get("servers")
        .or_else(|| root_obj.get("mcpServers"))
        .ok_or_else(|| {
            "Config file must contain either \"servers\" or \"mcpServers\"".to_string()
        })?;

    let servers: BTreeMap<String, ServerDefinition> =
        serde_json::from_value(servers_val.clone())
            .map_err(|e| format!("Invalid server definitions: {}", e))?;

    let inputs = root_obj.get("inputs").cloned();

    Ok(CanonicalConfig { servers, inputs })
}

/// Load canonical config from disk
pub fn load_canonical(path: &Path) -> Result<CanonicalConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_canonical_str(&content)
}
