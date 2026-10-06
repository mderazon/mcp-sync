use std::path::PathBuf;
use std::process::exit;

use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{Shell, generate};

use mcp_sync::config::{load_canonical, resolve_canonical_path};
use mcp_sync::logger::Logger;
use mcp_sync::targets::{available_targets, sync_all};
use mcp_sync::updater;
use mcp_sync::watcher::watch_and_sync;

#[derive(Parser, Debug)]
#[command(
    name = "mcp-sync",
    version,
    about = "Synchronize MCP server configurations from a single source of truth across AI coding clients.",
    after_help = "TARGETS:\n    zed          -> ~/.config/zed/settings.json (context_servers)\n    vscode       -> ~/.config/Code/User/mcp.json (servers)\n    antigravity  -> ~/.gemini/config/mcp_config.json (mcpServers)\n    opencode     -> ~/.config/opencode/opencode.json (mcp)\n    codex        -> ~/.codex/config.toml (mcp_servers)\n"
)]
struct Cli {
    /// Watch canonical config for changes and sync automatically
    #[arg(short = 'w', long = "watch")]
    watch: bool,

    /// Show what would be modified without writing files
    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,

    /// Suppress stdout logging (errors still written to stderr)
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Check for updates and self-update to latest GitHub release
    #[arg(short = 'u', long = "update")]
    update: bool,

    /// Comma-separated targets: zed, vscode, antigravity, opencode, codex (default: all)
    #[arg(short = 't', long = "target", value_delimiter = ',')]
    target: Vec<String>,

    /// Path to canonical config (default: ~/.config/mcp/servers.json)
    #[arg(short = 'c', long = "config", value_hint = ValueHint::FilePath)]
    config: Option<PathBuf>,

    /// Path to log file (default: ~/.local/state/mcp-sync/mcp-sync.log)
    #[arg(short = 'l', long = "log-file", value_hint = ValueHint::FilePath)]
    log_file: Option<PathBuf>,

    /// Generate shell completion script to stdout (bash, zsh, fish, powershell, elvish)
    #[arg(long = "completions", value_enum)]
    completions: Option<Shell>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum Commands {
    /// Check for updates and self-update to latest GitHub release
    Update,
    /// Generate shell completion script to stdout
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "mcp-sync", &mut std::io::stdout());
}

fn main() {
    let cli = Cli::parse();

    // Handle completions first if requested
    let shell = match (cli.completions, cli.command) {
        (Some(s), _) => Some(s),
        (_, Some(Commands::Completions { shell })) => Some(shell),
        _ => None,
    };

    if let Some(shell) = shell {
        print_completions(shell);
        exit(0);
    }

    // Handle update if requested
    if cli.update || matches!(cli.command, Some(Commands::Update)) {
        match updater::update() {
            Ok(_) => exit(0),
            Err(e) => {
                eprintln!("Update error: {}", e);
                exit(1);
            }
        }
    }

    // Validate targets dynamically from registered pluggable targets
    let valid_names: Vec<&'static str> = available_targets().iter().map(|t| t.name()).collect();
    for t in &cli.target {
        if !valid_names.contains(&t.as_str()) {
            eprintln!(
                "Invalid target '{}'. Valid targets are: {}",
                t,
                valid_names.join(", ")
            );
            exit(2);
        }
    }

    let logger = Logger::new(cli.log_file.as_deref(), cli.quiet);

    let canonical_path = match resolve_canonical_path(cli.config.as_deref()) {
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

    if cli.watch {
        if let Err(e) = watch_and_sync(&canonical_path, &cli.target, cli.dry_run, &logger) {
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

        match sync_all(&config, &cli.target, cli.dry_run, &logger) {
            Ok(_) => {
                if cli.dry_run && !cli.quiet {
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
