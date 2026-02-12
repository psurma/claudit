use crate::log;
use crate::usage_api::UsageData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const MAX_AGE_SECS: i64 = 7 * 24 * 3600; // 7 days

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

pub fn load_history(app: &tauri::AppHandle) -> UsageHistory {
    let path = match get_history_path(app) {
        Some(p) => p,
        None => return UsageHistory { snapshots: vec![] },
    };

    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log(&format!("history: parse error: {}", e));
            UsageHistory { snapshots: vec![] }
        }),
        Err(_) => UsageHistory { snapshots: vec![] },
    }
}

pub fn save_snapshot(app: &tauri::AppHandle, usage: &UsageData) {
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
            if let Err(e) = fs::write(&path, json) {
                log(&format!("history: write error: {}", e));
            }
        }
        Err(e) => log(&format!("history: serialize error: {}", e)),
    }
}
