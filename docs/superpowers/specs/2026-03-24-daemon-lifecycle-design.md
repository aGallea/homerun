# Daemon Lifecycle Controls

**Issue:** #19
**Date:** 2026-03-24

## Problem

The HomeRun desktop app does not start the daemon automatically. Users must run `homerund` manually in a separate terminal before using any client. There are no controls to start, stop, or restart the daemon from any client.

## Design

### Daemon: Graceful Shutdown

Add `POST /daemon/shutdown` endpoint to the daemon API:

1. Send 202 Accepted response immediately (so the HTTP client doesn't hang)
2. Gracefully stop all runners (SIGTERM, wait 5s, SIGKILL)
3. Remove the socket file (`~/.homerun/daemon.sock`)
4. Exit the process

Wire SIGTERM and SIGINT signals to the same graceful shutdown path so `kill <pid>` also shuts down cleanly.

**launchd interaction:** If the daemon is managed by launchd (service is installed), `KeepAlive` will restart it after shutdown. The shutdown endpoint must check `is_daemon_installed()` and return an error: "Daemon is managed by launchd. Uninstall the service first or use `launchctl unload`." Clients should surface this error clearly.

### Daemon Startup Guard

On startup, homerund:

1. Checks if `~/.homerun/daemon.sock` already exists
2. If it does, attempts a health check against it
3. If the health check succeeds — exit with error: "Daemon already running"
4. If the health check fails — stale socket from a crashed daemon. Remove it and proceed

The socket bind itself is the final guard against races: only one process can successfully bind a Unix socket path. If two daemons race past the health check, the second will fail to bind and exit.

The current code in `server.rs` that unconditionally removes an existing socket must be replaced with this check-then-remove logic.

### Start Logic

All clients follow the same pattern:

1. Check if daemon is already running (socket exists + `GET /health` succeeds)
2. If already running, return early with "daemon already running"
3. If socket exists but health fails, remove stale socket
4. Find the `homerund` binary and spawn it as a **detached background process** with stdout/stderr redirected to `/dev/null` (the daemon has its own file-based logging via `DaemonLogLayer`)
5. Poll the socket with health checks (up to ~5 seconds, ~200ms interval)
6. Report success or timeout error

**Finding the binary:**

- **Tauri app**: bundled sidecar via `tauri-plugin-shell` (`Command::new_sidecar("binaries/homerund")`)
- **CLI / TUI**: look for `homerund` in `PATH`

### Stop Logic

Call `POST /daemon/shutdown` on the Unix socket. Wait for the socket file to disappear (up to ~5 seconds). If the endpoint is unreachable (daemon is dead but socket exists), remove the stale socket file.

**Force kill:** If the daemon doesn't shut down within 5 seconds, read the PID from the `/health` response and send SIGKILL. The `/health` endpoint will be extended to include `pid` in its response.

### Restart Logic

Stop (as above), then start (as above).

### Shared Code

The `homerun` crate (CLI/TUI) gets a `daemon_lifecycle` module with:

- `start_daemon(binary: &str) -> Result<()>` — spawn detached process, poll health
- `stop_daemon(socket: &Path) -> Result<()>` — call shutdown endpoint, wait for socket removal
- `restart_daemon(binary: &str, socket: &Path) -> Result<()>` — stop then start

The Tauri backend implements its own start logic using the sidecar API but shares the same stop logic (call shutdown endpoint over the socket).

## Client Integration

### CLI

Add `homerun daemon start|stop|restart` subcommands:

- New `Daemon` variant in `CliCommand` with `DaemonAction` sub-enum (`Start`, `Stop`, `Restart`)
- `start` and `restart` skip the existing daemon health check gate (daemon may not be running)
- `stop` calls the shutdown endpoint

### TUI

- Daemon tab already exists (shows logs). Existing keybindings (`1-5` for log levels, `f` for follow) are preserved
- Add keybindings scoped to the Daemon tab: `s` to start, `x` to stop, `r` to restart
- Show daemon connection status (running/stopped) in the tab
- When daemon is stopped, show a "Press s to start daemon" prompt instead of logs
- **Behavior change:** The TUI currently exits immediately if the daemon is unreachable at startup. This must change — the TUI should launch in a "disconnected" mode showing the Daemon tab with the start prompt

### Desktop App

**Auto-start on launch:**

- In Tauri `setup` hook (`lib.rs`): check daemon health, if not running spawn sidecar, poll until ready
- If sidecar is not found or fails to start, the app still launches — the existing "Unable to connect" banner handles the degraded state

**Daemon page:**

- Add Start / Stop / Restart buttons in the status area
- Buttons are enabled/disabled based on daemon state (e.g., Stop disabled when daemon is down)

**Error banner:**

- The "Unable to connect to daemon" banner gets a "Start daemon" button
- Button calls the `start_daemon` IPC command with loading state

**Tauri IPC commands:**

- `start_daemon` — spawn sidecar, poll health
- `stop_daemon` — call shutdown endpoint
- `restart_daemon` — stop then start

## Error Handling

- **Binary not found**: "homerund not found in PATH" (CLI/TUI) / "sidecar binary missing" (desktop)
- **Start timeout**: "Daemon failed to start within 5 seconds — check logs at ~/.homerun/logs/"
- **Stop when launchd active**: "Daemon is managed by launchd. Uninstall the service first."
- **Stop timeout**: Force kill the daemon process via SIGKILL using PID from health response
- **Already running**: "Daemon is already running"
- **Stale socket**: Remove socket and proceed with start
