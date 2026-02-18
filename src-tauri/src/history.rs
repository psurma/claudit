use crate::log;
use crate::usage_api::UsageData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const MAX_AGE_SECS: i64 = 7 * 24 * 3600; // 7 days

static HISTORY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub timestamp: i64,
    pub buckets: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageHistory {
    pub snapshots: Vec<UsageSnapshot>,
}

fn get_history_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    match app.path().app_data_dir() {
        Ok(dir) => {
            if !dir.exists() {
                let _ = fs::create_dir_all(&dir);
            }
            Some(dir.join("usage_history.json"))
        }
        Err(e) => {
            log(&format!("history: failed to get app data dir: {}", e));
            None
        }
    }
}

const LABEL_MIGRATIONS: &[(&str, &str)] = &[
    ("Session (5hr rolling)", "Current session"),
    ("Weekly All Models", "Current week (all models)"),
    ("Weekly Sonnet", "Current week (Sonnet only)"),
    ("Weekly Opus", "Current week (Opus only)"),
];

fn migrate_labels(history: &mut UsageHistory) {
    for snapshot in &mut history.snapshots {
        for &(old, new) in LABEL_MIGRATIONS {
            if let Some(val) = snapshot.buckets.remove(old) {
                snapshot.buckets.insert(new.to_string(), val);
            }
        }
    }
}

/// Set restrictive file permissions (0600) on Unix systems.
#[cfg(unix)]
fn set_owner_only_perms(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_owner_only_perms(_path: &PathBuf) {}

pub fn load_history(app: &tauri::AppHandle) -> UsageHistory {
    let path = match get_history_path(app) {
        Some(p) => p,
        None => return UsageHistory { snapshots: vec![] },
    };

    match fs::read_to_string(&path) {
        Ok(contents) => {
            let mut history: UsageHistory = serde_json::from_str(&contents).unwrap_or_else(|e| {
                log(&format!("history: parse error: {}", e));
                UsageHistory { snapshots: vec![] }
            });
            migrate_labels(&mut history);
            history
        }
        Err(_) => UsageHistory { snapshots: vec![] },
    }
}

pub fn save_snapshot(app: &tauri::AppHandle, usage: &UsageData) {
    let lock = HISTORY_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap();

    let path = match get_history_path(app) {
        Some(p) => p,
        None => return,
    };

    let mut history = load_history(app);

    let now = chrono::Utc::now().timestamp();
    let mut buckets = HashMap::new();
    for limit in &usage.limits {
        buckets.insert(limit.label.clone(), limit.usage_pct);
    }

    history.snapshots.push(UsageSnapshot {
        timestamp: now,
        buckets,
    });

    // Prune entries older than 7 days
    let cutoff = now - MAX_AGE_SECS;
    history.snapshots.retain(|s| s.timestamp >= cutoff);

    match serde_json::to_string(&history) {
        Ok(json) => {
            // Atomic write: write to temp file, then rename
            let tmp_path = path.with_extension("json.tmp");
            if let Err(e) = fs::write(&tmp_path, &json) {
                log(&format!("history: write error: {}", e));
                return;
            }
            set_owner_only_perms(&tmp_path);
            if let Err(e) = fs::rename(&tmp_path, &path) {
                log(&format!("history: rename error: {}", e));
            }
        }
        Err(e) => log(&format!("history: serialize error: {}", e)),
    }
}
