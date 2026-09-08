pub mod config;
pub mod fs_utils;
pub mod logger;
pub mod targets;
pub mod watcher;

pub use config::{load_canonical, parse_canonical_str, resolve_canonical_path, CanonicalConfig, ServerDefinition};
pub use logger::Logger;
pub use targets::{available_targets, sync_all, AntigravityTarget, OpenCodeTarget, Target, VSCodeTarget, ZedTarget};
pub use watcher::watch_and_sync;
