use std::path::Path;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::load_canonical;
use crate::logger::Logger;
use crate::targets::sync_all;

pub fn watch_and_sync(
    canonical_path: &Path,
    targets: &[String],
    dry_run: bool,
    logger: &Logger,
) -> Result<(), String> {
    let watch_dir = canonical_path
        .parent()
        .ok_or_else(|| format!("Cannot determine parent directory of {}", canonical_path.display()))?;

    let canonical_file_name = canonical_path
        .file_name()
        .ok_or_else(|| format!("Invalid file path {}", canonical_path.display()))?
        .to_os_string();

    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    watcher
        .watch(watch_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch directory {}: {}", watch_dir.display(), e))?;

    logger.info(&format!(
        "Watching {} for changes (debounce: 250ms)... Press Ctrl+C to stop.",
        canonical_path.display()
    ));

    let debounce_duration = Duration::from_millis(250);
    let mut last_event_time: Option<Instant> = None;

    loop {
        // If we have a pending event, wait with timeout for debounce
        let timeout = if let Some(last) = last_event_time {
            let elapsed = last.elapsed();
            if elapsed >= debounce_duration {
                // Debounce period expired, trigger sync
                last_event_time = None;
                logger.info("Change detected in canonical config, re-syncing...");
                match load_canonical(canonical_path) {
                    Ok(config) => {
                        let _ = sync_all(&config, targets, dry_run, logger);
                    }
                    Err(e) => {
                        logger.error(&format!("Failed to load canonical config: {}", e));
                    }
                }
                continue;
            } else {
                debounce_duration - elapsed
            }
        } else {
            Duration::from_secs(3600)
        };

        match rx.recv_timeout(timeout) {
            Ok(event) => {
                let affects_canonical = event.paths.iter().any(|p| {
                    p.file_name().map(|n| n == canonical_file_name).unwrap_or(false)
                });

                if affects_canonical {
                    last_event_time = Some(Instant::now());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout reached, loop will check if last_event_time needs firing
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                logger.error("Watcher channel disconnected, exiting watch mode");
                break;
            }
        }
    }

    Ok(())
}
