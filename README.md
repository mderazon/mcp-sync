# mcp-sync

Single source of truth for MCP server configurations across **Zed**, **VSCode**, and **Antigravity**.

## The Problem

Different AI editors and assistants use different JSON schemas for the exact same Model Context Protocol (MCP) servers:

| Client | Path | Root Key | Remote Key | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **Zed** | `~/.config/zed/settings.json` | `context_servers` | `url` | JSONC with comments, per-server `settings` |
| **VSCode** | `~/.config/Code/User/mcp.json` | `servers` | `url` | Requires explicit `type: "stdio" \| "http"` |
| **Antigravity** | `~/.gemini/config/mcp_config.json` | `mcpServers` | `serverUrl` | Native `serverUrl`, supports `disabled: true` |

`mcp-sync` reads a single canonical config (`~/.config/mcp/servers.json`) and generates the native configs for all three targets.

## Features

- **Smart Merge**: Syncs server definitions (commands, args, env, URLs) while preserving each app's native enabled/disabled state.
- **Safe Zed Splicing**: Balanced-brace JSONC parser replaces only the `context_servers` block, preserving all other settings, custom comments, and formatting.
- **Atomic Writes**: Uses temporary files and atomic `rename`—no partial writes, no `.bak` file clutter.
- **Ultra Light Watch Mode**: Inotify-based `--watch` with 250ms debounce (~2.5 MB RAM, 0% CPU idle).
- **Cron-Safe**: Idempotent CLI, exits 0 on success. Logs to `~/.local/state/mcp-sync/mcp-sync.log`.

## Install

```bash
cargo install --path .
```

Or build the release binary directly:

```bash
cargo build --release
cp target/release/mcp-sync ~/.local/bin/mcp-sync
```

## Canonical Config (`~/.config/mcp/servers.json`)

Supports both `"servers"` and `"mcpServers"` as the root key:

```json
{
  "servers": {
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"]
    },
    "context7": {
      "command": "context7-mcp",
      "args": ["--api-key", "ctx7sk-..."],
      "env": {
        "CONTEXT7_API_KEY": "ctx7sk-..."
      },
      "settings": {
        "context7_api_key": "ctx7sk-..."
      }
    },
    "astro-docs": {
      "url": "https://mcp.docs.astro.build/mcp"
    }
  }
}
```

## Usage

```bash
# Sync all targets once
mcp-sync

# Preview changes without modifying files
mcp-sync --dry-run

# Sync specific targets only
mcp-sync --target zed,antigravity

# Continuous watch mode
mcp-sync --watch

# Quiet mode (ideal for cron)
mcp-sync --quiet
```

### CLI Flags

- `-w, --watch`: Watch `~/.config/mcp/` directory for changes and re-sync automatically.
- `-n, --dry-run`: Preview changes without modifying target files.
- `-t, --target <list>`: Comma-separated target subset (`zed`, `vscode`, `antigravity`).
- `-c, --config <path>`: Custom canonical config path (default: `~/.config/mcp/servers.json`).
- `-l, --log-file <path>`: Custom log path (default: `~/.local/state/mcp-sync/mcp-sync.log`).
- `-q, --quiet`: Suppress stdout (errors still logged to stderr and file).
- `-h, --help`: Show usage.
- `-V, --version`: Print version.

## Background Automation

### Systemd User Service (Watch Mode)

`~/.config/systemd/user/mcp-sync.service`:

```ini
[Unit]
Description=MCP Server Config Synchronizer
After=default.target

[Service]
ExecStart=%h/.local/bin/mcp-sync --watch --quiet
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now mcp-sync.service
```

### Cron (Hourly)

```cron
0 * * * * ~/.local/bin/mcp-sync --quiet
```

## License

[MIT](./LICENSE)
