# Changelog

## 0.6.15 (2026-02-20)

### Added
- In-app auto-update: "Install & Restart" button replaces the old "Download" link that opened GitHub
- Uses Tauri's built-in updater plugin with Ed25519 signature verification
- After downloading, shows "Restart now" link to apply the update immediately
- `scripts/release.sh` automates building, signing, generating `latest.json`, and creating GitHub releases

### Changed
- Update check now uses tauri-plugin-updater instead of manual GitHub API calls
- Removed manual version comparison helpers (`parse_version`, `is_newer_version`)

## 0.6.14 (2026-02-20)

### Added
- Session usage percentage now displays next to the tray icon in the macOS menu bar (e.g., "12%")
- Updates automatically on each refresh cycle

### Fixed
- Sparkline tooltip now maps to the full time window (e.g., 11am-4pm) instead of just the data point range, so hovering at the right edge correctly shows the window end time
- Reset time no longer shows "0m" when minutes are zero (e.g., "1h 0m" is now "1h")

## 0.6.13 (2026-02-18)

### Fixed
- Usage data now renders in ~1 second instead of waiting 16+ seconds for costs to load
- Split `get_all_data` into separate `get_usage_data` and `get_costs_data` commands
- Usage and costs fetch concurrently but render independently as each completes
- Costs section shows "Loading costs..." while ccusage runs in the background

## 0.6.12 (2026-02-18)

### Security
- `open_url` now validates URL structure and blocks shell metacharacters (defense-in-depth)
- Debug log file created with 0600 permissions (owner-only read/write)
- History file uses atomic write (write to .tmp then rename) to prevent corruption
- History file created with 0600 permissions
- Capabilities scoped to `["panel"]` window instead of wildcard `["*"]`

### Improved
- Extracted shared `buildSparklineSVG()` function, eliminating duplicated sparkline rendering code
- Extracted `fetch_with_timeout()` helper in Rust, removing duplicated timeout-match pattern
- Merged duplicate history loading blocks into single `spawn_blocking` call
- Removed dead code: `get_stay_on_top_pref` command (never called from frontend)
- History file race condition fixed with Mutex protecting read-modify-write cycle
- `escapeHtml()` now uses string replacement instead of DOM manipulation (faster)
- Timers cleared when panel is hidden (saves network requests and battery)
- Multiple `Local::now()` calls in ccusage consolidated to avoid midnight edge case
- Blur suppression threshold extracted to named constant `BLUR_SUPPRESS_MS`
- `DAY_NAMES` moved to module-scope constant (was recreated per `formatReset` call)
- Sparkline dimension constants extracted (`SPARK_WIDTH`, `SPARK_HEIGHT`, etc.)

## 0.6.11 (2026-02-18)

### Fixed
- Usage labels now match Claude Code CLI exactly: "Current session", "Current week (all models)", "Current week (Sonnet only)"
- Percentage display uses Math.floor (not Math.round) to match CLI behavior, fixing 1% discrepancy
- Old sparkline history data auto-migrated to new label names

## 0.6.10 (2026-02-17)

### Fixed
- Refresh spinner no longer shows on auto-refresh or initial load, only on manual click

## 0.6.9 (2026-02-17)

### Added
- Collapsible "Weekly Limits" section with chevron toggle (collapsed state persisted)
- Collapsible "Extra Usage" section with chevron toggle (collapsed state persisted)
- Session (5hr) limit always visible; weekly and extra usage can be collapsed to reduce panel size
- Generic collapsible section mechanism replaces the costs-only implementation

### Changed
- Costs, weekly, and extra sections all use the same `initCollapsible()` pattern (DRY)

## 0.6.8 (2026-02-16)

### Changed
- Moved light/dark mode toggle from header button into Settings as a "Dark mode" toggle switch
- Consistent with existing preference toggles (autostart, stay on top)
- Removed moon/sun icon button from header to reduce clutter

### Security
- `open_url` now validates URL scheme (only http/https allowed), preventing command injection
- Windows: replaced `cmd /c start` with `explorer.exe` to avoid shell metacharacter interpretation
- Enabled Content Security Policy (CSP) in Tauri config
- Debug log moved from world-readable `/tmp/` to app data directory
- Added try-catch around sparkline JSON.parse for defensive parsing

### Improved
- Extracted shared `formatTime12h()` and `MONTH_NAMES` constant (was duplicated 3x)
- Extracted `getColorClass()` helper (was duplicated 2x)
- Extracted `push_bucket()` helper in usage API (was duplicated 4x)
- Merged duplicate CSS selector for detached panel

## 0.6.7 (2026-02-16)

### Fixed
- Download link in update checker now opens the GitHub release page in the system browser
- Previously the link did nothing because Tauri's webview doesn't handle `target="_blank"` links
- Added cross-platform `open_url` command (macOS `open`, Windows `start`, Linux `xdg-open`)

## 0.6.6 (2026-02-16)

### Added
- Tray icon toggle: clicking the tray icon when the panel is already open now closes it (docked mode only)
- Login button shown in usage error state when token is expired/unauthorized
- Clicking "Open claude to login" launches Terminal with `claude` CLI to trigger OAuth flow
- Cross-platform terminal launch: macOS (Terminal.app via osascript), Windows (cmd), Linux (gnome-terminal/konsole/xfce4-terminal/xterm)

### Fixed
- When detached, clicking tray icon still focuses the window (no toggle) as expected

## 0.6.5 (2026-02-12)

### Added
- Prediction line on session sparklines showing projected usage at window end
- Dashed line extends from last data point using linear regression of historical points
- Predicted value clamped to 0-100% range
- Only appears on the current (live) session window, not past windows
- No prediction shown for flat usage (slope near zero)
- Weekly sparklines unchanged

## 0.6.4 (2026-02-12)

### Added
- Sparkline tooltip on hover showing timestamp and usage percentage for the nearest data point
- Tooltip displays time for today's data ("2:30pm") and includes date for older data ("Feb 10, 2:30pm")
- Tooltip follows mouse horizontally, clamped within panel bounds
- Works on both navigable session sparklines and weekly sparklines
- Adapts to light and dark themes via CSS variables

## 0.6.3 (2026-02-12)

### Added
- Cross-platform support: Windows and Linux builds now possible alongside macOS
- Windows credential manager and Linux libsecret support via `keyring` crate
- Platform-specific ccusage path detection (Homebrew, npm global, AppData, etc.)
- Windows (`windows`) and Linux (`deb`, `appimage`) bundle targets in config
- Center-screen fallback positioning for Linux (tray click position may be unavailable)

### Changed
- Replaced `core-graphics` cursor detection with tray event position (cross-platform)
- Log file now uses `std::env::temp_dir()` instead of hardcoded `/tmp/`
- Home directory detection uses `dirs` crate instead of hardcoded paths
- CSS font stack updated: `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", "Ubuntu", sans-serif`
- Uses `where` on Windows and `which` on Unix for ccusage fallback lookup
- Uses `;` PATH separator on Windows, `:` on Unix

### Fixed
- Panel border removed in docked mode (box-shadow is sufficient), border retained in detached mode

### Removed
- `core-graphics` dependency (was macOS-only)

## 0.6.2 (2026-02-12)

### Fixed
- Extra Usage now displays dollars correctly (API returns cents, was showing $2008 instead of $20.08)

### Changed
- Session sparkline reverted to 5-hour window with navigable prev/next arrows
- Arrow buttons and trackpad horizontal swipe to browse previous 5-hour windows
- Each window shows start/end time labels (e.g. "2pm - 7pm")
- "No data" shown for empty windows
- Weekly sparklines remain unchanged

## 0.6.1 (2026-02-12)

### Fixed
- Hidden scrollbar in detached mode that appeared as a border on the right
- Update checker now uses proper semver comparison (no longer shows older versions as updates)
- Session sparkline now shows 24 hours of history so previous 5hr windows are visible

## 0.6.0 (2026-02-12)

### Added
- Preferences pane with gear icon button in the header
- "Start on system startup" toggle using macOS LaunchAgent (tauri-plugin-autostart)
- "Stay on top when undocked" toggle, persisted across sessions
- Version display and "Check for updates" link that queries GitHub releases
- Collapsible "Estimated Costs" section with chevron indicator
- Costs collapsed state persisted in localStorage

## 0.5.0 (2026-02-12)

### Added
- Breakout/detach mode: pop the panel out into a persistent floating window
- Pop-out button in header (between theme toggle and refresh)
- When detached: panel stays visible on blur, header becomes draggable, window is resizable (min 300x400)
- When detached: always-on-top is disabled so other windows can overlap
- Tray click while detached focuses the existing window without repositioning
- Dock-in button returns panel to normal popover behavior
- Escape key only dismisses panel when in docked mode

## 0.4.0 (2026-02-12)

### Changed
- Renamed project and app from ClaudeCost to Claudit
- Updated bundle identifier to com.claudit.monitor
- All internal references, localStorage keys, and log files renamed

## 0.3.1 (2026-02-12)

### Added
- Light/dark mode toggle button in the header (moon/sun icon)
- Light theme with high-contrast colors for comfortable reading in bright environments
- Theme persists across sessions via localStorage
- Defaults to macOS system preference (prefers-color-scheme), auto-tracks system changes when no manual override

### Changed
- Reset time format now shows days and absolute date/time: "Resets in 6d 12h 10m (Thursday 19th Feb 8am)"
- CSS variables moved from `:root` to `[data-theme]` selectors for proper theme switching
- Sparkline background, panel shadow, and error background now use CSS variables that adapt per theme
- Dim text (`--text-dim`) improved to `#6b6b80` in light mode for better readability
- Status colors (green, amber, red, blue) use darker shades in light mode for WCAG contrast

## 0.3.0 (2026-02-12)

### Added
- Sparkline usage graphs showing usage trends over time below each progress bar
- Usage history persistence to `~/Library/Application Support/com.claudit.monitor/usage_history.json`
- Session (5hr) sparklines show last 5 hours of data, weekly sparklines show 7 days
- SVG area charts with gradient fill matching limit status color (green/amber/red)
- Automatic history pruning (entries older than 7 days removed on each save)
- Sparklines appear after 2+ data points are collected (after ~2 minutes)

### Changed
- Panel height increased from 500px to 620px to accommodate sparkline graphs

## 0.2.0 (2026-02-12)

### Fixed
- Usage API parsing to match actual Anthropic response format (named fields: five_hour, seven_day, seven_day_opus, seven_day_sonnet)
- ccusage date format (YYYYMMDD) and JSON wrapper parsing
- Data loading hang - usage and cost fetches now run concurrently with timeouts
- Keychain access runs in blocking task to avoid stalling async runtime
- Panel now appears on the monitor where the tray icon was clicked (CoreGraphics cursor detection)

### Added
- Extra Usage display showing monthly spend limit with progress bar
- Weekly Sonnet usage limit display
- 10s/15s timeouts on API and ccusage calls to prevent infinite loading

## 0.1.0 (2026-02-12)

### Added
- Initial release of Claudit macOS menu bar app
- System tray icon with click-to-toggle panel
- Usage limits display with progress bars (session, weekly all models, weekly opus)
- Color-coded progress bars (green < 70%, amber 70-90%, red > 90%)
- Reset countdown timers for each usage limit
- Estimated costs from ccusage (today, 7-day, 30-day)
- 5-minute cost cache to avoid excessive ccusage calls
- Auto-refresh every 60 seconds with visible countdown
- Manual refresh button with spinner animation
- Panel hides on blur (click away to dismiss)
- Dark theme with macOS panel aesthetic
- Graceful error handling for missing keychain, expired tokens, missing ccusage
- Dock icon hidden (Accessory activation policy)
- Template tray icon for proper macOS light/dark menu bar support
