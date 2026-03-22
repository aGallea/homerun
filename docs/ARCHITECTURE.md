# Architecture

HomeRun is a macOS tool for managing GitHub Actions self-hosted runners. It has three components that share a background daemon.

## Components

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

- **homerund** (Rust/Axum) — background daemon on a Unix socket. Manages runner lifecycle, authenticates with GitHub, streams logs, collects metrics.
- **homerun** (Rust/Ratatui) — TUI and CLI client. Keyboard-driven interface for managing runners.
- **Tauri desktop app** (React + TypeScript) — GUI client with the same capabilities as the TUI.

All clients talk to the daemon over the Unix socket using REST, SSE (log streaming), and WebSocket (real-time events). The daemon spawns and monitors runner processes.

## How Runners Work

Runners are **native child processes** of the daemon — not Docker containers, not VMs. Each runner is an instance of the [official GitHub Actions runner](https://github.com/actions/runner), the same binary you would install manually.

### Runner Creation Flow

1. User requests a new runner via the GUI, TUI, or CLI
2. Daemon downloads the GitHub Actions runner binary (or uses a cached version from `~/.homerun/cache/`)
3. Copies the binary to a per-runner work directory (`~/.homerun/runners/{id}/`)
4. Obtains a **registration token** from the GitHub API
5. Runs `config.sh` to register the runner with the target repository
6. Spawns `run.sh` as a child process in unattended mode
7. Runner connects to GitHub via outbound HTTPS and waits for jobs

### What Happens When a Job Runs

The runner process receives a job from GitHub, executes it on the host machine, and reports results back. The daemon monitors the runner's stdout to detect job lifecycle events:

- `"Running job: {name}"` → state changes to Busy
- `"completed with result: Succeeded"` → state changes back to Online, job counter incremented

### Runner State Machine

```
Creating → Registering → Online ⇄ Busy
                           ↓
                        Stopping → Offline → Registering (restart)
                                     ↓
                                   Deleting
Any state → Error → Registering (retry) | Offline
```

### Runner Groups

Multiple runners can be created in a batch and managed as a group. Group runners share a `group_id` and support bulk actions (start all, stop all, restart all, delete all) and declarative scaling (set target count, daemon adds or removes runners).

## Authentication

HomeRun authenticates with GitHub to access the API (listing repos, obtaining registration tokens, deregistering runners).

Two methods are supported:

1. **GitHub Device Flow** (recommended) — user authorizes via browser, no token to copy-paste
2. **Personal Access Token** — user provides a PAT directly

The token is stored in the **macOS Keychain** and restored automatically on daemon restart.

## Communication

The daemon exposes an HTTP API over a Unix socket at `~/.homerun/daemon.sock`.

| Protocol  | Purpose            | Example                                   |
| --------- | ------------------ | ----------------------------------------- |
| REST      | CRUD operations    | `POST /runners`, `DELETE /runners/{id}`   |
| SSE       | Live log streaming | `GET /runners/{id}/logs`                  |
| WebSocket | Real-time events   | `GET /events` (state changes, job events) |

Clients poll every 2 seconds as a fallback and use WebSocket events for immediate updates.

## Storage

```
~/.homerun/
├── daemon.sock              # Unix socket
├── config.toml              # Daemon configuration
├── runners.json             # Persisted runner configs
├── runners/
│   └── {runner-id}/         # Per-runner work directory
│       ├── .runner          # GitHub registration config
│       ├── run.sh           # Runner executable
│       └── _work/           # Job workspace
├── cache/
│   └── runner-{version}/    # Cached runner binary
└── logs/
```

Runner configurations are persisted to `runners.json`. On daemon restart, runners are loaded in Offline state and can be started again without re-registration (the `.runner` config file persists).

## Process Management

Each runner is a `tokio::process::Child`. The daemon:

- Tracks PIDs and process handles
- Captures stdout/stderr for log streaming
- Spawns a monitoring task per runner that detects process exit
- Sends SIGTERM for graceful shutdown (10s timeout), then SIGKILL
- Collects per-process CPU and memory metrics via `sysinfo`

## GitHub API Integration

The daemon uses the `octocrab` crate to interact with GitHub:

- **List repositories** — fetches repos accessible to the authenticated user
- **Registration tokens** — `POST /repos/{owner}/{repo}/actions/runners/registration-token` to get a temporary token for `config.sh`
- **Deregistration** — `config.sh remove` to cleanly unregister a runner on deletion
- **Workflow scanning** — reads `.github/workflows/` files to find repos using self-hosted runners

All GitHub communication from the runner processes is outbound HTTPS. No inbound ports or firewall rules needed.

## Why Rust

- **Low overhead daemon** — runs 24/7 managing child processes; Rust's zero-cost abstractions and no GC mean minimal resource usage
- **Reliable concurrency** — tokio async runtime handles multiple runner processes, log streams, WebSocket connections, and metrics collection without thread-safety bugs
- **Single language** — daemon, TUI, and CLI share code and types; no serialization boundaries between them
- **Native macOS integration** — FFI to macOS Keychain (`security-framework`), launchd, and system notifications without a runtime
- **Tauri backend** — the desktop app's Rust backend is a thin IPC layer that reuses the same Unix socket client pattern as the TUI
