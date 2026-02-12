use crate::ccusage::{self, CostCache, CostData};
use crate::history::{self, UsageSnapshot};
use crate::keychain;
use crate::usage_api::{self, UsageData};
use crate::log;
use serde::Serialize;
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
    crate::PANEL_VISIBLE.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn detach_panel(app: tauri::AppHandle) -> Result<(), ()> {
    log("detach_panel: detaching");
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.set_always_on_top(false);
        let _ = window.set_resizable(true);
        let _ = window.set_min_size(Some(tauri::LogicalSize::new(300.0, 400.0)));
        crate::PANEL_DETACHED.store(true, std::sync::atomic::Ordering::SeqCst);
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
        crate::PANEL_DETACHED.store(false, std::sync::atomic::Ordering::SeqCst);
        crate::PANEL_VISIBLE.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = window.hide();
        let _ = app.emit("panel-attached", ());
        log("attach_panel: done, panel hidden");
    }
    Ok(())
}
