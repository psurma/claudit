# Changelog

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
