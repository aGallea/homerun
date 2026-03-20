# HomeRun — Design Spec

A macOS desktop app + TUI for managing GitHub Actions self-hosted runners. Replaces the manual GitHub setup with a one-click experience.

## Problem

Setting up GitHub self-hosted runners requires navigating to repo settings, copying shell commands, running them manually, and managing processes by hand. There's no unified way to manage multiple runners across multiple repos, monitor their health, or persist them as services.

## Solution

An open-source macOS app that automates the entire self-hosted runner lifecycle: authenticate with GitHub, pick a repo, launch runners with one click, and monitor everything from a dashboard.

## Target Audience

Open source / public product — anyone with a GitHub account and a Mac who wants to run GitHub Actions locally.

## Architecture

### Three Components

1. **Daemon (`homerund`)** — Rust binary that runs as a background service. Manages runner processes, talks to GitHub API, streams logs, collects metrics. Exposes a local API over Unix socket.
2. **Tauri App** — React frontend in a native macOS window. Control panel for repos, runners, logs, metrics, and settings. Connects to the daemon via Unix socket.
3. **TUI (`homerun`)** — Ratatui-based terminal UI. Lazygit/k9s-style keyboard-driven interface. Same capabilities as the Tauri app, connects to the same daemon.

### Communication

```
┌──────────┐   ┌─────┐
│ Tauri App │   │ TUI │     (thin clients)
└────┬─────┘   └──┬──┘
     └──────┬─────┘
            │ Unix socket (REST + SSE + WebSocket)
     ┌──────┴──────┐
     │   Daemon    │     (homerund)
     └──────┬──────┘
            │ spawns/manages
   ┌────────┼────────┐
   │        │        │
┌──┴──┐ ┌──┴──┐ ┌──┴──┐
│Run 1│ │Run 2│ │Run N│  (GitHub runner processes)
└─────┘ └─────┘ └─────┘
```

All GitHub communication is outbound HTTPS from the runner processes. No inbound ports needed.

## Authentication

### OAuth (Primary)

1. User clicks "Login with GitHub" (Tauri) or runs `homerun login` (TUI)
2. Daemon spins up a temporary HTTP listener on a random localhost port for the OAuth callback
3. Browser opens to GitHub OAuth consent screen (requires a registered GitHub App)
4. User approves → GitHub redirects to the localhost callback
5. Daemon exchanges authorization code for access token, shuts down the temporary listener
6. Token stored in macOS Keychain via `security-framework` crate

This flow works identically from both the Tauri app and TUI — the daemon handles the callback listener in both cases.

### Personal Access Token (Fallback)

1. User clicks "Use Personal Access Token" or runs `homerun login --token <PAT>`
2. Daemon validates token against GitHub API
3. Token stored in macOS Keychain

### Required Scopes

For personal repos, only `repo` scope is needed. For organization repos, the additional `manage_runners:org` scope is required (not `admin:org` — that's overly broad). The app detects whether a repo belongs to an org and prompts for the additional scope if needed.

When using a GitHub App (OAuth flow), fine-grained permissions are configured on the app itself: `actions:write` + `administration:write` for runner registration.

### Security

- Tokens never stored in plaintext on disk — macOS Keychain only
- Unix socket for daemon communication — no network exposure
- Each runner gets an isolated working directory

## Daemon API

The daemon exposes a REST-style API over Unix socket at `~/.homerun/daemon.sock`, built with **Axum** (Rust async web framework). Axum supports HTTP, SSE, and WebSocket upgrade on the same listener, making it straightforward to serve all three protocols over the Unix socket.

### Endpoints

**Auth:**
- `POST /auth/github` — OAuth callback
- `POST /auth/token` — PAT login
- `DELETE /auth` — Logout
- `GET /auth/status` — Current user info

**Repos:**
- `GET /repos` — List user's GitHub repos
- `GET /repos/:id/workflows` — Workflow run history
- `GET /repos/:id/runners` — Registered runners for repo

**Runners:**
- `POST /runners` — Create and start a runner
- `DELETE /runners/:id` — Stop, deregister, and delete a runner
- `PATCH /runners/:id` — Update labels or mode
- `POST /runners/:id/start` — Start a stopped runner
- `POST /runners/:id/stop` — Stop a runner (graceful)
- `POST /runners/:id/restart` — Restart a runner

**Monitoring:**
- `GET /runners/:id/logs` — Stream logs (SSE)
- `GET /metrics` — Current CPU/RAM/disk per runner
- `GET /metrics/history?runner=:id&period=1h` — Historical metrics (in-memory ring buffer, last 24h)
- `WS /events` — Real-time status updates (WebSocket)

## Runner Lifecycle

### State Machine

```
Creating → Registering → Online ⇄ Busy
                           │         │
                           ↓         ↓
                        Offline   Stopping (waits for job to finish) → Offline
                           │
                           ↓
                        Deleting → (removed)

Any state → Error → auto-restart (up to 3 attempts) → Online | Error (if exhausted)
```

- **Stopping from Busy:** Graceful — waits for the current job to complete, then transitions to Offline. A force-stop option is available that kills the process immediately (job will show as failed on GitHub).
- **Deleting:** Always goes through Stop first (graceful by default), then deregisters and cleans up.

### What the Daemon Does

1. **Create:** Downloads GitHub Actions runner binary (cached after first download), creates isolated directory per runner at `~/.homerun/runners/<name>/`
2. **Register:** Calls GitHub API to get a registration token, runs `config.sh` with repo URL + token + labels
3. **Start:** App-managed mode → spawns `run.sh` as child process. Service mode → installs via `launchctl` (macOS launchd plist)
4. **Monitor:** Watches process health, captures stdout/stderr for log streaming, collects CPU/RAM via `sysinfo` crate
5. **Stop:** Sends graceful shutdown signal → waits for current job to finish → marks runner as Offline
6. **Delete:** Stops runner → deregisters from GitHub → removes local directory

### Runner Modes

- **App-managed:** Runner runs as a child process of the daemon. Stops when daemon stops (unless daemon is a service itself).
- **Background Service:** Runner installed as its own launchd plist. Starts and stops independently of the daemon. Survives daemon restarts and reboots. The daemon monitors service runners via launchd status checks but does not own their lifecycle.

Users can switch modes at any time from the UI or TUI. Switching from app-managed to service installs a launchd plist; switching back removes it and re-parents the process under the daemon.

## Tauri App (GUI)

### Tech Stack

- **Tauri 2.0** — Rust backend, native macOS webview
- **React** — frontend framework
- **TypeScript** — type safety

### Navigation

Sidebar with 5 main sections + settings:

1. **Dashboard** — overview stats (total runners, online, busy, CPU usage) + runners table
2. **Repositories** — browse GitHub repos, see runner counts, quick-add runners
3. **Runners** — detailed list with filtering and bulk actions
4. **Monitoring** — system-wide CPU/RAM/disk over time, per-runner resource graphs, alerts
5. **Workflow Runs** — recent workflow runs across all repos, local vs GitHub-hosted, duration, status

### Dashboard

- **Stats cards:** Total runners, online count, busy count, aggregate CPU usage
- **Runners table:** Name, repository, status (Online/Busy/Offline/Error), mode (App/Service), CPU usage, actions menu (⋯)
- **Actions menu:** Start, Stop, Restart, Delete (with confirmation dialog)
- **"+ New Runner" button** — opens the new runner flow

### Smart Repo Discovery

On first launch (and available anytime via a "Scan" button), HomeRun helps users find repos that need self-hosted runners:

1. **Local scan** — asks the user for their workspace folder (e.g., `~/workspace`). Recursively finds `.github/workflows/*.yml` files and checks for `runs-on: self-hosted`. Fast, catches repos the user actively develops.
2. **GitHub API scan** — fetches the user's repos and checks workflow files via the API for `runs-on: self-hosted`. Catches repos not cloned locally.

Results are presented as a suggested list: "These repos use self-hosted runners — set them up?" The user can select which repos to configure and how many runners per repo, then launch them all in one go.

This also works in the TUI: `homerun scan ~/workspace` for local, `homerun scan --remote` for API, or `homerun scan ~/workspace --remote` for both.

### New Runner Flow

Three-step wizard:

1. **Pick a Repository** — searchable list of user's GitHub repos (with discovered repos highlighted at the top), shows existing runner count per repo
2. **Configure** — runner name (auto-generated, customizable), labels (auto-detected: `self-hosted`, `macOS`, `ARM64` + custom), mode selection (app-managed vs background service), runner group (only shown for organization repos that support groups)
3. **Launch** — confirmation screen with summary, one-click "Launch Runner" button. Daemon handles everything: download, registration, configuration, start.

### Runner Detail View

- Runner info (name, repo, status, mode, labels, uptime, jobs completed/failed)
- Live log stream
- Resource usage graphs (CPU/RAM over time)
- Controls: start, stop, restart, edit labels, switch mode
- Danger zone: delete runner (with confirmation)

## TUI

### Tech Stack

- **Ratatui** — Rust TUI library (successor to tui-rs)
- Same binary as the CLI, launched with `homerun` (TUI mode) or `homerun --no-tui <command>` (plain output)

### Layout

- **Split pane:** Runner list (left) + detail panel (right)
- **Tab bar:** `[1] Runners` `[2] Repos` `[3] Workflows` `[4] Monitoring`
- **Status bar:** Keybindings + summary stats (online count, busy count, CPU)

### Keybindings

- `↑↓` — navigate runner list
- `Enter` — select/expand runner
- `a` — add new runner (interactive wizard)
- `d` — delete runner (with confirmation)
- `s` — start/stop toggle
- `r` — restart
- `l` — full log view
- `e` — edit labels
- `1-4` — switch tabs
- `q` — quit
- `?` — help

### Plain CLI Mode

`homerun --no-tui <command>` for scripting and piping:

```
homerun --no-tui list
homerun --no-tui add gifted --count 4 --labels ci,e2e --mode service
homerun --no-tui remove gifted-runner-2
homerun --no-tui status
```

## Notifications

macOS native notifications via the daemon. Configurable per-type:

- **Job completed** — when a workflow job finishes on a runner
- **Job failed** — when a workflow job fails on a runner
- **Runner crashed** — when a runner exits unexpectedly (includes auto-restart status)
- **High resource usage** — when CPU or RAM exceeds configurable threshold

Toggle each notification type on/off in settings (both Tauri app and TUI).

## Auto-Start on Boot

- **Daemon auto-start:** Installs a `launchd` plist (`~/Library/LaunchAgents/com.homerun.daemon.plist`) so the daemon starts on login. Toggle in settings.
- **Service runners:** Have their own launchd plists — they auto-start independently on boot, regardless of whether the daemon is running.
- **App-managed runners:** Stay offline until manually started via the UI, TUI, or CLI. Require the daemon to be running.

## Data Storage

- **Config:** `~/.homerun/config.toml` — daemon settings, notification preferences, auto-start config
- **Runner data:** `~/.homerun/runners/<name>/` — isolated directory per runner with GitHub runner binary and work directories
- **Logs:** `~/.homerun/logs/` — daemon and runner logs, rotated
- **Tokens:** macOS Keychain — never on disk
- **Runner binary cache:** `~/.homerun/cache/` — downloaded GitHub runner binaries, reused across runners
- **Metrics:** In-memory ring buffer (last 24h of per-runner CPU/RAM/disk samples). Not persisted to disk — resets on daemon restart.

## Error Handling

- **GitHub API unreachable:** Daemon retries with exponential backoff. Runners already online continue working (they talk to GitHub directly). UI/TUI show a "GitHub API unavailable" banner.
- **Token expired/revoked:** Daemon detects 401 responses, transitions to logged-out state, notifies the user to re-authenticate.
- **Daemon crash (app-managed runners):** App-managed runners die with the daemon. On daemon restart, it detects previously-running app-managed runners from config and offers to restart them. Service-mode runners are unaffected (managed by launchd).
- **Unix socket unavailable:** If a client (Tauri/TUI) can't connect to the socket, it shows "Daemon not running" with a button/command to start it.
- **Runner process crash:** Auto-restart up to 3 times with 10s backoff. After 3 failures, mark as Error state and notify the user.

## Runner Binary Updates

- On startup and daily, the daemon checks GitHub's runner releases API for new versions.
- If an update is available, a notification is shown in the UI/TUI: "Runner v2.X.Y available — update now?"
- Update is per-runner: stop runner → download new binary → replace → restart. Running jobs are not interrupted.
- Old binary versions are kept in cache until manually cleaned or no runner uses them.

## Disk Management

- Each runner's `_work` directory accumulates build artifacts over time.
- **Automatic cleanup:** After each job completes, the runner's built-in cleanup runs (GitHub runner default behavior).
- **Manual cleanup:** UI/TUI shows disk usage per runner. A "Clean workspace" action deletes the `_work` directory contents.
- **Disk usage alerts:** Configurable threshold (default: 90% disk usage). Triggers a macOS notification.
- **Delete runner:** Removes the entire runner directory including all cached data.

## Deployment

### Binaries

Two binaries ship:

1. **`homerund`** — the daemon. Can be started manually (`homerund`) or auto-started via launchd.
2. **`homerun`** — the TUI/CLI client. Launches TUI by default. `homerun --no-tui <command>` for plain CLI. Also includes `homerun daemon start|stop` as a convenience wrapper.

The Tauri app bundles `homerund` inside the `.app` package and installs it to `~/.homerun/bin/` on first launch. It also offers to install the `homerun` CLI to `/usr/local/bin/`.

### Distribution

- **Tauri app:** Distributed as a `.dmg` via GitHub Releases. Includes the daemon.
- **CLI/TUI only:** Installable via Homebrew (`brew install homerun`) for users who don't want the GUI. Includes both `homerun` and `homerund`.

## Requirements

- **macOS 13+ (Ventura)** — minimum for Tauri 2.0 webview support
- **No minimum hardware requirements** — the runners themselves are lightweight. Resource usage depends on the workflows being executed.

## Platform

- **macOS only** (initial release)
- **ARM64 + Intel** support
- Future: Windows + Linux

## Repository Standards

HomeRun is an open-source project and the repository should reflect best practices:

### Documentation
- **README.md** — project overview, screenshots/GIFs, quick start, installation (DMG + Homebrew), usage examples, architecture overview, contributing guide link
- **CONTRIBUTING.md** — how to set up the dev environment, coding standards, PR process
- **LICENSE** — MIT
- **CHANGELOG.md** — keep a changelog (auto-generated from conventional commits)
- **docs/** — detailed documentation: architecture deep-dive, API reference, configuration reference

### Testing
- **Rust (daemon/TUI):** `cargo test` with unit tests and integration tests. Use `mockall` for mocking GitHub API. Test coverage target: 80%+.
- **React (Tauri frontend):** Vitest + React Testing Library for component tests. Playwright for E2E tests of the Tauri app.
- **Coverage reports:** Generated in CI, posted as PR comments.

### CI/CD (GitHub Actions)
- **CI on PR:** Lint (clippy + eslint), format check (rustfmt + prettier), type check, unit tests, integration tests, coverage report
- **Release:** Automated via GitHub Releases. Tauri builds the `.dmg`, Homebrew formula updates automatically.
- **Conventional commits:** Enforced via commitlint. Powers auto-changelog and semantic versioning.

### Code Quality
- **Rust:** `clippy` (strict), `rustfmt`, `cargo audit` for dependency vulnerabilities
- **TypeScript:** ESLint, Prettier, `tsc --noEmit`
- **Pre-commit hooks:** Format, lint, type check (via `husky` + `lint-staged` for TS, `cargo fmt` + `cargo clippy` for Rust)

### Examples
- **examples/** directory with:
  - Basic setup walkthrough
  - Multi-runner configuration
  - CI workflow examples (`runs-on: self-hosted` templates)
  - Scripting with the CLI/TUI

### Project Management
- **GitHub Issues** with templates (bug report, feature request)
- **GitHub Discussions** enabled for Q&A
- **Labels** for triage (bug, enhancement, good first issue, help wanted)

## Tech Stack Summary

| Component | Technology |
|-----------|-----------|
| Daemon HTTP server | Rust + Axum (async web framework) |
| Daemon | Rust |
| Tauri App | Tauri 2.0 + React + TypeScript |
| TUI | Rust + Ratatui |
| Process management | Rust `tokio::process` + `sysinfo` crate |
| Auth token storage | macOS Keychain (via Tauri secure storage / `security-framework` crate) |
| GitHub API | `octocrab` crate (Rust GitHub API client) |
| Log streaming | Server-Sent Events (SSE) |
| Real-time updates | WebSocket |
| Auto-start | macOS launchd |
| Notifications | macOS native (`objc2` / `notify-rust` crate) |
