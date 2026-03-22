# Daemon Logs & Process Info Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface `homerund` daemon logs and process info in both the TUI and Tauri desktop clients, with real-time streaming, historical viewing, level filtering, and text search.

**Architecture:** A custom `tracing` layer captures daemon log events and routes them to a broadcast channel (SSE streaming), ring buffer (recent history), and file (`~/.homerun/logs/daemon.log`). Two new endpoints (`/daemon/logs` SSE, `/daemon/logs/recent` REST) serve logs. The existing `/metrics` endpoint is extended with daemon process info. A new "Daemon" tab in the TUI and "Daemon" page in the Tauri app consume these.

**Tech Stack:** Rust (tracing, axum, tokio broadcast, sysinfo), React 19 + TypeScript (Tauri frontend), Ratatui (TUI)

**Spec:** `docs/superpowers/specs/2026-03-23-daemon-logs-and-process-info-design.md`

---

## File Structure

### New Files

| File                                      | Responsibility                                                                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `crates/daemon/src/logging.rs`            | `DaemonLogEntry` type, `DaemonLogLayer` (tracing layer), `DaemonLogState` (broadcast + ring buffer + file writer) |
| `crates/daemon/src/api/daemon_logs.rs`    | `GET /daemon/logs` SSE endpoint, `GET /daemon/logs/recent` REST endpoint                                          |
| `crates/daemon/tests/daemon_logs_test.rs` | Tests for daemon log capture and API endpoints                                                                    |
| `crates/tui/src/ui/daemon.rs`             | TUI "Daemon" tab rendering                                                                                        |
| `apps/desktop/src/pages/Daemon.tsx`       | Tauri "Daemon" page component                                                                                     |
| `apps/desktop/src/hooks/useDaemonLogs.ts` | Hook for daemon log SSE subscription and state management                                                         |

### Modified Files

| File                                     | Changes                                                                                   |
| ---------------------------------------- | ----------------------------------------------------------------------------------------- |
| `crates/daemon/src/main.rs`              | Add `DaemonLogLayer` to tracing subscriber, pass `DaemonLogState` to `AppState`           |
| `crates/daemon/src/server.rs`            | Add daemon log fields to `AppState`, add new routes to router                             |
| `crates/daemon/src/api/mod.rs`           | Add `pub mod daemon_logs;`                                                                |
| `crates/daemon/src/metrics.rs`           | Add `DaemonMetrics` and `ChildProcessInfo` types, add `daemon_metrics()` method           |
| `crates/daemon/src/api/metrics.rs`       | Include `daemon` field in metrics response                                                |
| `crates/daemon/src/runner/mod.rs`        | Expose runner PIDs and names for child process listing                                    |
| `crates/daemon/Cargo.toml`               | No new deps needed (tracing, sysinfo, chrono already present)                             |
| `crates/tui/src/app.rs`                  | Add `Daemon` variant to `Tab` enum, add daemon log state fields to `App`                  |
| `crates/tui/src/client.rs`               | Add `get_daemon_logs_recent()` method, add `DaemonLogEntry` type                          |
| `crates/tui/src/ui/mod.rs`               | Add `pub mod daemon;`, add rendering call for Daemon tab                                  |
| `apps/desktop/src-tauri/src/client.rs`   | Add `get_daemon_logs_recent()` method                                                     |
| `apps/desktop/src-tauri/src/commands.rs` | Add `get_daemon_logs_recent` command                                                      |
| `apps/desktop/src-tauri/src/lib.rs`      | Register new command in `invoke_handler`                                                  |
| `apps/desktop/src/api/types.ts`          | Add `DaemonLogEntry`, `DaemonMetrics`, `ChildProcessInfo` types, extend `MetricsResponse` |
| `apps/desktop/src/api/commands.ts`       | Add `getDaemonLogsRecent()` wrapper                                                       |
| `apps/desktop/src/hooks/useMetrics.ts`   | No changes needed (already returns full `MetricsResponse`)                                |
| `apps/desktop/src/App.tsx`               | Add `/daemon` route                                                                       |
| `apps/desktop/src/components/Layout.tsx` | Add "Daemon" sidebar nav item                                                             |

---

## Task 1: DaemonLogEntry Type and DaemonLogState

**Files:**

- Create: `crates/daemon/src/logging.rs`
- Modify: `crates/daemon/src/server.rs:18-25` (AppState)
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Create `logging.rs` with `DaemonLogEntry` and `DaemonLogState`**

```rust
// crates/daemon/src/logging.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub const RECENT_DAEMON_LOGS_MAX: usize = 2000;
pub const DAEMON_LOG_BROADCAST_CAPACITY: usize = 1024;
pub const DAEMON_LOG_FILE_MAX_BYTES: u64 = 10 * 1024 * 1024; // 10MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Clone)]
pub struct DaemonLogState {
    pub log_tx: Arc<broadcast::Sender<DaemonLogEntry>>,
    pub recent_logs: Arc<Mutex<VecDeque<DaemonLogEntry>>>,
    log_file_path: PathBuf,
}

impl DaemonLogState {
    pub fn new(log_dir: &Path) -> Self {
        let log_file_path = log_dir.join("daemon.log");

        // Rotate existing log file on startup
        if log_file_path.exists() {
            let backup = log_dir.join("daemon.log.1");
            let _ = fs::rename(&log_file_path, &backup);
        }

        let (log_tx, _) = broadcast::channel(DAEMON_LOG_BROADCAST_CAPACITY);

        Self {
            log_tx: Arc::new(log_tx),
            recent_logs: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_DAEMON_LOGS_MAX))),
            log_file_path,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DaemonLogEntry> {
        self.log_tx.subscribe()
    }

    pub async fn push(&self, entry: DaemonLogEntry) {
        // Broadcast to SSE subscribers
        let _ = self.log_tx.send(entry.clone());

        // Push to ring buffer
        let mut recent = self.recent_logs.lock().await;
        if recent.len() >= RECENT_DAEMON_LOGS_MAX {
            recent.pop_front();
        }
        recent.push_back(entry.clone());
        drop(recent);

        // Append to file
        self.append_to_file(&entry);
    }

    fn append_to_file(&self, entry: &DaemonLogEntry) {
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
        else {
            eprintln!("Failed to open daemon log file: {:?}", self.log_file_path);
            return;
        };

        // Check file size for mid-session rotation
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > DAEMON_LOG_FILE_MAX_BYTES {
                drop(file);
                let backup = self.log_file_path.with_extension("log.1");
                let _ = fs::rename(&self.log_file_path, &backup);
                let Ok(new_file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.log_file_path)
                else {
                    return;
                };
                file = new_file;
            }
        }

        if let Ok(json) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{}", json);
        }
    }

    pub async fn get_recent(
        &self,
        level: Option<&str>,
        limit: usize,
        search: Option<&str>,
    ) -> Vec<DaemonLogEntry> {
        let recent = self.recent_logs.lock().await;
        recent
            .iter()
            .filter(|e| {
                if let Some(min_level) = level {
                    level_value(&e.level) >= level_value(min_level)
                } else {
                    true
                }
            })
            .filter(|e| {
                if let Some(s) = search {
                    e.message.to_lowercase().contains(&s.to_lowercase())
                } else {
                    true
                }
            })
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

pub fn level_value(level: &str) -> u8 {
    match level.to_uppercase().as_str() {
        "ERROR" => 5,
        "WARN" => 4,
        "INFO" => 3,
        "DEBUG" => 2,
        "TRACE" => 1,
        _ => 0,
    }
}
```

- [ ] **Step 2: Add the module to `crates/daemon/src/lib.rs` or `main.rs`**

Add `pub mod logging;` to the daemon's module declarations.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p homerund`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/logging.rs
git commit -m "feat: add DaemonLogEntry type and DaemonLogState"
```

---

## Task 2: DaemonLogLayer (tracing Layer)

**Files:**

- Modify: `crates/daemon/src/logging.rs`

- [ ] **Step 1: Add `DaemonLogLayer` to `logging.rs`**

Add below the existing code:

```rust
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub struct DaemonLogLayer {
    state: DaemonLogState,
    runtime: tokio::runtime::Handle,
}

impl DaemonLogLayer {
    pub fn new(state: DaemonLogState, runtime: tokio::runtime::Handle) -> Self {
        Self { state, runtime }
    }
}

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

impl<S: Subscriber> Layer<S> for DaemonLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        let entry = DaemonLogEntry {
            timestamp: Utc::now(),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
        };

        let state = self.state.clone();
        self.runtime.spawn(async move {
            state.push(entry).await;
        });
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p homerund`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/logging.rs
git commit -m "feat: add DaemonLogLayer tracing layer"
```

---

## Task 3: Wire DaemonLogLayer into Daemon Startup

**Files:**

- Modify: `crates/daemon/src/main.rs:6-10`
- Modify: `crates/daemon/src/server.rs:18-25` (AppState), `server.rs:28-37` (AppState::new), `server.rs:55-105` (create_router)

- [ ] **Step 1: Add `DaemonLogState` to `AppState`**

In `crates/daemon/src/server.rs`, add to the `AppState` struct:

```rust
pub daemon_logs: DaemonLogState,
pub daemon_start_time: std::time::Instant,
pub daemon_pid: u32,
```

Update `AppState::new()` to accept and store `DaemonLogState`, and capture `std::time::Instant::now()` and `std::process::id()`.

- [ ] **Step 2: Update `main.rs` to create `DaemonLogState` and layer**

Replace the tracing setup in `crates/daemon/src/main.rs`:

```rust
use homerund::logging::{DaemonLogLayer, DaemonLogState};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let config = homerund::config::Config::default();
    config.ensure_dirs()?;

    let daemon_log_state = DaemonLogState::new(&config.log_dir());
    let runtime = tokio::runtime::Handle::current();

    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let daemon_layer = DaemonLogLayer::new(daemon_log_state.clone(), runtime);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(daemon_layer)
        .init();

    tracing::info!("HomeRun daemon starting...");
    homerund::server::serve(config, daemon_log_state).await
}
```

Update `serve()` signature to accept `DaemonLogState` and pass it to `AppState::new()`.

- [ ] **Step 3: Verify it compiles and runs**

Run: `cargo check -p homerund`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/main.rs crates/daemon/src/server.rs
git commit -m "feat: wire DaemonLogLayer into daemon startup"
```

---

## Task 4: Daemon Log API Endpoints

**Files:**

- Create: `crates/daemon/src/api/daemon_logs.rs`
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/server.rs:55-105` (router)

- [ ] **Step 1: Create `daemon_logs.rs` with SSE and recent endpoints**

```rust
// crates/daemon/src/api/daemon_logs.rs
use crate::logging::{level_value, DaemonLogEntry};
use crate::server::AppState;
use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::stream::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub level: Option<String>,
}

pub async fn stream_daemon_logs(
    State(state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.daemon_logs.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |entry| {
        let level_filter = query.level.clone();
        async move {
            match entry {
                Ok(log) => {
                    if let Some(ref min_level) = level_filter {
                        if level_value(&log.level) < level_value(min_level) {
                            return None;
                        }
                    }
                    let json = serde_json::to_string(&log).unwrap_or_default();
                    Some(Ok(Event::default().data(json)))
                }
                _ => None,
            }
        }
    });
    Sse::new(stream)
}

#[derive(Deserialize)]
pub struct RecentQuery {
    pub level: Option<String>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

pub async fn recent_daemon_logs(
    State(state): State<AppState>,
    Query(query): Query<RecentQuery>,
) -> Json<Vec<DaemonLogEntry>> {
    let limit = query.limit.unwrap_or(500).min(2000);
    let entries = state
        .daemon_logs
        .get_recent(query.level.as_deref(), limit, query.search.as_deref())
        .await;
    Json(entries)
}
```

- [ ] **Step 2: Add module declaration**

In `crates/daemon/src/api/mod.rs`, add: `pub mod daemon_logs;`

- [ ] **Step 3: Add routes to router**

In `crates/daemon/src/server.rs` `create_router()`, add:

```rust
.route("/daemon/logs", get(api::daemon_logs::stream_daemon_logs))
.route("/daemon/logs/recent", get(api::daemon_logs::recent_daemon_logs))
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p homerund`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/api/daemon_logs.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add daemon log SSE and recent logs API endpoints"
```

---

## Task 5: Extend Metrics with Daemon Process Info

**Files:**

- Modify: `crates/daemon/src/metrics.rs:31-45` (types), `metrics.rs:58-140` (methods)
- Modify: `crates/daemon/src/api/metrics.rs:5-22`
- Modify: `crates/daemon/src/runner/mod.rs` (expose PID + name)

- [ ] **Step 1: Add daemon metric types to `metrics.rs`**

Add to `crates/daemon/src/metrics.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DaemonMetrics {
    pub pid: u32,
    pub uptime_seconds: u64,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub child_processes: Vec<ChildProcessInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChildProcessInfo {
    pub pid: u32,
    pub runner_id: String,
    pub runner_name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}
```

- [ ] **Step 2: Add `daemon_metrics()` method to `MetricsCollector`**

```rust
pub fn daemon_metrics(
    &self,
    daemon_pid: u32,
    uptime: std::time::Duration,
    runners: &[(String, String, Option<u32>)], // (runner_id, runner_name, pid)
) -> DaemonMetrics {
    let system = self.system.lock().unwrap();
    let pid = sysinfo::Pid::from_u32(daemon_pid);

    let (cpu_percent, memory_bytes) = system
        .process(pid)
        .map(|p| (p.cpu_usage(), p.memory()))
        .unwrap_or((0.0, 0));

    let child_processes = runners
        .iter()
        .filter_map(|(id, name, runner_pid)| {
            runner_pid.and_then(|rpid| {
                self.runner_metrics(rpid).map(|m| ChildProcessInfo {
                    pid: rpid,
                    runner_id: id.clone(),
                    runner_name: name.clone(),
                    cpu_percent: m.cpu_percent,
                    memory_bytes: m.memory_bytes,
                })
            })
        })
        .collect();

    DaemonMetrics {
        pid: daemon_pid,
        uptime_seconds: uptime.as_secs(),
        cpu_percent,
        memory_bytes,
        child_processes,
    }
}
```

- [ ] **Step 3: Expose runner PID and name from `RunnerManager`**

Add a method to `RunnerManager` in `crates/daemon/src/runner/mod.rs`:

```rust
pub async fn runner_pids_and_names(&self) -> Vec<(String, String, Option<u32>)> {
    let runners = self.runners.read().await;
    runners
        .values()
        .map(|r| (r.config.id.clone(), r.config.name.clone(), r.pid))
        .collect()
}
```

- [ ] **Step 4: Update metrics endpoint to include daemon info**

In `crates/daemon/src/api/metrics.rs`, update `get_metrics()`:

```rust
pub async fn get_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let system = state.metrics.system_snapshot();
    let runners = state.runner_manager.list().await;
    state.metrics.refresh_processes();

    let runner_metrics: Vec<_> = runners
        .iter()
        .filter_map(|r| {
            r.pid.and_then(|pid| {
                state.metrics.runner_metrics(pid).map(|mut m| {
                    m.runner_id = r.config.id.clone();
                    m
                })
            })
        })
        .collect();

    let runner_pids = state.runner_manager.runner_pids_and_names().await;
    let uptime = state.daemon_start_time.elapsed();
    let daemon = state.metrics.daemon_metrics(state.daemon_pid, uptime, &runner_pids);

    Json(serde_json::json!({
        "system": system,
        "runners": runner_metrics,
        "daemon": daemon
    }))
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p homerund`
Expected: compiles with no errors

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/metrics.rs crates/daemon/src/api/metrics.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: extend metrics endpoint with daemon process info"
```

---

## Task 6: Daemon Integration Tests

**Files:**

- Create: `crates/daemon/tests/daemon_logs_test.rs`

- [ ] **Step 1: Write integration tests for daemon log state**

```rust
// crates/daemon/tests/daemon_logs_test.rs
use homerund::logging::{DaemonLogEntry, DaemonLogState};
use chrono::Utc;
use tempfile::TempDir;

#[tokio::test]
async fn test_push_and_get_recent() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    let entry = DaemonLogEntry {
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "hello world".to_string(),
    };
    state.push(entry).await;

    let recent = state.get_recent(None, 500, None).await;
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].message, "hello world");
}

#[tokio::test]
async fn test_level_filtering() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for (level, msg) in [("DEBUG", "debug msg"), ("INFO", "info msg"), ("WARN", "warn msg"), ("ERROR", "error msg")] {
        state.push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: level.to_string(),
            target: "test".to_string(),
            message: msg.to_string(),
        }).await;
    }

    let warn_and_above = state.get_recent(Some("WARN"), 500, None).await;
    assert_eq!(warn_and_above.len(), 2);
    assert_eq!(warn_and_above[0].level, "WARN");
    assert_eq!(warn_and_above[1].level, "ERROR");
}

#[tokio::test]
async fn test_text_search() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    state.push(DaemonLogEntry {
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "Runner started successfully".to_string(),
    }).await;
    state.push(DaemonLogEntry {
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "Auth token loaded".to_string(),
    }).await;

    let results = state.get_recent(None, 500, Some("runner")).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].message.contains("Runner"));
}

#[tokio::test]
async fn test_ring_buffer_cap() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for i in 0..2100 {
        state.push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: format!("msg {}", i),
        }).await;
    }

    let recent = state.get_recent(None, 2000, None).await;
    assert_eq!(recent.len(), 2000);
    // Oldest entries should have been dropped
    assert_eq!(recent[0].message, "msg 100");
}

#[tokio::test]
async fn test_broadcast_subscription() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());
    let mut rx = state.subscribe();

    state.push(DaemonLogEntry {
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "broadcast test".to_string(),
    }).await;

    let received = rx.recv().await.unwrap();
    assert_eq!(received.message, "broadcast test");
}

#[tokio::test]
async fn test_log_file_rotation_on_startup() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("daemon.log");
    std::fs::write(&log_path, "old log content").unwrap();

    let _state = DaemonLogState::new(tmp.path());

    // Old file should have been rotated
    let backup = tmp.path().join("daemon.log.1");
    assert!(backup.exists());
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "old log content");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p homerund -- daemon_logs`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/tests/daemon_logs_test.rs
git commit -m "test: add daemon log state integration tests"
```

---

## Task 7: TUI — Add Daemon Tab State

**Files:**

- Modify: `crates/tui/src/app.rs:39-75` (Tab enum), `app.rs:77-92` (App struct), `app.rs:8-23` (Action enum)
- Modify: `crates/tui/src/client.rs` (add DaemonLogEntry type and client methods)

- [ ] **Step 1: Add `DaemonLogEntry` type and client method**

In `crates/tui/src/client.rs`, add the type alongside the existing response types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}
```

Add a `DaemonMetrics` type and extend `MetricsResponse`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonMetrics {
    pub pid: u32,
    pub uptime_seconds: u64,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub child_processes: Vec<ChildProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildProcessInfo {
    pub pid: u32,
    pub runner_id: String,
    pub runner_name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}
```

Add `daemon: Option<DaemonMetrics>` to the existing `MetricsResponse` struct.

Add client method:

```rust
pub async fn get_daemon_logs_recent(
    &self,
    level: Option<&str>,
    limit: Option<usize>,
    search: Option<&str>,
) -> Result<Vec<DaemonLogEntry>> {
    let mut url = "/daemon/logs/recent?".to_string();
    if let Some(l) = level { url.push_str(&format!("level={}&", l)); }
    if let Some(n) = limit { url.push_str(&format!("limit={}&", n)); }
    if let Some(s) = search { url.push_str(&format!("search={}&", urlencoding::encode(s))); }
    let resp = self.get(&url).await?;
    let entries: Vec<DaemonLogEntry> = serde_json::from_slice(&resp)?;
    Ok(entries)
}
```

- [ ] **Step 2: Add `Daemon` variant to `Tab` enum and daemon fields to `App`**

In `crates/tui/src/app.rs`:

Add `Daemon` to the `Tab` enum:

```rust
pub enum Tab {
    Runners,
    Repos,
    Monitoring,
    Daemon,
}
```

Add daemon state fields to `App`:

```rust
pub daemon_logs: Vec<DaemonLogEntry>,
pub daemon_log_scroll: usize,
pub daemon_follow: bool,
pub daemon_log_level: String,     // "TRACE", "DEBUG", "INFO", "WARN", "ERROR"
pub daemon_search: String,
pub daemon_searching: bool,       // true when search input is active
```

Add `RefreshDaemonLogs` to the `Action` enum. Update tab cycling (next/prev tab logic) to include the `Daemon` variant.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p homerun`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/client.rs
git commit -m "feat(tui): add Daemon tab state and client methods"
```

---

## Task 8: TUI — Daemon Tab Rendering

**Files:**

- Create: `crates/tui/src/ui/daemon.rs`
- Modify: `crates/tui/src/ui/mod.rs`
- Modify: `crates/tui/src/ui/tabs.rs` (add Daemon tab label)

- [ ] **Step 1: Create `daemon.rs` UI component**

```rust
// crates/tui/src/ui/daemon.rs
use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw_daemon_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Info bar
            Constraint::Length(1), // Filter bar
            Constraint::Min(5),   // Logs
        ])
        .split(area);

    draw_info_bar(f, app, chunks[0]);
    draw_filter_bar(f, app, chunks[1]);
    draw_log_viewer(f, app, chunks[2]);
}

fn draw_info_bar(f: &mut Frame, app: &App, area: Rect) {
    let info = if let Some(ref metrics) = app.metrics {
        if let Some(ref daemon) = metrics.daemon {
            let uptime = format_uptime(daemon.uptime_seconds);
            let mem = format_bytes(daemon.memory_bytes);
            Line::from(vec![
                Span::styled(" PID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(daemon.pid.to_string(), Style::default().fg(Color::Green)),
                Span::styled("  Uptime: ", Style::default().fg(Color::DarkGray)),
                Span::styled(uptime, Style::default().fg(Color::Green)),
                Span::styled("  CPU: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:.1}%", daemon.cpu_percent), Style::default().fg(Color::Green)),
                Span::styled("  Memory: ", Style::default().fg(Color::DarkGray)),
                Span::styled(mem, Style::default().fg(Color::Green)),
                Span::styled("  Children: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} processes", daemon.child_processes.len()),
                    Style::default().fg(Color::Green),
                ),
            ])
        } else {
            Line::from(Span::styled(" Daemon info unavailable", Style::default().fg(Color::DarkGray)))
        }
    } else {
        Line::from(Span::styled(" Loading...", Style::default().fg(Color::DarkGray)))
    };

    let widget = Paragraph::new(info)
        .style(Style::default().bg(Color::Rgb(15, 52, 96)));
    f.render_widget(widget, area);
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let levels = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
    let mut spans = vec![Span::styled(" Level: ", Style::default().fg(Color::DarkGray))];

    for level in &levels {
        let style = if *level == app.daemon_log_level {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("[{}] ", level), style));
    }

    spans.push(Span::styled("│ Search: ", Style::default().fg(Color::DarkGray)));
    let search_text = if app.daemon_search.is_empty() {
        "".to_string()
    } else {
        app.daemon_search.clone()
    };
    spans.push(Span::styled(search_text, Style::default().fg(Color::Green)));

    spans.push(Span::raw("  "));
    let follow_style = if app.daemon_follow {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    spans.push(Span::styled(
        if app.daemon_follow { "⟡ Follow" } else { "  Follow" },
        follow_style,
    ));

    let widget = Paragraph::new(Line::from(spans));
    f.render_widget(widget, area);
}

fn draw_log_viewer(f: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height as usize;

    if app.daemon_logs.is_empty() {
        let msg = Paragraph::new(" No daemon logs yet")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(msg, area);
        return;
    }

    let lines: Vec<Line> = app
        .daemon_logs
        .iter()
        .map(|entry| {
            let level_color = match entry.level.as_str() {
                "ERROR" => Color::Red,
                "WARN" => Color::Yellow,
                "INFO" => Color::Green,
                "DEBUG" => Color::Blue,
                "TRACE" => Color::DarkGray,
                _ => Color::White,
            };

            // Extract HH:MM:SS from timestamp
            let time = if entry.timestamp.len() >= 19 {
                &entry.timestamp[11..19]
            } else {
                &entry.timestamp
            };

            // Shorten target (last segment)
            let target_short = entry
                .target
                .rsplit("::")
                .next()
                .unwrap_or(&entry.target);

            Line::from(vec![
                Span::styled(
                    format!("{} ", time),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<5} ", entry.level),
                    Style::default().fg(level_color),
                ),
                Span::styled(
                    format!("{:<12} ", target_short),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(&entry.message),
            ])
        })
        .collect();

    let scroll = if app.daemon_follow {
        lines.len().saturating_sub(visible_height)
    } else {
        app.daemon_log_scroll
    };

    let widget = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(widget, area);
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{} MB", bytes / 1_048_576)
    } else {
        format!("{} KB", bytes / 1024)
    }
}
```

- [ ] **Step 2: Register module and wire into draw function**

In `crates/tui/src/ui/mod.rs`, add `pub mod daemon;` and add a match arm for `Tab::Daemon` in the main draw function that calls `daemon::draw_daemon_tab()`.

In `crates/tui/src/ui/tabs.rs`, add "Daemon" to the tab labels list.

- [ ] **Step 3: Wire daemon log fetching into the TUI event loop**

In the main TUI event loop (wherever `RefreshMetrics` action is handled), add handling for `RefreshDaemonLogs` — call `client.get_daemon_logs_recent()` and update `app.daemon_logs`. Trigger this refresh when the Daemon tab is active, on the same polling interval as metrics.

Add keyboard handling for the Daemon tab:

- `1`-`5` keys: set `daemon_log_level` to TRACE/DEBUG/INFO/WARN/ERROR
- `/`: toggle `daemon_searching` mode
- `f`: toggle `daemon_follow`
- `↑`/`↓`: scroll logs (sets `daemon_follow = false`)
- Character input when `daemon_searching`: append to `daemon_search`
- `Esc` when searching: clear search and exit search mode

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p homerun`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/ui/daemon.rs crates/tui/src/ui/mod.rs crates/tui/src/ui/tabs.rs crates/tui/src/app.rs
git commit -m "feat(tui): add Daemon tab with log viewer and process info"
```

---

## Task 9: Tauri Backend — Daemon Log Commands

**Files:**

- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add `DaemonLogEntry` type and client method**

In `apps/desktop/src-tauri/src/client.rs`, add the type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}
```

Add client method:

```rust
pub async fn get_daemon_logs_recent(
    &self,
    level: Option<&str>,
    limit: Option<usize>,
    search: Option<&str>,
) -> Result<Vec<DaemonLogEntry>> {
    let mut params = Vec::new();
    if let Some(l) = level { params.push(format!("level={}", l)); }
    if let Some(n) = limit { params.push(format!("limit={}", n)); }
    if let Some(s) = search { params.push(format!("search={}", urlencoding::encode(s))); }
    let query = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
    let resp = self.get(&format!("/daemon/logs/recent{}", query)).await?;
    Ok(serde_json::from_slice(&resp)?)
}
```

Also add `DaemonMetrics` and `ChildProcessInfo` types, and extend the existing `MetricsResponse` with `pub daemon: Option<DaemonMetrics>`.

- [ ] **Step 2: Add Tauri command**

In `apps/desktop/src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn get_daemon_logs_recent(
    state: tauri::State<'_, AppState>,
    level: Option<String>,
    limit: Option<usize>,
    search: Option<String>,
) -> Result<Vec<DaemonLogEntry>, String> {
    let client = state.client.lock().await;
    client
        .get_daemon_logs_recent(level.as_deref(), limit, search.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register command in `lib.rs`**

In `apps/desktop/src-tauri/src/lib.rs`, add `commands::get_daemon_logs_recent` to the `invoke_handler` macro.

- [ ] **Step 4: Verify it compiles**

Run: `cd apps/desktop/src-tauri && cargo check`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/client.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(tauri): add daemon log backend commands"
```

---

## Task 10: Tauri Frontend — Types and API Layer

**Files:**

- Modify: `apps/desktop/src/api/types.ts`
- Modify: `apps/desktop/src/api/commands.ts`

- [ ] **Step 1: Add TypeScript types**

In `apps/desktop/src/api/types.ts`:

```typescript
export interface DaemonLogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export interface DaemonMetrics {
  pid: number;
  uptime_seconds: number;
  cpu_percent: number;
  memory_bytes: number;
  child_processes: ChildProcessInfo[];
}

export interface ChildProcessInfo {
  pid: number;
  runner_id: string;
  runner_name: string;
  cpu_percent: number;
  memory_bytes: number;
}
```

Extend the existing `MetricsResponse`:

```typescript
export interface MetricsResponse {
  system: SystemMetrics;
  runners: RunnerMetrics[];
  daemon?: DaemonMetrics;
}
```

- [ ] **Step 2: Add API command wrapper**

In `apps/desktop/src/api/commands.ts`:

```typescript
getDaemonLogsRecent: (level?: string, limit?: number, search?: string) =>
  invoke<DaemonLogEntry[]>("get_daemon_logs_recent", { level, limit, search }),
```

- [ ] **Step 3: Verify types compile**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/api/types.ts apps/desktop/src/api/commands.ts
git commit -m "feat(desktop): add daemon log types and API commands"
```

---

## Task 11: Tauri Frontend — useDaemonLogs Hook

**Files:**

- Create: `apps/desktop/src/hooks/useDaemonLogs.ts`

- [ ] **Step 1: Create the hook**

```typescript
// apps/desktop/src/hooks/useDaemonLogs.ts
import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/commands";
import { DaemonLogEntry } from "../api/types";

export function useDaemonLogs(pollInterval = 2000) {
  const [logs, setLogs] = useState<DaemonLogEntry[]>([]);
  const [level, setLevel] = useState<string>("INFO");
  const [search, setSearch] = useState<string>("");
  const [follow, setFollow] = useState(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const lastTimestampRef = useRef<string | null>(null);

  const fetchLogs = useCallback(async () => {
    try {
      const entries = await api.getDaemonLogsRecent(level, 2000, search || undefined);
      setLogs(entries);
      if (entries.length > 0) {
        lastTimestampRef.current = entries[entries.length - 1].timestamp;
      }
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [level, search]);

  useEffect(() => {
    fetchLogs();
    const interval = setInterval(fetchLogs, pollInterval);
    return () => clearInterval(interval);
  }, [fetchLogs, pollInterval]);

  return {
    logs,
    level,
    setLevel,
    search,
    setSearch,
    follow,
    setFollow,
    loading,
    error,
    refresh: fetchLogs,
  };
}
```

- [ ] **Step 2: Verify types compile**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/hooks/useDaemonLogs.ts
git commit -m "feat(desktop): add useDaemonLogs hook"
```

---

## Task 12: Tauri Frontend — Daemon Page Component

**Files:**

- Create: `apps/desktop/src/pages/Daemon.tsx`
- Modify: `apps/desktop/src/App.tsx:9-27` (add route)
- Modify: `apps/desktop/src/components/Layout.tsx` (add sidebar item)

- [ ] **Step 1: Create Daemon page component**

Create `apps/desktop/src/pages/Daemon.tsx`. This component should:

- Use `useDaemonLogs()` for log data and controls
- Use `useMetrics()` for daemon process info (reads `metrics.daemon`)
- Render 4 status cards (Status/PID, Uptime, CPU, Memory) from `metrics.daemon`
- Render a collapsible child processes table from `metrics.daemon.child_processes`
- Render a logs panel with:
  - Level filter pills (ERROR, WARN, INFO, DEBUG, TRACE) — click to set level
  - Search input — updates `search` state
  - Follow toggle — auto-scrolls to bottom
  - Log entries in monospace, color-coded by level (red=ERROR, yellow=WARN, green=INFO, blue=DEBUG, gray=TRACE)
  - Timestamps displayed as HH:MM:SS
  - Target shortened to last segment

Follow the existing component patterns in `RunnerDetail.tsx` for log display (scroll behavior, search filtering, color coding). Follow the `Dashboard.tsx` patterns for card layout.

Use the mockup from the design spec as the reference for visual layout.

- [ ] **Step 2: Add route**

In `apps/desktop/src/App.tsx`, add inside the `<Route element={<Layout />}>`:

```typescript
<Route path="/daemon" element={<Daemon />} />
```

Add the import at the top.

- [ ] **Step 3: Add sidebar navigation item**

In `apps/desktop/src/components/Layout.tsx`, add a "Daemon" link in the sidebar navigation, below the existing items. Use the same pattern as other nav items. Use a terminal/server icon.

- [ ] **Step 4: Verify it compiles and renders**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no errors

Run: `cd apps/desktop && npm run build`
Expected: builds successfully

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/pages/Daemon.tsx apps/desktop/src/App.tsx apps/desktop/src/components/Layout.tsx
git commit -m "feat(desktop): add Daemon page with logs and process info"
```

---

## Task 13: Final Integration Verification

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Run cargo fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues

- [ ] **Step 4: Run TypeScript type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 5: Run prettier**

Run: `cd apps/desktop && npx prettier --check src/`
Expected: no formatting issues (or run `--write` to fix)

- [ ] **Step 6: Build desktop app**

Run: `cd apps/desktop && npm run build`
Expected: builds successfully

- [ ] **Step 7: Commit any remaining fixes**

If any issues were found and fixed in steps 1-6, commit them:

```bash
git commit -m "fix: address lint and formatting issues"
```
