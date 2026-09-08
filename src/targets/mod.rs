pub mod antigravity;
pub mod vscode;
pub mod zed;

use crate::config::CanonicalConfig;
use crate::logger::Logger;

pub use antigravity::AntigravityTarget;
pub use vscode::VSCodeTarget;
pub use zed::ZedTarget;

pub fn sync_all(
    config: &CanonicalConfig,
    targets: &[String],
    dry_run: bool,
    logger: &Logger,
) -> Result<Vec<String>, String> {
    let mut messages = Vec::new();
    let want = |t: &str| -> bool {
        targets.is_empty() || targets.iter().any(|target| target == t)
    };

    if want("zed") {
        let zed = ZedTarget::new(None);
        match zed.sync(config, dry_run) {
            Ok(msg) => {
                logger.info(&msg);
                messages.push(msg);
            }
            Err(e) => {
                let err_msg = format!("[zed] Sync failed: {}", e);
                logger.error(&err_msg);
                return Err(err_msg);
            }
        }
    }

    if want("vscode") {
        let vscode = VSCodeTarget::new(None);
        match vscode.sync(config, dry_run) {
            Ok(msg) => {
                logger.info(&msg);
                messages.push(msg);
            }
            Err(e) => {
                let err_msg = format!("[vscode] Sync failed: {}", e);
                logger.error(&err_msg);
                return Err(err_msg);
            }
        }
    }

    if want("antigravity") {
        let ag = AntigravityTarget::new(None);
        match ag.sync(config, dry_run) {
            Ok(msg) => {
                logger.info(&msg);
                messages.push(msg);
            }
            Err(e) => {
                let err_msg = format!("[antigravity] Sync failed: {}", e);
                logger.error(&err_msg);
                return Err(err_msg);
            }
        }
    }

    let target_list = if targets.is_empty() {
        "zed, vscode, antigravity".to_string()
    } else {
        targets.join(", ")
    };
    let summary = format!(
        "Synced {} servers to targets: [{}]",
        config.servers.len(),
        target_list
    );
    logger.info(&summary);

    Ok(messages)
}
