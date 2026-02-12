# Claudit

A macOS menu bar app that shows your Claude Code usage limits and estimated costs at a glance.

![Claudit panel screenshot](screenshot.png)

## Features

- **Usage limits** - Session (5hr rolling), weekly all models, weekly Sonnet, and weekly Opus with progress bars
- **Sparkline graphs** - Usage trend history for each limit
- **Estimated costs** - Today, last 7 days, and last 30 days (powered by [ccusage](https://github.com/ryoppippi/ccusage))
- **Extra usage tracking** - Monthly spend limit with progress bar
- **Auto-refresh** - Updates every 60 seconds with visible countdown
- **Light/dark mode** - Toggle or follow system preference
- **Breakout mode** - Pop out the panel into a persistent, draggable, resizable floating window

## How It Works

- Reads your OAuth token from the macOS Keychain (stored by Claude Code)
- Fetches usage data from the Anthropic API
- Runs `ccusage` for cost estimates
- Lives in your menu bar with no dock icon

## Install

Download the latest `.dmg` from [Releases](https://github.com/psurma/claudit/releases), open it, and drag Claudit to Applications.

Since the app isn't signed with an Apple Developer certificate, macOS will block it on first launch. Remove the quarantine flag:

```bash
find /Applications/Claudit.app -exec xattr -c {} \;
```

## Requirements

- macOS 10.15+ (Apple Silicon)
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed and authenticated
- [ccusage](https://github.com/ryoppippi/ccusage) installed globally (`npm install -g ccusage`)

## Building from Source

```bash
npm install
npx tauri build
```

The built app will be at `src-tauri/target/release/bundle/macos/Claudit.app`.

## Development

```bash
npx tauri dev
```

## Tech Stack

- [Tauri v2](https://v2.tauri.app/) (Rust backend)
- Vanilla JavaScript frontend
- No bundler required
