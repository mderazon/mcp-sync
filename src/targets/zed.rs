use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde_json::{Map, Value};

use crate::config::CanonicalConfig;
use crate::fs_utils::atomic_write;

pub struct ZedTarget {
    path: PathBuf,
}

impl ZedTarget {
    pub fn new(path: Option<PathBuf>) -> Self {
        let p = path.unwrap_or_else(|| {
            dirs::config_dir()
                .map(|d| d.join("zed/settings.json"))
                .or_else(|| dirs::home_dir().map(|h| h.join(".config/zed/settings.json")))
                .unwrap_or_else(|| PathBuf::from(".config/zed/settings.json"))
        });
        Self { path: p }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Locate the span (start, end) of `"context_servers": { ... }` in a JSONC string.
    /// Returns (start_index, end_index) where start is at `"context_servers"`
    /// and end is right after the matching closing `}`.
    pub fn find_context_servers_span(content: &str) -> Option<(usize, usize)> {
        let bytes = content.as_bytes();
        let len = bytes.len();
        let target = b"\"context_servers\"";

        let mut i = 0;
        let mut in_string = false;
        let mut in_single_comment = false;
        let mut in_multi_comment = false;

        let mut found_key_start = None;

        while i < len {
            if in_single_comment {
                if bytes[i] == b'\n' {
                    in_single_comment = false;
                }
                i += 1;
                continue;
            }

            if in_multi_comment {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    in_multi_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }

            if in_string {
                if bytes[i] == b'\\' {
                    i += 2; // skip escaped character
                    continue;
                }
                if bytes[i] == b'"' {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            // Not in string or comment
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                in_single_comment = true;
                i += 2;
                continue;
            }
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                in_multi_comment = true;
                i += 2;
                continue;
            }

            if bytes[i] == b'"' {
                if bytes[i..].starts_with(target) {
                    found_key_start = Some(i);
                    i += target.len();
                    break;
                }
                in_string = true;
                i += 1;
                continue;
            }

            i += 1;
        }

        let start_pos = found_key_start?;

        // Find the opening `{`
        let mut brace_start = None;
        while i < len {
            if bytes[i] == b'{' {
                brace_start = Some(i);
                i += 1;
                break;
            }
            i += 1;
        }

        let _ = brace_start?;
        let mut brace_depth = 1;
        in_string = false;
        in_single_comment = false;
        in_multi_comment = false;

        while i < len && brace_depth > 0 {
            if in_single_comment {
                if bytes[i] == b'\n' {
                    in_single_comment = false;
                }
                i += 1;
                continue;
            }

            if in_multi_comment {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    in_multi_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }

            if in_string {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                in_single_comment = true;
                i += 2;
                continue;
            }
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                in_multi_comment = true;
                i += 2;
                continue;
            }

            if bytes[i] == b'"' {
                in_string = true;
                i += 1;
                continue;
            }

            if bytes[i] == b'{' {
                brace_depth += 1;
            } else if bytes[i] == b'}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    return Some((start_pos, i + 1));
                }
            }

            i += 1;
        }

        None
    }

    /// Extract existing enabled states for servers in current context_servers block.
    pub fn parse_existing_states(content: &str) -> BTreeMap<String, bool> {
        let mut states = BTreeMap::new();
        if let Some((start, end)) = Self::find_context_servers_span(content) {
            let block = &content[start..end];
            // Format into dummy valid JSON: { <block> }
            let wrap = format!("{{{}}}", block);
            // Basic strip comments and trailing commas to parse safely
            let cleaned = strip_jsonc_comments(&wrap);
            if let Ok(v) = serde_json::from_str::<Value>(&cleaned)
                && let Some(servers) = v.get("context_servers").and_then(|cs| cs.as_object())
            {
                for (name, s_val) in servers {
                    let enabled = s_val.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);
                    states.insert(name.clone(), enabled);
                }
            }
        }
        states
    }

    /// Render the `context_servers` block matching Zed's indentation style.
    pub fn render_context_servers(
        config: &CanonicalConfig,
        existing_states: &BTreeMap<String, bool>,
    ) -> String {
        let mut entries = Vec::new();

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
                obj.insert("url".to_string(), Value::String(url.to_string()));
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
            }

            if let Some(ref settings) = server.settings {
                obj.insert("settings".to_string(), settings.clone());
            }

            // Pretty print object with 4-space base indentation for Zed
            let val = Value::Object(obj);
            let rendered_obj = serde_json::to_string_pretty(&val).unwrap_or_else(|_| "{}".to_string());
            let indented = reindent(&rendered_obj, "    ");
            entries.push(format!("    \"{}\": {}", name, indented));
        }

        let body = entries.join(",\n");
        format!("\"context_servers\": {{\n{}\n  }}", body)
    }

    /// Sync the canonical config into Zed's settings.json
    pub fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String> {
        if !self.path.exists() {
            return Err(format!("Zed settings file not found at {}", self.path.display()));
        }

        let content = fs::read_to_string(&self.path)
            .map_err(|e| format!("Failed to read {}: {}", self.path.display(), e))?;

        let existing_states = Self::parse_existing_states(&content);
        let new_block = Self::render_context_servers(config, &existing_states);

        let new_content = if let Some((start, end)) = Self::find_context_servers_span(&content) {
            format!("{}{}{}", &content[..start], new_block, &content[end..])
        } else {
            // Insert before the last '}'
            if let Some(last_brace) = content.rfind('}') {
                let prefix = content[..last_brace].trim_end();
                let separator = if prefix.ends_with('{') || prefix.ends_with(',') {
                    "\n  "
                } else {
                    ",\n  "
                };
                format!("{}{}{}\n}}\n", prefix, separator, new_block)
            } else {
                return Err(format!("Invalid JSON in {}", self.path.display()));
            }
        };

        if dry_run {
            return Ok(format!("[zed] Would update {}", self.path.display()));
        }

        atomic_write(&self.path, &new_content)
            .map_err(|e| format!("Failed to write {}: {}", self.path.display(), e))?;

        Ok(format!("[zed] Updated {}", self.path.display()))
    }
}

/// Helper to re-indent JSON text lines after line 0
fn reindent(text: &str, prefix: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(prefix);
        }
        out.push_str(line);
    }
    out
}

/// Minimal comment & trailing comma cleanup for parsing existing states
fn strip_jsonc_comments(jsonc: &str) -> String {
    let mut out = String::with_capacity(jsonc.len());
    let bytes = jsonc.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_str = false;

    while i < len {
        if in_str {
            if bytes[i] == b'\\' && i + 1 < len {
                out.push(bytes[i] as char);
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_str = false;
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        if bytes[i] == b'"' {
            in_str = true;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    // Strip trailing commas before } or ]
    let mut result = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let n = chars.len();
    for j in 0..n {
        if chars[j] == ',' {
            // check next non-whitespace char
            let mut k = j + 1;
            while k < n && chars[k].is_whitespace() {
                k += 1;
            }
            if k < n && (chars[k] == '}' || chars[k] == ']') {
                continue; // skip trailing comma
            }
        }
        result.push(chars[j]);
    }

    result
}
