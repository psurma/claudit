use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::keychain;
use crate::usage_api;

pub static NOTIFICATIONS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Tracks the `reset_at` value we last notified for, so we only fire once per session window.
static LAST_NOTIFIED_RESET: Mutex<Option<String>> = Mutex::new(None);

pub async fn check_and_notify() {
    if !NOTIFICATIONS_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let token = match tokio::task::spawn_blocking(keychain::get_oauth_token).await {
        Ok(Ok(t)) => t,
        _ => {
            crate::log("notifier: no valid token, skipping");
            return;
        }
    };

    let data = match usage_api::fetch_usage(&token).await {
        Ok(d) => d,
        Err(e) => {
            crate::log(&format!("notifier: fetch_usage error: {}", e));
            return;
        }
    };

    let session = match data.limits.iter().find(|l| l.label == "Current session") {
        Some(s) => s,
        None => {
            crate::log("notifier: no session limit found");
            return;
        }
    };

    let reset_at_str = match &session.reset_at {
        Some(r) => r.clone(),
        None => {
            crate::log("notifier: no reset_at on session");
            return;
        }
    };

    // Check if we already notified for this session window
    {
        let guard = LAST_NOTIFIED_RESET.lock().unwrap();
        if guard.as_deref() == Some(&reset_at_str) {
            return; // already notified for this window
        }
    }

    let reset_at = match chrono::DateTime::parse_from_rfc3339(&reset_at_str) {
        Ok(dt) => dt,
        Err(_) => {
            crate::log(&format!("notifier: failed to parse reset_at: {}", reset_at_str));
            return;
        }
    };

    let now = chrono::Utc::now();
    let until_reset = reset_at.signed_duration_since(now);
    let minutes_left = until_reset.num_minutes();

    let usage_pct = session.usage_pct; // 0.0 - 1.0

    crate::log(&format!(
        "notifier: {}min until reset, usage={:.0}%",
        minutes_left,
        usage_pct * 100.0
    ));

    // Trigger conditions:
    // - 30-75 minutes until reset
    // - Usage below 80% (at least 20% going unused)
    if minutes_left >= 30 && minutes_left <= 75 && usage_pct < 0.80 {
        let pct = (usage_pct * 100.0).floor() as i32;
        let unused = 100 - pct;

        crate::log(&format!("notifier: firing notification ({}% unused, {}min left)", unused, minutes_left));

        let body = format!(
            "You've only used {}% of your session. ~{}min left before it resets.",
            pct, minutes_left
        );

        let result = notify_rust::Notification::new()
            .summary("Use your tokens!")
            .body(&body)
            .appname("Claudit")
            .show();

        match result {
            Ok(_) => crate::log("notifier: notification sent"),
            Err(e) => crate::log(&format!("notifier: failed to send: {}", e)),
        }

        // Mark this window as notified
        let mut guard = LAST_NOTIFIED_RESET.lock().unwrap();
        *guard = Some(reset_at_str);
    }
}
