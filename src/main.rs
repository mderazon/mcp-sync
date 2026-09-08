use std::env;
use std::path::PathBuf;
use std::process::exit;

use mcp_sync::config::{load_canonical, resolve_canonical_path};
use mcp_sync::logger::Logger;
use mcp_sync::targets::{available_targets, sync_all};
use mcp_sync::watcher::watch_and_sync;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct CliArgs {
    watch: bool,
    dry_run: bool,
    targets: Vec<String>,
    config_path: Option<PathBuf>,
    log_path: Option<PathBuf>,
    quiet: bool,
}

impl CliArgs {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut watch = false;
        let mut dry_run = false;
        let mut targets = Vec::new();
        let mut config_path = None;
        let mut log_path = None;
        let mut quiet = false;

        let valid_names: Vec<&'static str> = available_targets()
            .iter()
            .map(|t| t.name())
            .collect();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--watch" | "-w" => watch = true,
                "--dry-run" | "-n" => dry_run = true,
                "--quiet" | "-q" => quiet = true,
                "--target" | "-t" => {
                    let val = args
                        .next()
                        .ok_or_else(|| "--target requires an argument".to_string())?;
                    targets.extend(
                        val.split(',')
                            .map(|s| s.trim().to_lowercase())
                            .filter(|s| !s.is_empty()),
                    );
                }
                "--config" | "-c" => {
                    let val = args
                        .next()
                        .ok_or_else(|| "--config requires a path argument".to_string())?;
                    config_path = Some(PathBuf::from(val));
                }
                "--log-file" | "-l" => {
                    let val = args
                        .next()
                        .ok_or_else(|| "--log-file requires a path argument".to_string())?;
                    log_path = Some(PathBuf::from(val));
                }
                "--help" | "-h" => {
                    print_help();
                    exit(0);
                }
                "--version" | "-v" | "-V" => {
                    println!("mcp-sync {}", VERSION);
                    exit(0);
                }
                unknown => {
                    return Err(format!("Unknown argument '{}'. Use --help for usage.", unknown));
                }
            }
        }

        // Validate targets dynamically from registered pluggable targets
        for t in &targets {
            if !valid_names.contains(&t.as_str()) {
                return Err(format!(
                    "Invalid target '{}'. Valid targets are: {}",
                    t,
                    valid_names.join(", ")
                ));
            }
        }

        Ok(CliArgs {
            watch,
            dry_run,
            targets,
            config_path,
            log_path,
            quiet,
        })
    }
}

fn print_help() {
    println!(
        r#"mcp-sync {VERSION}
Synchronize MCP server configurations from a single source of truth across AI coding clients.

USAGE:
    mcp-sync [OPTIONS]

OPTIONS:
    -w, --watch              Watch canonical config for changes and sync automatically
    -n, --dry-run            Show what would be modified without writing files
    -t, --target <TARGETS>   Comma-separated targets: zed, vscode, antigravity, opencode (default: all)
    -c, --config <PATH>      Path to canonical config (default: ~/.config/mcp/servers.json)
    -l, --log-file <PATH>    Path to log file (default: ~/.local/state/mcp-sync/mcp-sync.log)
    -q, --quiet              Suppress stdout logging (errors still written to stderr)
    -h, --help               Print help information
    -V, --version            Print version information

TARGETS:
    zed          -> ~/.config/zed/settings.json (context_servers)
    vscode       -> ~/.config/Code/User/mcp.json (servers)
    antigravity  -> ~/.gemini/config/mcp_config.json (mcpServers)
    opencode     -> ~/.config/opencode/opencode.json (mcp)
"#
    );
}

fn main() {
    let args = match CliArgs::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}", e);
            exit(2);
        }
    };

    let logger = Logger::new(args.log_path.as_deref(), args.quiet);

    let canonical_path = match resolve_canonical_path(args.config_path.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            logger.error(&e);
            exit(1);
        }
    };

    if !canonical_path.exists() {
        logger.error(&format!(
            "Canonical config file not found at {}",
            canonical_path.display()
        ));
        exit(1);
    }

    if args.watch {
        if let Err(e) = watch_and_sync(&canonical_path, &args.targets, args.dry_run, &logger) {
            logger.error(&format!("Watch error: {}", e));
            exit(1);
        }
    } else {
        let config = match load_canonical(&canonical_path) {
            Ok(c) => c,
            Err(e) => {
                logger.error(&format!("Failed to load config: {}", e));
                exit(1);
            }
        };

        match sync_all(&config, &args.targets, args.dry_run, &logger) {
            Ok(_) => {
                if args.dry_run && !args.quiet {
                    println!("(dry-run complete, no files were modified)");
                }
            }
            Err(e) => {
                logger.error(&format!("Sync failed: {}", e));
                exit(1);
            }
        }
    }
}
