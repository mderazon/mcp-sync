pub mod config;
pub mod fs_utils;
pub mod logger;
pub mod targets;
pub mod watcher;

pub use config::{
    CanonicalConfig, ServerDefinition, load_canonical, parse_canonical_str, resolve_canonical_path,
};
pub use logger::Logger;
pub use targets::{
    AntigravityTarget, CodexTarget, OpenCodeTarget, Target, VSCodeTarget, ZedTarget,
    available_targets, sync_all,
};
pub use watcher::watch_and_sync;
