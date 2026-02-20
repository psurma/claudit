use crate::ccusage::{self, CostCache, CostData};
use crate::history::{self, UsageSnapshot};
use crate::keychain;
use crate::usage_api::{self, UsageData};
use crate::log;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize)]
pub struct UsageResult {
    pub usage: Option<UsageData>,
    pub usage_error: Option<String>,
    pub usage_history: Option<Vec<UsageSnapshot>>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostsResult {
    pub costs: Option<CostData>,
    pub costs_error: Option<String>,
}

async fn fetch_with_timeout<T, E: std::fmt::Display>(
    label: &str,
    timeout_secs: u64,
    future: impl std::future::Future<Output = Result<T, E>>,
) -> (Option<T>, Option<String>) {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        future,
    ).await;
    match result {
        Ok(Ok(data)) => { log(&format!("{} OK", label)); (Some(data), None) }
        Ok(Err(e)) => { log(&format!("{} error: {}", label, e)); (None, Some(e.to_string())) }
        Err(_) => { log(&format!("{} timeout", label)); (None, Some("Request timed out".to_string())) }
    }
}

#[tauri::command]
pub async fn get_usage_data(app: tauri::AppHandle) -> Result<UsageResult, ()> {
    log("get_usage_data: starting");
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

    let token_result = tokio::task::spawn_blocking(keychain::get_oauth_token)
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    log(&format!("get_usage_data: keychain result={}", token_result.is_ok()));

    let (usage, usage_error) = match token_result {
        Ok(ref token) => {
            log("get_usage_data: fetching usage API");
            fetch_with_timeout("usage", 10, usage_api::fetch_usage(token)).await
        }
        Err(ref e) => (None, Some(e.clone())),
    };

    if let Some(ref data) = usage {
        if let Some(session) = data.limits.iter().find(|l| l.label == "Current session") {
            let pct = (session.usage_pct * 100.0).floor() as i32;
            let title = format!("{}%", pct);
            log(&format!("set tray title: {}", title));
            if let Some(tray) = app.tray_by_id("main-tray") {
                let _ = tray.set_title(Some(&title));
            } else {
                log("tray not found by id main-tray");
            }
        }
    }

    let usage_history = {
        let app_clone = app.clone();
        let usage_for_save = usage.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(ref data) = usage_for_save {
                history::save_snapshot(&app_clone, data);
            }
            history::load_history(&app_clone).snapshots
        })
        .await
        .ok()
    };

    log("get_usage_data: done");
    Ok(UsageResult { usage, usage_error, usage_history, timestamp })
}

#[tauri::command]
pub async fn get_costs_data(cost_cache: State<'_, CostCache>) -> Result<CostsResult, ()> {
    log("get_costs_data: starting");
    let cost_cache_ref = cost_cache.inner().clone();
    let (costs, costs_error) = fetch_with_timeout("costs", 45, ccusage::fetch_costs(&cost_cache_ref)).await;
    log("get_costs_data: done");
    Ok(CostsResult { costs, costs_error })
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
    pub release_notes: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    log("check_for_updates: using tauri updater plugin");
    let current = env!("CARGO_PKG_VERSION").to_string();

    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;

    match update {
        Some(update) => {
            let latest = update.version.clone();
            let notes = update.body.clone();
            log(&format!(
                "check_for_updates: current={}, latest={}, update available",
                current, latest
            ));
            Ok(UpdateInfo {
                current_version: current,
                latest_version: latest,
                update_available: true,
                release_notes: notes,
            })
        }
        None => {
            log(&format!("check_for_updates: current={}, up to date", current));
            Ok(UpdateInfo {
                current_version: current.clone(),
                latest_version: current,
                update_available: false,
                release_notes: None,
            })
        }
    }
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    log("install_update: checking for update");
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;

    let update = update.ok_or_else(|| "No update available".to_string())?;
    log(&format!("install_update: downloading v{}", update.version));

    let _ = app.emit("update-progress", "downloading");

    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    log("install_update: download and install complete");
    let _ = app.emit("update-progress", "done");
    Ok(())
}

#[tauri::command]
pub async fn relaunch_app(app: tauri::AppHandle) -> Result<(), String> {
    log("relaunch_app: restarting");
    app.restart();
}

#[tauri::command]
pub async fn open_login() -> Result<(), String> {
    log("open_login: launching claude CLI");
    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Terminal\" to do script \"claude\"")
            .output()
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        tokio::process::Command::new("cmd")
            .args(["/c", "start", "cmd", "/k", "claude"])
            .output()
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        // Try common terminal emulators in order
        let terminals = [
            ("gnome-terminal", vec!["--", "claude"]),
            ("konsole", vec!["-e", "claude"]),
            ("xfce4-terminal", vec!["-e", "claude"]),
            ("xterm", vec!["-e", "claude"]),
        ];
        let mut launched = false;
        for (term, args) in &terminals {
            if tokio::process::Command::new(term)
                .args(args)
                .spawn()
                .is_ok()
            {
                launched = true;
                break;
            }
        }
        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    // Validate scheme
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Only HTTP/HTTPS URLs are allowed".to_string());
    }
    // Validate URL has a host after the scheme
    let after_scheme = if url.starts_with("https://") { &url[8..] } else { &url[7..] };
    if after_scheme.is_empty() || after_scheme.starts_with('/') {
        return Err("Invalid URL: missing host".to_string());
    }
    // Block shell metacharacters (defense-in-depth; Command::new doesn't use shell)
    if url.contains(&['`', '|', ';', '&', '$', '(', ')', '{', '}', '<', '>', '\n', '\r'][..]) {
        return Err("URL contains invalid characters".to_string());
    }
    log(&format!("open_url: {}", url));
    #[cfg(target_os = "macos")]
    {
        tokio::process::Command::new("open")
            .arg(&url)
            .output()
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        tokio::process::Command::new("explorer")
            .arg(&url)
            .output()
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        tokio::process::Command::new("xdg-open")
            .arg(&url)
            .output()
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

