use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum CcusageError {
    #[error("ccusage not found. Install with: npm install -g ccusage")]
    NotFound,
    #[error("ccusage failed: {0}")]
    ExecutionError(String),
    #[error("Failed to parse output: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CostData {
    pub today: f64,
    pub week: f64,
    pub month: f64,
}

#[derive(Debug, Deserialize)]
struct CcusageOutput {
    daily: Vec<DailyEntry>,
}

#[derive(Debug, Deserialize)]
struct DailyEntry {
    #[serde(default)]
    date: Option<String>,
    #[serde(default, alias = "totalCost")]
    total_cost: Option<f64>,
}

#[derive(Clone)]
pub struct CostCache {
    data: std::sync::Arc<Mutex<Option<(std::time::Instant, CostData)>>>,
}

impl CostCache {
    pub fn new() -> Self {
        Self {
            data: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    pub fn get(&self) -> Option<CostData> {
        let lock = self.data.lock().ok()?;
        if let Some((when, ref data)) = *lock {
            if when.elapsed() < std::time::Duration::from_secs(300) {
                return Some(data.clone());
            }
        }
        None
    }

    pub fn set(&self, data: CostData) {
        if let Ok(mut lock) = self.data.lock() {
            *lock = Some((std::time::Instant::now(), data));
        }
    }
}

pub async fn fetch_costs(cache: &CostCache) -> Result<CostData, CcusageError> {
    if let Some(cached) = cache.get() {
        return Ok(cached);
    }

    // ccusage expects YYYYMMDD format
    let since = Local::now()
        .date_naive()
        .checked_sub_days(chrono::Days::new(30))
        .unwrap_or(Local::now().date_naive())
        .format("%Y%m%d")
        .to_string();

    let ccusage_path = find_ccusage()?;

    let output = Command::new(&ccusage_path)
        .args(["daily", "--since", &since, "--json"])
        .env("PATH", build_path())
        .output()
        .await
        .map_err(|e| CcusageError::ExecutionError(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CcusageError::ExecutionError(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // ccusage wraps output in { "daily": [...] }
    let parsed: CcusageOutput = serde_json::from_str(&stdout)
        .map_err(|e| CcusageError::ParseError(format!("{}: {}", e, &stdout[..stdout.len().min(200)])))?;

    let today_str = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let week_ago = Local::now()
        .date_naive()
        .checked_sub_days(chrono::Days::new(7))
        .unwrap_or(Local::now().date_naive());

    let mut costs = CostData::default();

    for entry in &parsed.daily {
        let cost = entry.total_cost.unwrap_or(0.0);
        let date_str = entry.date.as_deref().unwrap_or("");

        costs.month += cost;

        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            if date >= week_ago {
                costs.week += cost;
            }
        }

        if date_str == today_str {
            costs.today = cost;
        }
    }

    // Round to 2 decimal places
    costs.today = (costs.today * 100.0).round() / 100.0;
    costs.week = (costs.week * 100.0).round() / 100.0;
    costs.month = (costs.month * 100.0).round() / 100.0;

    cache.set(costs.clone());
    Ok(costs)
}

fn home_dir_string() -> Option<String> {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string())
}

fn find_ccusage() -> Result<String, CcusageError> {
    let home = home_dir_string().unwrap_or_default();

    #[cfg(target_os = "macos")]
    let candidates: Vec<String> = vec![
        format!("{}/.npm-global/bin/ccusage", home),
        "/opt/homebrew/bin/ccusage".to_string(),
        "/usr/local/bin/ccusage".to_string(),
    ];

    #[cfg(target_os = "windows")]
    let candidates: Vec<String> = {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        vec![
            format!("{}\\npm\\ccusage.cmd", appdata),
            format!("{}\\AppData\\Roaming\\npm\\ccusage.cmd", home),
        ]
    };

    #[cfg(target_os = "linux")]
    let candidates: Vec<String> = vec![
        format!("{}/.npm-global/bin/ccusage", home),
        "/usr/local/bin/ccusage".to_string(),
        "/usr/bin/ccusage".to_string(),
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // Fall back to which/where
    let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    if let Ok(output) = std::process::Command::new(which_cmd).arg("ccusage").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }

    Err(CcusageError::NotFound)
}

fn build_path() -> String {
    let home = home_dir_string().unwrap_or_default();

    #[cfg(target_os = "macos")]
    let extra: Vec<String> = vec![
        format!("{}/.npm-global/bin", home),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];

    #[cfg(target_os = "windows")]
    let extra: Vec<String> = {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        vec![format!("{}\\npm", appdata)]
    };

    #[cfg(target_os = "linux")]
    let extra: Vec<String> = vec![
        format!("{}/.npm-global/bin", home),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
    ];

    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{}{}{}", extra.join(sep), sep, current)
}
