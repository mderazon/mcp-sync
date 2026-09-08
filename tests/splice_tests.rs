use mcp_sync::config::parse_canonical_str;
use mcp_sync::targets::ZedTarget;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_find_context_servers_span_clean() {
    let text = r#"{
  "theme": "One Dark",
  "context_servers": {
    "server-1": {
      "enabled": true,
      "url": "https://example.com"
    }
  },
  "git_panel": {
    "dock": "left"
  }
}"#;

    let span = ZedTarget::find_context_servers_span(text);
    assert!(span.is_some());
    let (start, end) = span.unwrap();
    let extracted = &text[start..end];
    assert!(extracted.starts_with("\"context_servers\""));
    assert!(extracted.ends_with('}'));
    assert!(extracted.contains("server-1"));
}

#[test]
fn test_find_context_servers_span_with_comments_and_braces_in_strings() {
    let text = r#"{
  // This is a comment before
  /* Multi-line
     comment with "context_servers" fake key
  */
  "context_servers": {
    "server-complex": {
      "enabled": false,
      "command": "echo",
      "args": ["{not_a_brace}", "another } fake brace"]
    }
  },
  "other_key": true
}"#;

    let span = ZedTarget::find_context_servers_span(text);
    assert!(span.is_some());
    let (start, end) = span.unwrap();
    let extracted = &text[start..end];
    assert!(extracted.starts_with("\"context_servers\""));
    assert!(extracted.ends_with('}'));
    assert!(extracted.contains("server-complex"));
    assert!(extracted.contains("not_a_brace"));
}

#[test]
fn test_zed_sync_file_splicing() {
    let mut tmp = NamedTempFile::new().unwrap();
    let initial_content = r#"{
  "theme": "One Dark",
  "context_servers": {
    "old-server": {
      "enabled": false,
      "url": "https://old.com"
    }
  },
  "git_panel": {
    "dock": "left"
  }
}
"#;
    tmp.write_all(initial_content.as_bytes()).unwrap();
    tmp.flush().unwrap();

    let target = ZedTarget::new(Some(tmp.path().to_path_buf()));

    let canonical = r#"{
        "servers": {
            "old-server": {
                "url": "https://updated-old.com"
            },
            "brand-new": {
                "command": "npx",
                "args": ["-y", "brand-new-pkg"]
            }
        }
    }"#;
    let config = parse_canonical_str(canonical).unwrap();

    let res = target.sync(&config, false);
    assert!(res.is_ok());

    let updated = std::fs::read_to_string(tmp.path()).unwrap();

    // Verify surrounding settings are intact
    assert!(updated.contains("\"theme\": \"One Dark\""));
    assert!(updated.contains("\"git_panel\""));

    // Verify smart merge: old-server preserved its enabled: false state!
    assert!(updated.contains("\"old-server\""));
    assert!(updated.contains("\"enabled\": false"));
    assert!(updated.contains("https://updated-old.com"));

    // Verify brand-new server defaulted to enabled: true
    assert!(updated.contains("\"brand-new\""));
    assert!(updated.contains("\"enabled\": true"));
}
