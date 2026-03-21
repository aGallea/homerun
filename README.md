# HomeRun

> One-click GitHub Actions self-hosted runners for macOS

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/aGallea/homerun/actions/workflows/ci.yml/badge.svg)](https://github.com/aGallea/homerun/actions/workflows/ci.yml)
[![macOS 13+](https://img.shields.io/badge/macOS-13%2B-brightgreen)](https://github.com/aGallea/homerun)

HomeRun replaces the manual GitHub self-hosted runner setup process with a unified macOS desktop app and terminal UI. Authenticate with GitHub once, pick a repository, and launch runners with a single click. HomeRun handles download, registration, process management, log streaming, and resource monitoring — everything the official docs make you do by hand.

## Features

- **One-click runner setup** — no shell scripts, no copy-pasting tokens
- **Unified dashboard** — monitor all runners across all repos in one place
- **Real-time logs & metrics** — CPU/RAM per runner with live streaming
- **Two run modes** — app-managed (daemon child) or background service (launchd)
- **Auto-restart** — crashed runners recover automatically (up to 3 attempts)
- **Smart repo discovery** — scan your workspace or GitHub for repos that need self-hosted runners
- **Terminal UI** — full keyboard-driven TUI with the same capabilities as the GUI
- **CLI mode** — scriptable `homerun --no-tui` commands for automation
- **macOS native** — Keychain token storage, launchd auto-start, native notifications

## Architecture

```
┌──────────────┐   ┌─────────┐
│  Tauri App   │   │   TUI   │     (thin clients)
└──────┬───────┘   └────┬────┘
       └────────┬────────┘
                │ Unix socket (REST + SSE + WebSocket)
       ┌────────┴────────┐
       │   homerund      │     (daemon — ~/.homerun/daemon.sock)
       └────────┬────────┘
                │ spawns / monitors
      ┌─────────┼─────────┐
      │         │         │
   ┌──┴──┐  ┌──┴──┐  ┌──┴──┐
   │Run 1│  │Run 2│  │Run N│   (GitHub Actions runner processes)
   └─────┘  └─────┘  └─────┘
```

All GitHub communication is outbound HTTPS from the runner processes. No inbound ports needed.

## Quick Start

### Install (DMG)

1. Download the latest `.dmg` from [Releases](https://github.com/aGallea/homerun/releases)
2. Open the `.dmg` and drag HomeRun to Applications
3. Launch HomeRun — the daemon starts automatically

### Install (Homebrew)

```sh
brew install homerun
```

This installs both the `homerun` TUI/CLI and the `homerund` daemon.

### Run

```sh
# Start the daemon
homerund

# Launch the TUI
homerun

# Or use CLI mode
homerun --no-tui list
```

## Screenshots

> _Screenshots coming soon — the app is under active development._

## CLI Usage

```sh
# List all runners
homerun --no-tui list

# Add 4 runners for a repo
homerun --no-tui add my-runner --count 4 --labels ci,e2e --mode service

# Check status
homerun --no-tui status

# Remove a runner
homerun --no-tui remove my-runner-2

# Scan workspace for repos using self-hosted runners
homerun --no-tui scan ~/workspace

# Scan GitHub repos remotely
homerun --no-tui scan --remote

# Login with a Personal Access Token
homerun --no-tui login --token <PAT>
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Daemon | Rust + Axum (async HTTP/SSE/WebSocket over Unix socket) |
| TUI / CLI | Rust + Ratatui + Clap |
| Desktop app | Tauri 2.0 + React + TypeScript |
| Process management | `tokio::process` + `sysinfo` |
| GitHub API | `octocrab` crate |
| Auth token storage | macOS Keychain (`security-framework`) |
| Log streaming | Server-Sent Events (SSE) |
| Real-time updates | WebSocket |
| Auto-start | macOS launchd |
| Notifications | macOS native (`notify-rust`) |

## Requirements

- macOS 13+ (Ventura or later)
- ARM64 or Intel Mac
- A GitHub account

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to set up the dev environment, coding standards, and the PR process.

## License

[MIT](LICENSE) © 2026 aGallea
