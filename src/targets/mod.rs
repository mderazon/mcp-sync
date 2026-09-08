pub mod antigravity;
pub mod opencode;
pub mod vscode;
pub mod zed;

use crate::config::CanonicalConfig;
use crate::logger::Logger;

pub use antigravity::AntigravityTarget;
pub use opencode::OpenCodeTarget;
pub use vscode::VSCodeTarget;
pub use zed::ZedTarget;

/// Pluggable Target trait that all agent targets implement.
pub trait Target: Send + Sync {
    /// Identifier name for the target (e.g. "zed", "vscode", "antigravity", "opencode")
    fn name(&self) -> &'static str;

    /// Execute synchronization for this target
    fn sync(&self, config: &CanonicalConfig, dry_run: bool) -> Result<String, String>;
}

/// Registry of all supported targets.
/// Adding a new agent in the future only requires implementing `Target` and adding an entry here.
pub fn available_targets() -> Vec<Box<dyn Target>> {
    vec![
        Box::new(ZedTarget::new(None)),
        Box::new(VSCodeTarget::new(None)),
        Box::new(AntigravityTarget::new(None)),
        Box::new(OpenCodeTarget::new(None)),
    ]
}

pub fn sync_all(
    config: &CanonicalConfig,
    target_filter: &[String],
    dry_run: bool,
    logger: &Logger,
) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let targets = available_targets();

    let should_sync = |name: &str| -> bool {
        target_filter.is_empty() || target_filter.iter().any(|t| t == name)
    };

    let mut synced_targets = Vec::new();

    for target in targets {
        if should_sync(target.name()) {
            match target.sync(config, dry_run) {
                Ok(msg) => {
                    logger.info(&msg);
                    synced_targets.push(target.name());
                    messages.push(msg);
                }
                Err(e) => {
                    let err_msg = format!("[{}] Sync failed: {}", target.name(), e);
                    logger.error(&err_msg);
                    return Err(err_msg);
                }
            }
        }
    }

    let summary = format!(
        "Synced {} servers to targets: [{}]",
        config.servers.len(),
        synced_targets.join(", ")
    );
    logger.info(&summary);

    Ok(messages)
}
