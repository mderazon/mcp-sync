# mcp-sync

A lightweight, fast, and idiomatic Rust CLI tool to synchronize Model Context Protocol (MCP) server definitions from a single canonical configuration file to multiple AI clients:
- **Zed** (`~/.config/zed/settings.json`)
- **VSCode (Native Copilot Chat)** (`~/.config/Code/User/mcp.json`)
- **Antigravity** (`~/.gemini/config/mcp_config.json`)

## Why `mcp-sync`?

Each AI code editor and assistant implements MCP with slightly different configuration schemas:
- **Zed** uses a JSONC `settings.json` file with a `context_servers` key, supporting per-server `settings` and `enabled: true/false`.
- **VSCode** uses an `mcp.json` file under User profile storage with `servers`, requiring explicit `type: "stdio"` or `type: "http"`.
- **Antigravity** uses `mcp_config.json` with `mcpServers`, using `serverUrl` for remote endpoints and `disabled: true` for inactive tools.

`mcp-sync` solves this by giving you a **single source of truth** for your MCP server definitions, automatically transforming and syncing them with zero configuration drift.

## Key Features

- **Smart Merge**: Keeps server definitions synchronized from canonical config, while preserving each application's local enabled/disabled state (toggled via UI or CLI).
- **Zero Config Pollution**: Uses atomic file replacements (`.tmp -> rename`). Never leaves dangling `.bak` files behind.
- **Safe JSONC Splicing**: For Zed, uses balanced-brace parsing to splice only the `context_servers` block, preserving all other settings, comments, and structure intact.
- **Watch Mode**: `--watch` monitors the canonical file directory using inotify with a 250ms debounce window.
- **Cron-Safe**: Pure, idempotent one-shot command returning exit code 0 on success.
- **Concise Logging**: Logs clean, non-chatty sync events to `~/.local/state/mcp-sync/mcp-sync.log`.

---

## Canonical Configuration Format

By default, `mcp-sync` reads `~/.config/mcp/servers.json` (or `config.json` / `mcp.json`).

The file can use either `"servers"` or `"mcpServers"` as the top-level key:

```json
{
  "servers": {
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@latest"]
    },
    "context7": {
      "command": "context7-mcp",
      "args": ["--api-key", "${CONTEXT7_API_KEY}"],
      "env": {
        "CONTEXT7_API_KEY": "your-api-key"
      },
      "settings": {
        "context7_api_key": "your-api-key"
      }
    },
    "astro-docs": {
      "url": "https://mcp.docs.astro.build/mcp"
    }
  }
}
```

### Supported Server Fields

| Field | Type | Description |
| :--- | :--- | :--- |
| `command` | `string` | Executable for local stdio server |
| `args` | `string[]` | Arguments passed to the executable |
| `env` | `object` | Environment variables for the server process |
| `url` / `serverUrl` | `string` | Endpoint URL for remote HTTP/SSE servers |
| `type` | `string` | Optional (`"stdio"` or `"http"`). Inferred automatically if omitted. |
| `settings` | `object` | Optional client settings passed to tools that support them (e.g. Zed). |

---

## Installation

### From Source

```bash
cargo install --path .
```

Or build the release binary manually:

```bash
cargo build --release
cp target/release/mcp-sync ~/.local/bin/mcp-sync
```

---

## Usage

### One-shot Sync

Sync all configured targets:

```bash
mcp-sync
```

### Dry Run

Preview changes without modifying any files:

```bash
mcp-sync --dry-run
```

### Target Subset

Sync to specific targets only:

```bash
mcp-sync --target zed,antigravity
```

### Continuous Watch Mode

Run continuously in the background to sync whenever `servers.json` changes:

```bash
mcp-sync --watch
```

### Command-Line Options

```text
mcp-sync 0.1.0
Synchronize MCP server configurations from a single source of truth to Zed, VSCode, and Antigravity.

USAGE:
    mcp-sync [OPTIONS]

OPTIONS:
    -w, --watch              Watch canonical config for changes and sync automatically
    -n, --dry-run            Show what would be modified without writing files
    -t, --target <TARGETS>   Comma-separated targets: zed, vscode, antigravity (default: all)
    -c, --config <PATH>      Path to canonical config (default: ~/.config/mcp/servers.json)
    -l, --log-file <PATH>    Path to log file (default: ~/.local/state/mcp-sync/mcp-sync.log)
    -q, --quiet              Suppress stdout logging (errors still written to stderr)
    -h, --help               Print help information
    -V, --version            Print version information
```

---

## Automation

### Systemd User Service (Watch Mode)

To run `mcp-sync` as a user daemon in the background:

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

Enable and start:
```bash
systemctl --user daemon-reload
systemctl --user enable --now mcp-sync.service
```

### Cron Setup

Alternatively, run once per hour via crontab:

```bash
crontab -e
```

Add the following entry:
```cron
0 * * * * ~/.local/bin/mcp-sync --quiet
```

---

## License

MIT License. See [LICENSE](./LICENSE) for details.
