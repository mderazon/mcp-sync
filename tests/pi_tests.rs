use mcp_sync::config::parse_canonical_str;
use mcp_sync::targets::PiTarget;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_pi_target_generation_and_safe_mode() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"{
  "autoEnableCodemode": false,
  "mcpServers": {
    "disabled-srv": {
      "command": "npx",
      "args": ["-y", "old-pkg"],
      "enabled": false,
      "exposure": "direct",
      "timeout": 120
    },
    "oauth-srv": {
      "url": "https://oauth.example.com/mcp",
      "oauth": { "clientName": "pi" }
    },
    "switcher": {
      "url": "https://old.example.com/mcp",
      "headers": { "Authorization": "Bearer old" },
      "oauth": { "clientName": "pi" },
      "exposure": "deferred"
    },
    "reenabled": {
      "command": "old-tool",
      "enabled": false
    },
    "local-only": {
      "command": "local-tool",
      "args": ["--flag"]
    }
  }
}"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = PiTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "disabled-srv": {
                "command": "npx",
                "args": ["-y", "new-pkg"]
            },
            "oauth-srv": {
                "url": "https://oauth.example.com/v2/mcp",
                "headers": { "X-Api-Key": "secret123" }
            },
            "switcher": {
                "command": "switcher-mcp",
                "args": ["serve"]
            },
            "reenabled": {
                "command": "new-tool",
                "enabled": true
            },
            "new-stdio": {
                "command": "python",
                "args": ["server.py"],
                "env": { "PORT": "8080" },
                "env_vars": ["GOOGLE_API_KEY"]
            },
            "new-remote": {
                "url": "https://mcp.remote.com",
                "type": "http",
                "enabled": false
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();
    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&updated).unwrap();

    // Root-level Pi options are preserved
    assert_eq!(
        doc.get("autoEnableCodemode").unwrap().as_bool(),
        Some(false)
    );

    let servers = doc.get("mcpServers").unwrap().as_object().unwrap();

    // Unmanaged server preserved
    let local_only = servers.get("local-only").unwrap();
    assert_eq!(
        local_only.get("command").unwrap().as_str(),
        Some("local-tool")
    );

    // Smart merge: disabled state and Pi-specific extras are preserved
    let disabled = servers.get("disabled-srv").unwrap();
    assert_eq!(disabled.get("enabled").unwrap().as_bool(), Some(false));
    assert_eq!(disabled.get("exposure").unwrap().as_str(), Some("direct"));
    assert_eq!(disabled.get("timeout").unwrap().as_i64(), Some(120));
    assert_eq!(
        disabled.get("args").unwrap().as_array().unwrap()[1]
            .as_str()
            .unwrap(),
        "new-pkg"
    );

    // OAuth config survives a URL update
    let oauth = servers.get("oauth-srv").unwrap();
    assert_eq!(
        oauth.get("url").unwrap().as_str(),
        Some("https://oauth.example.com/v2/mcp")
    );
    assert_eq!(
        oauth.get("headers").unwrap().get("X-Api-Key").unwrap(),
        "secret123"
    );
    assert_eq!(
        oauth
            .get("oauth")
            .unwrap()
            .get("clientName")
            .unwrap()
            .as_str(),
        Some("pi")
    );

    // Switching HTTP -> stdio removes URL-only fields but keeps exposure
    let switcher = servers.get("switcher").unwrap();
    assert_eq!(
        switcher.get("command").unwrap().as_str(),
        Some("switcher-mcp")
    );
    assert!(switcher.get("url").is_none());
    assert!(switcher.get("headers").is_none());
    assert!(switcher.get("oauth").is_none());
    assert_eq!(switcher.get("exposure").unwrap().as_str(), Some("deferred"));

    // Canonical explicit enabled wins over the existing state
    let reenabled = servers.get("reenabled").unwrap();
    assert!(reenabled.get("enabled").is_none());

    // New stdio server: env_vars become ${VAR} references
    let stdio = servers.get("new-stdio").unwrap();
    assert_eq!(stdio.get("command").unwrap().as_str(), Some("python"));
    assert!(stdio.get("enabled").is_none());
    assert_eq!(
        stdio
            .get("env")
            .unwrap()
            .get("GOOGLE_API_KEY")
            .unwrap()
            .as_str(),
        Some("${GOOGLE_API_KEY}")
    );
    assert_eq!(
        stdio.get("env").unwrap().get("PORT").unwrap().as_str(),
        Some("8080")
    );

    // New remote server
    let remote = servers.get("new-remote").unwrap();
    assert_eq!(
        remote.get("url").unwrap().as_str(),
        Some("https://mcp.remote.com")
    );
    assert_eq!(remote.get("type").unwrap().as_str(), Some("http"));
    assert_eq!(remote.get("enabled").unwrap().as_bool(), Some(false));
}

#[test]
fn test_pi_target_type_handling_and_dry_run() {
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(br#"{"mcpServers":{"kept":{"command":"keep"}}}"#)
        .unwrap();
    tmp.flush().unwrap();

    let target = PiTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "remote-sse": {
                "url": "https://example.com/sse",
                "type": "sse"
            },
            "remote-http": {
                "serverUrl": "https://example.com/mcp",
                "type": "streamable-http"
            },
            "stdio": {
                "command": "node",
                "type": "stdio"
            },
            "mismatched": {
                "command": "x",
                "type": "http"
            }
        }
    }"#;

    let config = parse_canonical_str(canonical).unwrap();

    let dry_run_msg = target.sync(&config, true).unwrap();
    assert!(dry_run_msg.contains("Would write"));
    let untouched = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(untouched.contains("\"kept\""));

    target.sync(&config, false).unwrap();

    let updated = std::fs::read_to_string(tmp.path()).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&updated).unwrap();
    let servers = doc.get("mcpServers").unwrap().as_object().unwrap();

    // Pi does not support legacy SSE, so the type is omitted
    assert!(servers.get("remote-sse").unwrap().get("type").is_none());
    assert_eq!(
        servers
            .get("remote-http")
            .unwrap()
            .get("type")
            .unwrap()
            .as_str(),
        Some("streamable-http")
    );
    assert_eq!(
        servers.get("stdio").unwrap().get("type").unwrap().as_str(),
        Some("stdio")
    );
    // A transport/type mismatch is not written
    assert!(servers.get("mismatched").unwrap().get("type").is_none());

    // Unmanaged server still preserved
    assert!(servers.contains_key("kept"));
}
