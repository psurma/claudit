mod ccusage;
mod commands;
mod history;
mod keychain;
mod usage_api;

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};

const PANEL_LABEL: &str = "panel";
const PANEL_WIDTH: f64 = 360.0;
const PANEL_HEIGHT: f64 = 620.0;
/// Max ms between blur-hide and tray click to suppress re-show (toggle behavior).
const BLUR_SUPPRESS_MS: u64 = 500;

pub static PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
pub static PANEL_DETACHED: AtomicBool = AtomicBool::new(false);
pub static STAY_ON_TOP_DETACHED: AtomicBool = AtomicBool::new(false);
/// Timestamp (ms since UNIX epoch) when the panel was last hidden by blur.
/// Used to suppress re-showing when the tray click caused the blur.
static LAST_BLUR_HIDE_MS: AtomicU64 = AtomicU64::new(0);

pub fn log(msg: &str) {
    let log_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.claudit.monitor");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("debug.log");

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&log_path)
    };
    #[cfg(not(unix))]
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    if let Ok(mut f) = file {
        let _ = writeln!(f, "[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg);
    }
}

fn show_panel(app: &tauri::AppHandle, cursor_pos: Option<PhysicalPosition<f64>>) {
    if let Some(w) = app.get_webview_window(PANEL_LABEL) {
        log("Showing panel");

        // When detached, just focus the existing window without repositioning
        if PANEL_DETACHED.load(Ordering::SeqCst) {
            log("Panel is detached, focusing without reposition");
            let _ = w.show();
            let _ = w.set_focus();
            PANEL_VISIBLE.store(true, Ordering::SeqCst);
            let _ = app.emit("panel-shown", ());
            return;
        }

        if let Some(pos) = cursor_pos {
            log(&format!("Tray click at physical ({}, {})", pos.x, pos.y));
            if let Ok(monitors) = app.available_monitors() {
                for mon in monitors {
                    let mpos = mon.position();
                    let size = mon.size();
                    let sf = mon.scale_factor();
                    // Convert physical click position to logical
                    let cx = pos.x / sf;
                    let cy = pos.y / sf;
                    let mx = mpos.x as f64 / sf;
                    let my = mpos.y as f64 / sf;
                    let mw = size.width as f64 / sf;
                    let mh = size.height as f64 / sf;
                    log(&format!("Monitor: logical ({}, {}) {}x{} sf={}", mx, my, mw, mh, sf));
                    if cx >= mx && cx < mx + mw && cy >= my && cy < my + mh {
                        let x = (cx - PANEL_WIDTH / 2.0).max(mx).min(mx + mw - PANEL_WIDTH);
                        let y = my + 30.0;
                        log(&format!("Moving panel to ({}, {})", x, y));
                        let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
                        break;
                    }
                }
            }
        } else {
            // Fallback: center on primary monitor (e.g. Linux where tray position may be unavailable)
            log("No cursor position, centering on primary monitor");
            if let Ok(Some(mon)) = app.primary_monitor() {
                let sf = mon.scale_factor();
                let mw = mon.size().width as f64 / sf;
                let x = (mw - PANEL_WIDTH) / 2.0;
                let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, 30.0)));
            }
        }

        let _ = w.show();
        let _ = w.set_focus();
        PANEL_VISIBLE.store(true, Ordering::SeqCst);
        let _ = app.emit("panel-shown", ());
        log(&format!("Panel shown, visible={:?}", w.is_visible()));
    } else {
        log("ERROR: panel window not found!");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log("App starting");

    tauri::Builder::default()
        .manage(ccusage::CostCache::new())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            commands::get_all_data,
            commands::hide_panel,
            commands::detach_panel,
            commands::attach_panel,
            commands::set_stay_on_top_pref,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
            commands::check_for_updates,
            commands::open_login,
            commands::open_url,
        ])
        .setup(|app| {
            log("Setup starting");

            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Create the panel window at startup (hidden), centered near top of screen
            log("Creating panel window at startup");
            let monitor = app.primary_monitor()?.unwrap_or_else(|| {
                app.available_monitors().unwrap().into_iter().next().unwrap()
            });
            let screen_width = monitor.size().width as f64 / monitor.scale_factor();

            let x = (screen_width - PANEL_WIDTH) / 2.0;
            let y = 30.0; // Just below the menu bar

            let window = WebviewWindowBuilder::new(app, PANEL_LABEL, WebviewUrl::App("index.html".into()))
                .title("Claudit")
                .inner_size(PANEL_WIDTH, PANEL_HEIGHT)
                .position(x, y)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .visible(false)
                .skip_taskbar(true)
                .build()?;
            log(&format!("Panel window created at ({}, {}), visible={:?}", x, y, window.is_visible()));

            // Build tray menu (right-click only)
            let refresh_item = MenuItemBuilder::with_id("refresh", "Refresh").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Claudit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&refresh_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let icon = tauri::include_image!("icons/tray-icon.png");

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .icon_as_template(true)
                .tooltip("Claudit")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = event {
                        // Check if blur just hid the panel (the tray click itself caused focus loss).
                        // If so, treat this as a toggle-close: don't re-show.
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let blur_ms = LAST_BLUR_HIDE_MS.load(Ordering::SeqCst);
                        if blur_ms > 0 && now_ms.saturating_sub(blur_ms) < BLUR_SUPPRESS_MS {
                            log("Tray click: suppressed (panel just hidden by blur)");
                            LAST_BLUR_HIDE_MS.store(0, Ordering::SeqCst);
                            return;
                        }

                        if PANEL_VISIBLE.load(Ordering::SeqCst) && !PANEL_DETACHED.load(Ordering::SeqCst) {
                            log("Tray click: hiding docked panel (toggle)");
                            PANEL_VISIBLE.store(false, Ordering::SeqCst);
                            if let Some(w) = tray.app_handle().get_webview_window(PANEL_LABEL) {
                                let _ = w.hide();
                            }
                        } else {
                            show_panel(tray.app_handle(), Some(position));
                        }
                    }
                })
                .on_menu_event(|app, event| {
                    log(&format!("Menu event: {:?}", event.id()));
                    match event.id().as_ref() {
                        "refresh" => {
                            show_panel(app, None);
                        }
                        "quit" => {
                            log("Quitting");
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            log("Tray icon with menu created, setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == PANEL_LABEL {
                if let tauri::WindowEvent::Focused(false) = event {
                    // Skip blur-hide when panel is detached
                    if PANEL_DETACHED.load(Ordering::SeqCst) {
                        log("Panel blur ignored (detached)");
                        return;
                    }
                    if PANEL_VISIBLE.load(Ordering::SeqCst) {
                        PANEL_VISIBLE.store(false, Ordering::SeqCst);
                        let _ = window.hide();
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        LAST_BLUR_HIDE_MS.store(now_ms, Ordering::SeqCst);
                        let _ = window.app_handle().emit("panel-hidden", ());
                        log("Panel hidden on blur");
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running application");
}
