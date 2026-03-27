# HomeRun

> One-click GitHub Actions self-hosted runners for macOS

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Coverage](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/aGallea/77f18f115b500bdc5d6df52f95d399b9/raw/coverage.json)
[![macOS 13+](https://img.shields.io/badge/macOS-13%2B-brightgreen)](https://github.com/aGallea/homerun)

HomeRun replaces the manual GitHub self-hosted runner setup process with a unified macOS desktop app and terminal UI. Authenticate with GitHub once, pick a repository, and launch runners with a single click. HomeRun handles download, registration, process management, log streaming, and resource monitoring — everything the official docs make you do by hand.

## Features

- **One-click runner setup** — no shell scripts, no copy-pasting tokens
- **Device Flow authentication** — log in with your GitHub account via browser; no PAT required
- **Batch runner creation** — spin up multiple runners for the same repo in one step with live progress
- **Unified dashboard** — monitor all runners across all repos in one place
- **Live log streaming** — tail runner output in real time from the runner detail view
- **Job tracking** — current job progress with step-by-step status, estimated completion, and full job history per runner
- **Real-time metrics** — CPU/RAM per runner via live WebSocket updates
- **Two run modes** — app-managed (daemon child) or background service (launchd)
- **Auto-restart** — crashed runners recover automatically (up to 3 attempts)
- **Smart repo discovery** — scan local workspace directories or your GitHub account for repos that use self-hosted runners
- **Terminal UI** — k9s-inspired TUI with info header, context-sensitive keybindings (F1-F4 tabs), repo search, and in-app login via Device Flow
- **CLI mode** — scriptable `homerun --no-tui` commands with colored output for automation
- **macOS native** — Keychain token storage, launchd auto-start, native notifications
- **Pre-commit hooks** — enforces `cargo fmt`, `cargo clippy`, conventional commits, and Prettier on every commit

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

Runners are native child processes of the daemon — not Docker containers. Each runner is an instance of the [official GitHub Actions runner binary](https://github.com/actions/runner). All GitHub communication is outbound HTTPS. No inbound ports needed.

For the full architecture deep-dive (runner lifecycle, state machine, process management, auth flow), see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

New to self-hosted runners? See [How Self-Hosted Runners Work](docs/SELF_HOSTED_RUNNERS.md) for a primer on runner communication, permissions, security considerations, and what HomeRun automates.

## Quick Start

### Install (DMG)

1. Download the latest `.dmg` for your architecture from [Releases](https://github.com/aGallea/homerun/releases):
   - **Apple Silicon** (M1/M2/M3/M4): `HomeRun_<version>_aarch64.dmg`
   - **Intel**: `HomeRun_<version>_x86_64.dmg`
2. Open the `.dmg` and drag HomeRun to Applications
3. Remove the macOS quarantine flag (required because the app is not yet code-signed):

   ```sh
   xattr -cr /Applications/HomeRun.app
   ```

4. Launch HomeRun — go to Settings > Startup > "Launch at login" to auto-start the daemon

The `.dmg` bundles the `homerund` daemon inside the app. Releases are automated via [release-please](https://github.com/googleapis/release-please) — every merge to `master` with conventional commits triggers a Release PR with version bumps and changelog.

### Install (Homebrew)

> _Coming soon — see [#18](https://github.com/aGallea/homerun/issues/18)._

### Build from Source

```sh
git clone https://github.com/aGallea/homerun.git
cd homerun

# Build the daemon and TUI
cargo build --release -p homerund -p homerun

# Binaries are in target/release/
ls target/release/homerund target/release/homerun
```

To build the desktop app as well, see [CONTRIBUTING.md](CONTRIBUTING.md) for the full dev setup.

### Run

```sh
# Start the daemon (must be running before using the TUI or desktop app)
homerund

# Launch the interactive TUI
homerun

# Or use CLI mode (plain text output, no interactive UI — useful for scripts)
homerun --no-tui list
```

## Screenshots

> _Screenshots coming soon — the app is under active development._

## CLI Usage

The `--no-tui` flag disables the interactive terminal UI and prints plain text output instead. This is useful for scripting, automation, and quick status checks.

```sh
# List all runners with status, mode, and CPU usage
homerun --no-tui list

# Show overall status (daemon, auth, runner counts, system metrics)
homerun --no-tui status

# Scan a local workspace for repos using self-hosted runners
homerun --no-tui scan ~/workspace

# Scan your GitHub repos remotely (requires authentication)
homerun --no-tui scan --remote

# Combine local and remote scanning
homerun --no-tui scan ~/workspace --remote

# Manage the daemon
homerun --no-tui daemon start
homerun --no-tui daemon stop
homerun --no-tui daemon restart
```

## Tech Stack

| Component          | Technology                                              |
| ------------------ | ------------------------------------------------------- |
| Daemon             | Rust + Axum (async HTTP/SSE/WebSocket over Unix socket) |
| TUI / CLI          | Rust + Ratatui + Clap                                   |
| Desktop app        | Tauri 2.0 + React + TypeScript                          |
| Process management | `tokio::process` + `sysinfo`                            |
| GitHub API         | `octocrab` crate                                        |
| Auth token storage | macOS Keychain (`security-framework`)                   |
| Log streaming      | Server-Sent Events (SSE)                                |
| Real-time updates  | WebSocket                                               |
| Auto-start         | macOS launchd                                           |
| Notifications      | macOS native (`notify-rust`)                            |

## Requirements

- macOS 13+ (Ventura or later)
- ARM64 or Intel Mac
- A GitHub account

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to set up the dev environment, coding standards, and the PR process.

## License

[MIT](LICENSE) © 2026 aGallea
