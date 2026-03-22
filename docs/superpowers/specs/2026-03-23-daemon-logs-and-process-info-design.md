# Daemon Logs & Process Info — Design Spec

**Issue:** [#36 — Display daemon logs in Tauri and TUI clients](https://github.com/aGallea/homerun/issues/36)
**Date:** 2026-03-23

## Summary

Add the ability to view `homerund` daemon logs and process info directly from both the Tauri desktop app and the TUI client. Daemon logs are currently only available via stdout/stderr. This feature surfaces them in-app with real-time streaming, historical viewing, filtering, and search. It also exposes daemon process metrics (PID, uptime, CPU, memory, child processes).

## Requirements

From the issue and comment:

- Stream daemon logs in real-time (tail -f style)
- Fetch historical logs with pagination
- Log level filtering (error, warn, info, debug, trace)
- Text search/filter
- Auto-scroll with ability to pause and scroll back
- Daemon process info: PID, uptime, CPU/memory usage
- Child process listing: PIDs, runner names, per-process CPU/memory

## Architecture Decisions

| Decision              | Choice                     | Rationale                                                                                      |
| --------------------- | -------------------------- | ---------------------------------------------------------------------------------------------- |
| Log storage           | File-based + in-memory     | Ring buffer for streaming performance, file for persistence across restarts                    |
| Streaming transport   | SSE (new endpoint)         | Matches existing runner log pattern, keeps concerns separate from WebSocket events             |
| Process info delivery | Extend `/metrics`          | Data is conceptually metrics, avoids new polling target, clients already consume this endpoint |
| TUI placement         | New "Daemon" tab (4th tab) | Keeps daemon concerns in one place, doesn't clutter monitoring view                            |
| Tauri placement       | New "Daemon" page          | Dedicated page, consistent with TUI approach, avoids cluttering recently redesigned dashboard  |
| Filtering             | Level filter + text search | Covers practical use cases without overcomplicating (no module filter or saved presets)        |
| Child process actions | Read-only                  | Runner management belongs on Runners tab; raw kill bypasses graceful lifecycle                 |

## Design

### 1. Daemon Log Capture Layer

A custom `tracing` layer intercepts daemon log events and routes them three ways.

**`DaemonLogEntry` type:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,       // "ERROR", "WARN", "INFO", "DEBUG", "TRACE"
    pub target: String,      // e.g., "homerund::runner::manager"
    pub message: String,
}
```

**`DaemonLogLayer`** implements `tracing_subscriber::Layer`:

- On each tracing event: creates a `DaemonLogEntry`, sends to broadcast channel (capacity 1024), pushes to ring buffer (max 2000 entries), appends JSON line to log file.
- Added alongside existing `fmt` layer: `registry().with(fmt_layer).with(daemon_log_layer)`.

**File logging:**

- Path: `~/.homerun/logs/daemon.log`, one JSON line per entry.
- Rotation: on daemon startup, rename existing file to `daemon.log.1` (keep 1 previous). Mid-session rotation at ~10MB.
- If file write fails (disk full, permissions): log to stderr, continue streaming via broadcast. Don't crash.

**New fields in `AppState`:**

- `daemon_log_tx: broadcast::Sender<DaemonLogEntry>` — for SSE subscribers
- `daemon_recent_logs: Arc<Mutex<VecDeque<DaemonLogEntry>>>` — ring buffer (max 2000)
- `daemon_start_time: Instant` — for uptime calculation
- `daemon_pid: u32` — captured at startup via `std::process::id()`

### 2. Daemon API Endpoints

**`GET /daemon/logs` — SSE streaming:**

- Streams `DaemonLogEntry` as JSON via SSE.
- Optional query param `?level=warn` — server-side filter, only sends entries at that level or above.
- Uses same pattern as existing `/runners/{id}/logs`.

**`GET /daemon/logs/recent` — REST historical:**

- Returns JSON array of `DaemonLogEntry` from ring buffer.
- Query params:
  - `level` — minimum level filter (default: all)
  - `limit` — max entries (default 500, max 2000)
  - `search` — case-insensitive substring match on message

**`GET /metrics` — extended response:**

Adds a `daemon` section to the existing response:

```json
{
  "system": {
    /* unchanged */
  },
  "runners": [
    /* unchanged */
  ],
  "daemon": {
    "pid": 12345,
    "uptime_seconds": 86400,
    "cpu_percent": 2.1,
    "memory_bytes": 45000000,
    "child_processes": [
      {
        "pid": 12350,
        "runner_id": "abc-123",
        "runner_name": "my-runner",
        "cpu_percent": 5.2,
        "memory_bytes": 150000000
      }
    ]
  }
}
```

Child process data comes from cross-referencing runner process PIDs (tracked by `RunnerManager`) with `sysinfo` process data.

**Router additions:**

```rust
.route("/daemon/logs", get(stream_daemon_logs))
.route("/daemon/logs/recent", get(recent_daemon_logs))
```

### 3. TUI — New "Daemon" Tab

Tab order: Runners | Repos | Monitoring | **Daemon**

Layout (top to bottom):

- **Info bar** — compact single row: PID, uptime, CPU, memory, child process count. Data from `/metrics`.
- **Filter bar** — log level selector (keyboard 1-5), text search (`/` key), follow toggle (`f` key).
- **Log viewer** — scrollable log entries with color-coded levels (green=INFO, yellow=WARN, red=ERROR), timestamp, module target, message. Highlighted current line.
- **Status bar** — keybinding hints: `↑↓ Scroll | / Search | f Follow | 1-5 Level | q Back`

**New files:**

- `crates/tui/src/ui/daemon.rs` — Daemon tab rendering
- Client methods in `crates/tui/src/client.rs`: `get_daemon_logs_recent()`, `subscribe_daemon_logs()`

### 4. Tauri Desktop App — New "Daemon" Page

New sidebar navigation item below Settings.

Layout (top to bottom):

- **Status cards row** — 4 cards: Status/PID, Uptime, CPU (with progress bar), Memory (with progress bar). Data from `/metrics`.
- **Child processes table** — collapsible section. Columns: Runner name, PID, CPU %, Memory. Read-only.
- **Logs panel** (fills remaining space) — toolbar with level filter pills, search input, follow toggle. Monospace log entries with color-coded levels, timestamps shortened to HH:MM:SS.

**New files:**

- `apps/desktop/src/pages/Daemon.tsx` — Daemon page component
- `apps/desktop/src/hooks/useDaemonLogs.ts` — SSE subscription lifecycle, log state, filter/search
- Tauri commands in `src-tauri/src/commands.rs`: `get_daemon_logs_recent`, `subscribe_daemon_logs`, `unsubscribe_daemon_logs`
- Type additions in `apps/desktop/src/api/types.ts`: `DaemonLogEntry`, `DaemonInfo`, `ChildProcess`

### 5. Data Flow

**Real-time streaming:**

```
tracing event → DaemonLogLayer → broadcast channel → SSE /daemon/logs
                                                      ↓
                TUI: SSE subscription → append to log view
                Tauri: SSE subscription → background task → Tauri event → React state → render
```

**Historical on page load:**

```
Client opens Daemon page → GET /daemon/logs/recent → populate log view
                        → GET /metrics → populate process info
                        → Subscribe SSE /daemon/logs → append new entries
```

**Process info polling:**

- TUI: polls `/metrics` on the same interval as existing monitoring tab
- Tauri: `useMetrics()` hook already polls — extend to expose `daemon` field

### 6. Error Handling

- **Broadcast overflow**: slow client gets `Lagged` error → reconnects, fetches `/daemon/logs/recent` to fill gap (same pattern as runner logs).
- **SSE disconnection**: dropping the broadcast receiver is sufficient cleanup. Client reconnects on next navigation or automatic retry.
- **Log file errors**: degrade gracefully — continue streaming via broadcast, log file write failure goes to stderr.
- **Metrics process info**: if `sysinfo` can't read daemon process (shouldn't happen for own PID), return null fields rather than failing the endpoint.
- **Empty states**: TUI shows "No daemon logs yet" when buffer empty. Tauri shows subtle placeholder. Child processes table shows "No runner processes" when none spawned.
