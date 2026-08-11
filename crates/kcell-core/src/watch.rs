//! Cell directory watch — default poll (zero deps); optional `notify` feature wakes rescans.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::control::{call_unix, ControlRequest};
use crate::error::Result;
use crate::package::sha256_hex;

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub roots: Vec<PathBuf>,
    /// Poll interval, or notify debounce floor (ms).
    pub interval_ms: u64,
    pub auto_bind: bool,
    /// Prefer OS file events when feature `notify` is enabled.
    pub use_notify: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedCell {
    pub dir: PathBuf,
    pub name: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchAction {
    Load { dir: PathBuf, replace: bool },
    Unload { name: String },
}

/// Scan watch roots for immediate child dirs containing `cell.yaml`.
pub fn scan_watch_roots(roots: &[PathBuf]) -> BTreeMap<PathBuf, WatchedCell> {
    let mut out = BTreeMap::new();
    for root in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let yaml = dir.join("cell.yaml");
            if !yaml.is_file() {
                continue;
            }
            let Ok(bytes) = std::fs::read(&yaml) else {
                continue;
            };
            let Ok(manifest) = crate::manifest::load_cell_from_path(&yaml) else {
                continue;
            };
            let digest = format!("sha256:{}", sha256_hex(&bytes));
            out.insert(
                dir.clone(),
                WatchedCell {
                    dir,
                    name: manifest.metadata.name,
                    digest,
                },
            );
        }
    }
    out
}

/// Diff two scans into load/unload actions.
pub fn diff_watch(
    prev: &BTreeMap<PathBuf, WatchedCell>,
    next: &BTreeMap<PathBuf, WatchedCell>,
) -> Vec<WatchAction> {
    let mut actions = Vec::new();
    for (path, cell) in next {
        match prev.get(path) {
            None => actions.push(WatchAction::Load {
                dir: path.clone(),
                replace: false,
            }),
            Some(old) if old.digest != cell.digest => actions.push(WatchAction::Load {
                dir: path.clone(),
                replace: true,
            }),
            _ => {}
        }
    }
    for (path, cell) in prev {
        if !next.contains_key(path) {
            actions.push(WatchAction::Unload {
                name: cell.name.clone(),
            });
        }
    }
    actions
}

fn wait_for_socket(socket: &Path) -> bool {
    for _ in 0..100 {
        if socket.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    socket.exists()
}

fn apply_diff_to_socket(
    socket: &Path,
    config: &WatchConfig,
    state: &mut BTreeMap<PathBuf, WatchedCell>,
) {
    let next = scan_watch_roots(&config.roots);
    let actions = diff_watch(state, &next);
    for action in actions {
        let req = match &action {
            WatchAction::Load { dir, replace } => {
                ControlRequest::load(dir.display().to_string(), *replace)
            }
            WatchAction::Unload { name } => ControlRequest::unload(name.clone()),
        };
        let _ = call_unix(socket, req);
        if config.auto_bind && matches!(action, WatchAction::Load { .. }) {
            let _ = call_unix(socket, ControlRequest::auto_bind(true));
        }
    }
    *state = next;
}

/// Background watcher: baseline snapshot, then poll and/or notify-wake rescans.
#[cfg(unix)]
pub fn spawn_watch_thread(socket: PathBuf, config: WatchConfig) {
    if config.use_notify {
        #[cfg(feature = "notify")]
        {
            spawn_notify_watch_thread(socket, config);
            return;
        }
        #[cfg(not(feature = "notify"))]
        {
            eprintln!(
                "kcell watch: --watch-notify requested but binary built without feature `notify`; using poll"
            );
        }
    }
    spawn_poll_watch_thread(socket, config);
}

#[cfg(unix)]
fn spawn_poll_watch_thread(socket: PathBuf, config: WatchConfig) {
    thread::spawn(move || {
        let interval = Duration::from_millis(config.interval_ms.max(200));
        if !wait_for_socket(&socket) {
            return;
        }
        let mut state = scan_watch_roots(&config.roots);
        loop {
            thread::sleep(interval);
            if !socket.exists() {
                break;
            }
            apply_diff_to_socket(&socket, &config, &mut state);
        }
    });
}

#[cfg(all(unix, feature = "notify"))]
fn spawn_notify_watch_thread(socket: PathBuf, config: WatchConfig) {
    use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    thread::spawn(move || {
        if !wait_for_socket(&socket) {
            return;
        }
        let debounce = Duration::from_millis(config.interval_ms.max(200));
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            NotifyConfig::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("kcell watch: notify init failed ({e}); falling back to poll");
                spawn_poll_watch_thread(socket, config);
                return;
            }
        };

        for root in &config.roots {
            if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
                eprintln!(
                    "kcell watch: cannot watch {}: {e}",
                    root.display()
                );
            }
        }

        let mut state = scan_watch_roots(&config.roots);
        let mut pending: Option<Instant> = None;

        loop {
            if !socket.exists() {
                break;
            }
            let timeout = pending
                .map(|t| {
                    let elapsed = t.elapsed();
                    if elapsed >= debounce {
                        Duration::from_millis(0)
                    } else {
                        debounce - elapsed
                    }
                })
                .unwrap_or(Duration::from_secs(3600));

            match rx.recv_timeout(timeout) {
                Ok(Ok(_event)) => {
                    pending = Some(Instant::now());
                }
                Ok(Err(e)) => {
                    eprintln!("kcell watch: notify error: {e}");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if pending.take().is_some() {
                        apply_diff_to_socket(&socket, &config, &mut state);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if let Some(t) = pending {
                if t.elapsed() >= debounce {
                    pending = None;
                    apply_diff_to_socket(&socket, &config, &mut state);
                }
            }
        }
    });
}

#[cfg(not(unix))]
pub fn spawn_watch_thread(_socket: PathBuf, _config: WatchConfig) {}

/// One-shot apply of watch actions (for tests).
pub fn apply_watch_actions(socket: &Path, actions: &[WatchAction]) -> Result<()> {
    for action in actions {
        let req = match action {
            WatchAction::Load { dir, replace } => {
                ControlRequest::load(dir.display().to_string(), *replace)
            }
            WatchAction::Unload { name } => ControlRequest::unload(name.clone()),
        };
        let resp = call_unix(socket, req)?;
        if !resp.ok {
            return Err(crate::error::Error::Validation(
                resp.error.unwrap_or_else(|| "watch action failed".into()),
            ));
        }
    }
    Ok(())
}

/// Whether this build includes the `notify` Cargo feature.
pub fn notify_feature_enabled() -> bool {
    cfg!(feature = "notify")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_detects_add_change_remove() {
        let mut prev = BTreeMap::new();
        prev.insert(
            PathBuf::from("/c/a"),
            WatchedCell {
                dir: PathBuf::from("/c/a"),
                name: "a".into(),
                digest: "sha256:1".into(),
            },
        );
        prev.insert(
            PathBuf::from("/c/b"),
            WatchedCell {
                dir: PathBuf::from("/c/b"),
                name: "b".into(),
                digest: "sha256:2".into(),
            },
        );

        let mut next = BTreeMap::new();
        next.insert(
            PathBuf::from("/c/a"),
            WatchedCell {
                dir: PathBuf::from("/c/a"),
                name: "a".into(),
                digest: "sha256:1-changed".into(),
            },
        );
        next.insert(
            PathBuf::from("/c/c"),
            WatchedCell {
                dir: PathBuf::from("/c/c"),
                name: "c".into(),
                digest: "sha256:3".into(),
            },
        );

        let actions = diff_watch(&prev, &next);
        assert!(actions
            .iter()
            .any(|a| matches!(a, WatchAction::Load { replace: true, .. })));
        assert!(actions
            .iter()
            .any(|a| matches!(a, WatchAction::Load { replace: false, .. })));
        assert!(actions
            .iter()
            .any(|a| matches!(a, WatchAction::Unload { name } if name == "b")));
    }

    #[test]
    fn scan_finds_repo_cells() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cells");
        let found = scan_watch_roots(&[root]);
        assert!(found.values().any(|c| c.name == "echo-cell"));
    }
}
