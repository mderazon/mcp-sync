use mcp_sync::config::parse_canonical_str;
use mcp_sync::targets::{AntigravityTarget, VSCodeTarget};
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn test_antigravity_disabled_state_preservation() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"{
  "mcpServers": {
    "server-disabled": {
      "command": "node",
      "args": ["server.js"],
      "disabled": true
    },
    "server-active": {
      "command": "python",
      "args": ["server.py"]
    }
  }
}"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = AntigravityTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "server-disabled": {
                "command": "node",
                "args": ["updated.js"]
            },
            "server-active": {
                "command": "python",
                "args": ["server.py"]
            },
            "server-new": {
                "url": "https://remote.example.com/sse"
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();
    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&updated).unwrap();
    let servers = doc.get("mcpServers").unwrap().as_object().unwrap();

    // server-disabled still has disabled: true
    let s_disabled = servers.get("server-disabled").unwrap();
    assert_eq!(s_disabled.get("disabled").and_then(|d| d.as_bool()), Some(true));
    assert_eq!(s_disabled.get("args").unwrap().as_array().unwrap()[0].as_str().unwrap(), "updated.js");

    // server-active has no disabled flag
    let s_active = servers.get("server-active").unwrap();
    assert!(s_active.get("disabled").is_none());

    // server-new has serverUrl
    let s_new = servers.get("server-new").unwrap();
    assert_eq!(s_new.get("serverUrl").unwrap().as_str().unwrap(), "https://remote.example.com/sse");
    assert!(s_new.get("disabled").is_none());
}

#[test]
fn test_vscode_servers_and_inputs_preservation() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"{
  "servers": {
    "old": {
      "type": "stdio",
      "command": "echo"
    }
  },
  "inputs": [
    {
      "id": "my-secret-key",
      "type": "promptString",
      "description": "Enter API key"
    }
  ]
}"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = VSCodeTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "stdio-tool": {
                "command": "npx",
                "args": ["-y", "my-pkg"],
                "env": { "FOO": "bar" }
            },
            "http-tool": {
                "url": "https://example.com/mcp"
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();
    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&updated).unwrap();

    // Verify servers block
    let servers = doc.get("servers").unwrap().as_object().unwrap();
    assert_eq!(servers.len(), 2);

    let stdio = servers.get("stdio-tool").unwrap();
    assert_eq!(stdio.get("type").unwrap().as_str().unwrap(), "stdio");
    assert_eq!(stdio.get("command").unwrap().as_str().unwrap(), "npx");

    let http = servers.get("http-tool").unwrap();
    assert_eq!(http.get("type").unwrap().as_str().unwrap(), "http");
    assert_eq!(http.get("url").unwrap().as_str().unwrap(), "https://example.com/mcp");

    // Verify inputs preserved
    let inputs = doc.get("inputs").unwrap().as_array().unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].get("id").unwrap().as_str().unwrap(), "my-secret-key");
}
