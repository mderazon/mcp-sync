use mcp_sync::config::parse_canonical_str;

#[test]
fn test_parse_servers_key() {
    let json = r#"{
        "servers": {
            "test-stdio": {
                "command": "npx",
                "args": ["-y", "test-server"],
                "env": { "TEST_VAR": "val" }
            },
            "test-remote": {
                "url": "https://mcp.example.com/sse"
            }
        }
    }"#;

    let config = parse_canonical_str(json).expect("should parse successfully");
    assert_eq!(config.servers.len(), 2);

    let stdio = config.servers.get("test-stdio").unwrap();
    assert_eq!(stdio.command.as_deref(), Some("npx"));
    assert_eq!(stdio.args.as_ref().unwrap(), &vec!["-y", "test-server"]);
    assert_eq!(stdio.env.as_ref().unwrap().get("TEST_VAR").unwrap(), "val");
    assert!(!stdio.is_remote());

    let remote = config.servers.get("test-remote").unwrap();
    assert_eq!(remote.get_url(), Some("https://mcp.example.com/sse"));
    assert!(remote.is_remote());
}

#[test]
fn test_parse_mcp_servers_key() {
    let json = r#"{
        "mcpServers": {
            "server-a": {
                "serverUrl": "https://remote.example.com"
            }
        }
    }"#;

    let config = parse_canonical_str(json).expect("should parse mcpServers key");
    assert_eq!(config.servers.len(), 1);
    let s = config.servers.get("server-a").unwrap();
    assert_eq!(s.get_url(), Some("https://remote.example.com"));
}

#[test]
fn test_parse_settings_and_inputs() {
    let json = r#"{
        "servers": {
            "with-settings": {
                "command": "npx",
                "settings": {
                    "api_key": "secret-123"
                }
            }
        },
        "inputs": [
            { "id": "token", "type": "promptString" }
        ]
    }"#;

    let config = parse_canonical_str(json).expect("should parse settings and inputs");
    let s = config.servers.get("with-settings").unwrap();
    assert!(s.settings.is_some());
    assert_eq!(
        s.settings
            .as_ref()
            .unwrap()
            .get("api_key")
            .unwrap()
            .as_str()
            .unwrap(),
        "secret-123"
    );
    assert!(config.inputs.is_some());
}
