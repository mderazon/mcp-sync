use mcp_sync::Target;
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
    assert_eq!(servers.len(), 3); // 2 canonical + 1 preserved unmanaged

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

#[test]
fn test_safe_mode_unmanaged_server_preservation() {
    // 1. Test Zed preserves unmanaged server
    let mut tmp_zed = NamedTempFile::new().unwrap();
    let zed_initial = r#"{
  "context_servers": {
    "managed-tool": {
      "enabled": true,
      "url": "https://managed.com"
    },
    "local-only-zed-tool": {
      "enabled": true,
      "command": "custom-cli",
      "args": ["--local"]
    }
  }
}"#;
    tmp_zed.write_all(zed_initial.as_bytes()).unwrap();
    tmp_zed.flush().unwrap();

    let zed_target = mcp_sync::targets::ZedTarget::new(Some(tmp_zed.path().to_path_buf()));
    let canonical = r#"{
        "servers": {
            "managed-tool": {
                "url": "https://updated-managed.com"
            }
        }
    }"#;
    let config = parse_canonical_str(canonical).unwrap();
    zed_target.sync(&config, false).unwrap();

    let zed_updated = std::fs::read_to_string(tmp_zed.path()).unwrap();
    assert!(zed_updated.contains("\"local-only-zed-tool\""), "Unmanaged tool in Zed must be preserved");
    assert!(zed_updated.contains("https://updated-managed.com"));

    // 2. Test VSCode preserves unmanaged server
    let mut tmp_vscode = NamedTempFile::new().unwrap();
    let vscode_initial = r#"{
  "servers": {
    "managed-tool": { "type": "http", "url": "https://managed.com" },
    "local-vscode-tool": { "type": "stdio", "command": "vscode-tool" }
  },
  "inputs": []
}"#;
    tmp_vscode.write_all(vscode_initial.as_bytes()).unwrap();
    tmp_vscode.flush().unwrap();

    let vscode_target = VSCodeTarget::new(Some(tmp_vscode.path().to_path_buf()));
    vscode_target.sync(&config, false).unwrap();

    let vscode_updated = std::fs::read_to_string(tmp_vscode.path()).unwrap();
    assert!(vscode_updated.contains("\"local-vscode-tool\""), "Unmanaged tool in VSCode must be preserved");

    // 3. Test Antigravity preserves unmanaged server
    let mut tmp_ag = NamedTempFile::new().unwrap();
    let ag_initial = r#"{
  "mcpServers": {
    "managed-tool": { "serverUrl": "https://managed.com" },
    "local-ag-tool": { "command": "ag-tool" }
  }
}"#;
    tmp_ag.write_all(ag_initial.as_bytes()).unwrap();
    tmp_ag.flush().unwrap();

    let ag_target = AntigravityTarget::new(Some(tmp_ag.path().to_path_buf()));
    ag_target.sync(&config, false).unwrap();

    let ag_updated = std::fs::read_to_string(tmp_ag.path()).unwrap();
    assert!(ag_updated.contains("\"local-ag-tool\""), "Unmanaged tool in Antigravity must be preserved");
}

#[test]
fn test_opencode_target_generation_and_safe_mode() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "local-only": {
      "type": "local",
      "enabled": false,
      "command": ["custom-tool", "--flag"]
    }
  }
}"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = mcp_sync::targets::OpenCodeTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "stdio-tool": {
                "command": "npx",
                "args": ["-y", "pkg"],
                "env": { "FOO": "bar" }
            },
            "http-tool": {
                "url": "https://api.example.com/mcp"
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();
    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&updated).unwrap();

    // Verify $schema preserved
    assert_eq!(doc.get("$schema").unwrap().as_str().unwrap(), "https://opencode.ai/config.json");

    let mcp = doc.get("mcp").unwrap().as_object().unwrap();

    // Safe mode: local-only preserved
    assert!(mcp.contains_key("local-only"));
    let local_only = mcp.get("local-only").unwrap();
    assert_eq!(local_only.get("enabled").unwrap().as_bool().unwrap(), false);

    // Stdio tool formatted correctly
    let stdio = mcp.get("stdio-tool").unwrap();
    assert_eq!(stdio.get("type").unwrap().as_str().unwrap(), "local");
    assert_eq!(
        stdio.get("command").unwrap().as_array().unwrap(),
        &vec![serde_json::Value::String("npx".into()), serde_json::Value::String("-y".into()), serde_json::Value::String("pkg".into())]
    );
    assert_eq!(stdio.get("environment").unwrap().get("FOO").unwrap().as_str().unwrap(), "bar");
    assert_eq!(stdio.get("enabled").unwrap().as_bool().unwrap(), true);

    // HTTP tool formatted correctly
    let http = mcp.get("http-tool").unwrap();
    assert_eq!(http.get("type").unwrap().as_str().unwrap(), "remote");
    assert_eq!(http.get("url").unwrap().as_str().unwrap(), "https://api.example.com/mcp");
    assert_eq!(http.get("enabled").unwrap().as_bool().unwrap(), true);
}

#[test]
fn test_codex_target_generation_and_safe_mode() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"model = "gpt-6"
personality = "pragmatic"

[mcp_servers.disabled-srv]
command = "npx"
args = ["-y", "disabled-pkg"]
enabled = false

[mcp_servers.unmanaged-internal]
command = "/usr/lib/internal_repl"
args = []

[projects."/home/michael/my-project"]
trust_level = "trusted"
"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = mcp_sync::targets::CodexTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "disabled-srv": {
                "command": "npx",
                "args": ["-y", "updated-disabled-pkg"]
            },
            "new-stdio": {
                "command": "python",
                "args": ["server.py"],
                "env": { "PORT": "8080" }
            },
            "new-remote": {
                "url": "https://mcp.remote.com",
                "headers": { "X-Api-Key": "secret123" }
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();
    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: toml_edit::DocumentMut = updated.parse().unwrap();

    // Top-level keys preserved
    assert_eq!(doc["model"].as_str(), Some("gpt-6"));
    assert_eq!(doc["personality"].as_str(), Some("pragmatic"));

    // Other tables preserved
    assert!(doc.contains_key("projects"));

    let mcp = doc["mcp_servers"].as_table().unwrap();

    // Unmanaged server preserved
    assert!(mcp.contains_key("unmanaged-internal"));
    assert_eq!(
        mcp["unmanaged-internal"]["command"].as_str(),
        Some("/usr/lib/internal_repl")
    );

    // Smart merge: disabled-srv remains disabled
    let disabled_srv = &mcp["disabled-srv"];
    assert_eq!(disabled_srv["enabled"].as_bool(), Some(false));
    assert_eq!(
        disabled_srv["args"].as_array().unwrap().get(1).unwrap().as_str(),
        Some("updated-disabled-pkg")
    );

    // New stdio server formatted correctly
    let new_stdio = &mcp["new-stdio"];
    assert_eq!(new_stdio["command"].as_str(), Some("python"));
    assert!(new_stdio.get("enabled").is_none());
    assert_eq!(new_stdio["env"]["PORT"].as_str(), Some("8080"));

    // New remote server formatted correctly
    let new_remote = &mcp["new-remote"];
    assert_eq!(new_remote["url"].as_str(), Some("https://mcp.remote.com"));
    assert_eq!(
        new_remote["http_headers"]["X-Api-Key"].as_str(),
        Some("secret123")
    );
}

