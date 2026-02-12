use crate::ccusage::{self, CostCache, CostData};
use crate::history::{self, UsageSnapshot};
use crate::keychain;
use crate::usage_api::{self, UsageData};
use crate::log;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
pub struct AppData {
    pub usage: Option<UsageData>,
    pub usage_error: Option<String>,
    pub costs: Option<CostData>,
    pub costs_error: Option<String>,
    pub usage_history: Option<Vec<UsageSnapshot>>,
    pub timestamp: String,
}

#[tauri::command]
pub async fn get_all_data(app: tauri::AppHandle, cost_cache: State<'_, CostCache>) -> Result<AppData, ()> {
    log("get_all_data: starting");
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

    // Run keychain lookup in a blocking task so it doesn't block the async runtime
    let token_result = tokio::task::spawn_blocking(keychain::get_oauth_token)
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    log(&format!("get_all_data: keychain result={}", token_result.is_ok()));

    // Fetch usage and costs concurrently
    let usage_future = async {
        match token_result {
            Ok(ref token) => {
                log("get_all_data: fetching usage API");
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    usage_api::fetch_usage(token),
                ).await;
                match result {
                    Ok(Ok(data)) => { log("get_all_data: usage OK"); (Some(data), None) }
                    Ok(Err(e)) => { log(&format!("get_all_data: usage error: {}", e)); (None, Some(e.to_string())) }
                    Err(_) => { log("get_all_data: usage timeout"); (None, Some("Request timed out".to_string())) }
                }
            }
            Err(ref e) => (None, Some(e.clone())),
        }
    };

    let cost_cache_ref = cost_cache.inner().clone();
    let costs_future = async move {
        log("get_all_data: fetching costs");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            ccusage::fetch_costs(&cost_cache_ref),
        ).await;
        match result {
            Ok(Ok(data)) => { log("get_all_data: costs OK"); (Some(data), None) }
            Ok(Err(e)) => { log(&format!("get_all_data: costs error: {}", e)); (None, Some(e.to_string())) }
            Err(_) => { log("get_all_data: costs timeout"); (None, Some("Request timed out".to_string())) }
        }
    };

    let ((usage, usage_error), (costs, costs_error)) = tokio::join!(usage_future, costs_future);

    // Save snapshot and load history if usage was fetched successfully
    let usage_history = if let Some(ref usage_data) = usage {
        let app_clone = app.clone();
        let usage_clone = usage_data.clone();
        tokio::task::spawn_blocking(move || {
            history::save_snapshot(&app_clone, &usage_clone);
            let h = history::load_history(&app_clone);
            h.snapshots
        })
        .await
        .ok()
    } else {
        // Still load history even if this fetch failed
        let app_clone = app.clone();
        tokio::task::spawn_blocking(move || {
            let h = history::load_history(&app_clone);
            h.snapshots
        })
        .await
        .ok()
    };

    log("get_all_data: done");

    Ok(AppData {
        usage,
        usage_error,
        costs,
        costs_error,
        usage_history,
        timestamp,
    })
}

#[tauri::command]
pub async fn hide_panel(app: tauri::AppHandle) -> Result<(), ()> {
    crate::PANEL_VISIBLE.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn detach_panel(app: tauri::AppHandle) -> Result<(), ()> {
    log("detach_panel: detaching");
    if let Some(window) = app.get_webview_window("panel") {
        let stay_on_top = crate::STAY_ON_TOP_DETACHED.load(Ordering::SeqCst);
        let _ = window.set_always_on_top(stay_on_top);
        let _ = window.set_resizable(true);
        let _ = window.set_min_size(Some(tauri::LogicalSize::new(300.0, 400.0)));
        crate::PANEL_DETACHED.store(true, Ordering::SeqCst);
        let _ = app.emit("panel-detached", ());
        log("detach_panel: done");
    }
    Ok(())
}

#[tauri::command]
pub async fn attach_panel(app: tauri::AppHandle) -> Result<(), ()> {
    log("attach_panel: re-docking");
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.set_always_on_top(true);
        let _ = window.set_resizable(false);
        let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
        let _ = window.set_size(tauri::LogicalSize::new(crate::PANEL_WIDTH, crate::PANEL_HEIGHT));
        crate::PANEL_DETACHED.store(false, Ordering::SeqCst);
        crate::PANEL_VISIBLE.store(false, Ordering::SeqCst);
        let _ = window.hide();
        let _ = app.emit("panel-attached", ());
        log("attach_panel: done, panel hidden");
    }
    Ok(())
}

#[tauri::command]
pub fn set_stay_on_top_pref(enabled: bool) -> Result<(), ()> {
    log(&format!("set_stay_on_top_pref: {}", enabled));
    crate::STAY_ON_TOP_DETACHED.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_stay_on_top_pref() -> Result<bool, ()> {
    Ok(crate::STAY_ON_TOP_DETACHED.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    log("check_for_updates: fetching latest release");
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/repos/psurma/claudit/releases/latest")
        .header("User-Agent", "Claudit")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status() == 404 {
        let current = env!("CARGO_PKG_VERSION").to_string();
        return Ok(UpdateInfo {
            current_version: current,
            latest_version: "unknown".to_string(),
            update_available: false,
            release_url: "https://github.com/psurma/claudit/releases".to_string(),
        });
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = body["tag_name"].as_str().unwrap_or("unknown");
    let latest = tag.strip_prefix('v').unwrap_or(tag).to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let html_url = body["html_url"]
        .as_str()
        .unwrap_or("https://github.com/psurma/claudit/releases")
        .to_string();

    let update_available = latest != "unknown" && is_newer_version(&latest, &current);

    log(&format!(
        "check_for_updates: current={}, latest={}, update={}",
        current, latest, update_available
    ));

    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest,
        update_available,
        release_url: html_url,
    })
}

fn parse_version(v: &str) -> Option<Vec<u64>> {
    v.split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let Some(l) = parse_version(latest) else { return false };
    let Some(c) = parse_version(current) else { return false };
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv { return true; }
        if lv < cv { return false; }
    }
    false
}
