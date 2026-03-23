# Job History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Track and persist workflow run history per runner, show last completed job, and expose via API for both TUI and desktop app.

**Architecture:** Add `JobHistoryEntry` records created on each job completion, stored per-runner in `~/.homerun/history/{runner_id}.json` and cached in-memory on `RunnerManager`. A `last_completed_job` field on `RunnerInfo` keeps the most recent result visible in the UI without requiring a separate API call. A new `GET /runners/{id}/history` endpoint exposes full history.

**Tech Stack:** Rust (serde, chrono, tokio), Axum API, Tauri IPC, React/TypeScript frontend

**Closes:** #37
**Enables:** #27 (progress estimation from historical durations)

---

## File Structure

### Daemon (crates/daemon/src/)

- **Modify: `runner/types.rs`** — Add `JobHistoryEntry`, `CompletedJob`, new fields on `RunnerInfo`
- **Create: `runner/history.rs`** — History persistence: load/save/delete per-runner JSON files
- **Modify: `runner/mod.rs`** — Add `job_history` field to `RunnerManager`, wire up capture on job events, cleanup on delete
- **Modify: `config.rs`** — Add `history_dir()` method, create dir in `ensure_dirs()`
- **Create: `api/history.rs`** — `GET /runners/{id}/history` endpoint
- **Modify: `api/mod.rs`** — Add `pub mod history`
- **Modify: `server.rs`** — Register history route

### Tauri App (apps/desktop/)

- **Modify: `src-tauri/src/client.rs`** — Add `JobHistoryEntry`, `CompletedJob` types and `get_runner_history()` method
- **Modify: `src-tauri/src/commands.rs`** — Add `get_runner_history` Tauri command
- **Modify: `src-tauri/src/lib.rs`** — Register command
- **Modify: `src/api/types.ts`** — Add `JobHistoryEntry`, `CompletedJob` TS interfaces, update `RunnerInfo`
- **Modify: `src/api/commands.ts`** — Add `getRunnerHistory()` API call
- **Modify: `src/hooks/useRunners.ts`** — (No change needed, history is a separate fetch)
- **Create: `src/hooks/useJobHistory.ts`** — Hook to fetch and manage history state
- **Modify: `src/pages/RunnerDetail.tsx`** — Show last completed job card + history section

---

## Task 1: Data Model — Types

**Files:**

- Modify: `crates/daemon/src/runner/types.rs`

- [ ] **Step 1: Add `JobHistoryEntry` and `CompletedJob` structs and new fields to `RunnerInfo`**

Add to `types.rs`:

```rust
use crate::runner::steps::StepInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryEntry {
    pub job_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub succeeded: bool,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    pub steps: Vec<StepInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job_name: String,
    pub succeeded: bool,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub duration_secs: u64,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
}
```

Add new fields to `RunnerInfo`:

```rust
// Add after `job_context`:
#[serde(skip_serializing_if = "Option::is_none", default)]
pub job_started_at: Option<chrono::DateTime<chrono::Utc>>,
#[serde(skip_serializing_if = "Option::is_none", default)]
pub last_completed_job: Option<CompletedJob>,
```

- [ ] **Step 2: Update all `RunnerInfo` construction sites**

Every place that constructs `RunnerInfo` needs the two new fields. Search for all occurrences in `mod.rs` and tests where `RunnerInfo { ... }` is built and add:

```rust
job_started_at: None,
last_completed_job: None,
```

There are ~7 construction sites in `mod.rs` (create, load_from_disk, tests).

- [ ] **Step 3: Run tests to verify compilation**

Run: `cargo test -p homerund -- --nocapture 2>&1 | head -30`
Expected: All existing tests pass (new fields are `None` by default, serde `default` handles deserialization of old data).

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/runner/types.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add JobHistoryEntry and CompletedJob data models

Add types for tracking per-runner job history and last completed job.
New fields on RunnerInfo: job_started_at, last_completed_job.

Closes #37 (partial)"
```

---

## Task 2: Config — History Directory

**Files:**

- Modify: `crates/daemon/src/config.rs`

- [ ] **Step 1: Add `history_dir()` method and create in `ensure_dirs()`**

In `Config` impl:

```rust
pub fn history_dir(&self) -> PathBuf {
    self.base_dir.join("history")
}
```

In `ensure_dirs()`, add:

```rust
std::fs::create_dir_all(self.history_dir())?;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p homerund config -- --nocapture`
Expected: All config tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/config.rs
git commit -m "feat: add history directory to config

Add history_dir() method pointing to ~/.homerun/history/
and create it on startup."
```

---

## Task 3: History Persistence Module

**Files:**

- Create: `crates/daemon/src/runner/history.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add `pub mod history;`)

- [ ] **Step 1: Write tests for history persistence**

Create `crates/daemon/src/runner/history.rs` with tests first:

```rust
use crate::runner::types::JobHistoryEntry;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

const MAX_HISTORY_PER_RUNNER: usize = 100;

/// Load all job history from disk. Reads each `{runner_id}.json` file
/// in the history directory.
pub fn load_all(history_dir: &Path) -> Result<HashMap<String, Vec<JobHistoryEntry>>> {
    let mut map = HashMap::new();
    if !history_dir.exists() {
        return Ok(map);
    }
    for entry in std::fs::read_dir(history_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let runner_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if runner_id.is_empty() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let entries: Vec<JobHistoryEntry> = serde_json::from_str(&content)?;
            map.insert(runner_id, entries);
        }
    }
    Ok(map)
}

/// Save a single runner's history to disk.
pub fn save(history_dir: &Path, runner_id: &str, entries: &[JobHistoryEntry]) -> Result<()> {
    std::fs::create_dir_all(history_dir)?;
    let path = history_dir.join(format!("{runner_id}.json"));
    let json = serde_json::to_string_pretty(entries)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Delete a runner's history file.
pub fn delete(history_dir: &Path, runner_id: &str) -> Result<()> {
    let path = history_dir.join(format!("{runner_id}.json"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Append a history entry, keeping the list capped at MAX_HISTORY_PER_RUNNER.
/// Returns the updated list.
pub fn append(entries: &mut Vec<JobHistoryEntry>, entry: JobHistoryEntry) {
    entries.push(entry);
    if entries.len() > MAX_HISTORY_PER_RUNNER {
        entries.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_entry(job_name: &str, succeeded: bool) -> JobHistoryEntry {
        let now = Utc::now();
        JobHistoryEntry {
            job_name: job_name.to_string(),
            started_at: now - chrono::Duration::seconds(30),
            completed_at: now,
            succeeded,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: None,
            steps: vec![],
        }
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![make_entry("build", true), make_entry("test", false)];
        save(dir.path(), "runner-1", &entries).unwrap();

        let loaded = load_all(dir.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["runner-1"].len(), 2);
        assert_eq!(loaded["runner-1"][0].job_name, "build");
        assert!(loaded["runner-1"][0].succeeded);
        assert_eq!(loaded["runner-1"][1].job_name, "test");
        assert!(!loaded["runner-1"][1].succeeded);
    }

    #[test]
    fn test_load_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_all(dir.path()).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let loaded = load_all(std::path::Path::new("/nonexistent/path")).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_delete_history() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![make_entry("build", true)];
        save(dir.path(), "runner-1", &entries).unwrap();
        assert!(dir.path().join("runner-1.json").exists());

        delete(dir.path(), "runner-1").unwrap();
        assert!(!dir.path().join("runner-1.json").exists());
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        delete(dir.path(), "nonexistent").unwrap();
    }

    #[test]
    fn test_append_caps_at_max() {
        let mut entries = Vec::new();
        for i in 0..(MAX_HISTORY_PER_RUNNER + 10) {
            append(&mut entries, make_entry(&format!("job-{i}"), true));
        }
        assert_eq!(entries.len(), MAX_HISTORY_PER_RUNNER);
        // Oldest entries should have been removed
        assert_eq!(entries[0].job_name, "job-10");
    }

    #[test]
    fn test_save_load_roundtrip_with_steps() {
        use crate::runner::steps::{StepInfo, StepStatus};

        let dir = tempfile::tempdir().unwrap();
        let now = Utc::now();
        let entry = JobHistoryEntry {
            job_name: "test-with-steps".to_string(),
            started_at: now - chrono::Duration::seconds(60),
            completed_at: now,
            succeeded: true,
            branch: Some("feat/history".to_string()),
            pr_number: Some(42),
            run_url: Some("https://github.com/owner/repo/actions/runs/123".to_string()),
            steps: vec![
                StepInfo {
                    number: 1,
                    name: "Checkout".to_string(),
                    status: StepStatus::Succeeded,
                    started_at: Some(now - chrono::Duration::seconds(55)),
                    completed_at: Some(now - chrono::Duration::seconds(50)),
                },
                StepInfo {
                    number: 2,
                    name: "Build".to_string(),
                    status: StepStatus::Succeeded,
                    started_at: Some(now - chrono::Duration::seconds(50)),
                    completed_at: Some(now - chrono::Duration::seconds(5)),
                },
            ],
        };
        save(dir.path(), "runner-2", &[entry]).unwrap();

        let loaded = load_all(dir.path()).unwrap();
        assert_eq!(loaded["runner-2"][0].steps.len(), 2);
        assert_eq!(loaded["runner-2"][0].steps[0].name, "Checkout");
        assert_eq!(loaded["runner-2"][0].pr_number, Some(42));
    }
}
```

- [ ] **Step 2: Add module declaration to mod.rs**

Add `pub mod history;` to the top of `crates/daemon/src/runner/mod.rs`.

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p homerund history -- --nocapture`
Expected: All 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/runner/history.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add job history persistence module

Per-runner JSON files in ~/.homerun/history/{runner_id}.json.
Supports load_all, save, delete, and append with 100-entry cap."
```

---

## Task 4: Wire History Into RunnerManager

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs`

- [ ] **Step 1: Add `job_history` field to `RunnerManager`**

Add to the struct:

```rust
job_history: Arc<RwLock<HashMap<String, Vec<types::JobHistoryEntry>>>>,
```

Initialize in `new()`:

```rust
job_history: Arc::new(RwLock::new(HashMap::new())),
```

- [ ] **Step 2: Load history from disk in `load_from_disk()` and populate `last_completed_job`**

At the end of `load_from_disk()`, before `Ok(need_restart)`:

```rust
// Load job history from disk
match history::load_all(&self.config.history_dir()) {
    Ok(hist) => {
        // Populate last_completed_job for each runner from its most recent history entry
        for (runner_id, entries) in &hist {
            if let Some(last) = entries.last() {
                if let Some(r) = runners.get_mut(runner_id) {
                    let duration_secs = (last.completed_at - last.started_at)
                        .num_seconds()
                        .max(0) as u64;
                    r.last_completed_job = Some(types::CompletedJob {
                        job_name: last.job_name.clone(),
                        succeeded: last.succeeded,
                        completed_at: last.completed_at,
                        duration_secs,
                        branch: last.branch.clone(),
                        pr_number: last.pr_number,
                        run_url: last.run_url.clone(),
                    });
                }
            }
        }
        drop(runners); // release the runners write lock before acquiring job_history
        let mut job_history = self.job_history.write().await;
        *job_history = hist;
    }
    Err(e) => {
        tracing::warn!("Failed to load job history: {}", e);
    }
}
```

Note: `runners` is already a mutable write lock acquired earlier in `load_from_disk()`. The lock must be dropped before acquiring `job_history` to avoid potential deadlocks. If the lock has already been dropped at this point, re-acquire it with `let mut runners = self.runners.write().await;` before the loop.

- [ ] **Step 3: Add public methods for history access**

Add to `impl RunnerManager`:

```rust
/// Record a completed job in history.
pub async fn record_job_history(&self, runner_id: &str, entry: types::JobHistoryEntry) {
    let mut hist = self.job_history.write().await;
    let entries = hist.entry(runner_id.to_string()).or_default();
    history::append(entries, entry);
    if let Err(e) = history::save(&self.config.history_dir(), runner_id, entries) {
        tracing::warn!("Failed to save job history for {}: {}", runner_id, e);
    }
}

/// Get job history for a runner (newest first).
pub async fn get_job_history(&self, runner_id: &str) -> Vec<types::JobHistoryEntry> {
    let hist = self.job_history.read().await;
    let mut entries = hist.get(runner_id).cloned().unwrap_or_default();
    entries.reverse();
    entries
}

/// Delete job history for a runner.
pub async fn delete_job_history(&self, runner_id: &str) {
    self.job_history.write().await.remove(runner_id);
    if let Err(e) = history::delete(&self.config.history_dir(), runner_id) {
        tracing::warn!("Failed to delete job history for {}: {}", runner_id, e);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund -- --nocapture 2>&1 | head -30`
Expected: Compiles and all existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "feat: wire job history into RunnerManager

Add in-memory history cache, loaded from disk on startup.
Methods: record_job_history, get_job_history, delete_job_history."
```

---

## Task 5: Capture History on Job Completion

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs`

This is the core logic. There are two code paths for job events:

1. **`tail_diag_logs` — orphaned/reattached process log watcher** (~line 523): reads from Runner\_\*.log file tailing, has full `RunnerManager` reference
2. **stdout reader in `do_register_and_start`** (~line 1099): reads from process stdout directly, only has cloned Arcs (no RunnerManager)

Both need identical changes, but with different access patterns.

- [ ] **Step 1: Modify `JobEvent::Started` handler — set `job_started_at` and clear `last_completed_job`**

In BOTH code paths, when `JobEvent::Started(job_name)` is matched, add:

```rust
r.job_started_at = Some(chrono::Utc::now());
r.last_completed_job = None;
```

alongside the existing `r.current_job = Some(job_name.clone());`

- [ ] **Step 2: Modify `JobEvent::Completed` handler — create history entry and populate `last_completed_job`**

In BOTH code paths, when `JobEvent::Completed { succeeded }` is matched, BEFORE clearing `current_job`/`job_context`:

1. Capture the data needed for the history entry from the runner state
2. Get steps from `step_watcher` BEFORE calling `stop_watching`
3. Build the `JobHistoryEntry` and `CompletedJob`
4. Then proceed with existing logic (increment counters, set state to Online)
5. Set `last_completed_job` instead of clearing `current_job` to None right away — actually we do clear `current_job` (the frontend uses it to distinguish active vs last), but we populate `last_completed_job` first.

For the **`tail_diag_logs` path** (~line 550), replace the `JobEvent::Completed` arm:

```rust
Some(JobEvent::Completed { succeeded }) => {
    // Capture steps before stopping the watcher
    let steps = step_watcher
        .get_steps(runner_id)
        .await
        .map(|s| s.steps)
        .unwrap_or_default();
    step_watcher.stop_watching(runner_id).await;

    // Build history entry and update runner state
    {
        let mut map = manager.runners.write().await;
        if let Some(r) = map.get_mut(runner_id) {
            let now = chrono::Utc::now();
            let started_at = r.job_started_at.unwrap_or(now);
            let duration_secs =
                (now - started_at).num_seconds().max(0) as u64;

            // Build history entry
            let history_entry = types::JobHistoryEntry {
                job_name: r.current_job.clone().unwrap_or_default(),
                started_at,
                completed_at: now,
                succeeded,
                branch: r.job_context.as_ref().map(|c| c.branch.clone()),
                pr_number: r.job_context.as_ref().and_then(|c| c.pr_number),
                run_url: r.job_context.as_ref().map(|c| c.run_url.clone()),
                steps,
            };

            // Set last completed job
            r.last_completed_job = Some(types::CompletedJob {
                job_name: history_entry.job_name.clone(),
                succeeded,
                completed_at: now,
                duration_secs,
                branch: history_entry.branch.clone(),
                pr_number: history_entry.pr_number,
                run_url: history_entry.run_url.clone(),
            });

            // Update counters and state
            if succeeded {
                r.jobs_completed += 1;
            } else {
                r.jobs_failed += 1;
            }
            r.state = RunnerState::Online;
            r.current_job = None;
            r.job_context = None;
            r.job_started_at = None;

            // Record history (need to drop lock first)
            let rid = runner_id.to_string();
            drop(map);
            manager.record_job_history(&rid, history_entry).await;
        }
    }
    manager.emit_state_event(runner_id, "online");
    let _ = manager.save_to_disk().await;
}
```

For the **stdout reader in `do_register_and_start`** (~line 1117), apply the same pattern. The difference is that this path uses `rid` (String) instead of `runner_id` (&str), only has cloned Arcs (not the full RunnerManager), and is currently missing `emit_state_event`/`save_to_disk` calls (existing bug — fix as part of this task).

**Additional clones needed for this closure:** Clone `self.job_history`, `self.config.history_dir()`, and `self.event_tx` into the closure alongside the existing `runners`, `step_watcher`, etc.:

```rust
let job_history_arc = self.job_history.clone();
let history_dir = self.config.history_dir();
let event_tx = self.event_tx.clone();
let config_arc = self.config.clone();
```

Then the Completed handler:

```rust
Some(JobEvent::Completed { succeeded }) => {
    // Capture steps before stopping the watcher
    let steps_data = step_watcher
        .get_steps(&rid)
        .await
        .map(|s| s.steps)
        .unwrap_or_default();
    step_watcher.stop_watching(&rid).await;

    let mut map = runners.write().await;
    if let Some(r) = map.get_mut(&rid) {
        let now = chrono::Utc::now();
        let started_at = r.job_started_at.unwrap_or(now);
        let duration_secs =
            (now - started_at).num_seconds().max(0) as u64;

        let history_entry = types::JobHistoryEntry {
            job_name: r.current_job.clone().unwrap_or_default(),
            started_at,
            completed_at: now,
            succeeded,
            branch: r.job_context.as_ref().map(|c| c.branch.clone()),
            pr_number: r.job_context.as_ref().and_then(|c| c.pr_number),
            run_url: r.job_context.as_ref().map(|c| c.run_url.clone()),
            steps: steps_data,
        };

        r.last_completed_job = Some(types::CompletedJob {
            job_name: history_entry.job_name.clone(),
            succeeded,
            completed_at: now,
            duration_secs,
            branch: history_entry.branch.clone(),
            pr_number: history_entry.pr_number,
            run_url: history_entry.run_url.clone(),
        });

        if succeeded {
            r.jobs_completed += 1;
        } else {
            r.jobs_failed += 1;
        }
        r.state = RunnerState::Online;
        r.current_job = None;
        r.job_context = None;
        r.job_started_at = None;
    }
    drop(map);

    // Record history via cloned Arcs
    {
        let mut hist = job_history_arc.write().await;
        let entries = hist.entry(rid.clone()).or_default();
        history::append(entries, history_entry);
        if let Err(e) = history::save(&history_dir, &rid, entries) {
            tracing::warn!("Failed to save job history for {}: {}", rid, e);
        }
    }

    // Emit state event + save (fixes existing bug: this path was missing these)
    let _ = event_tx.send(RunnerEvent {
        runner_id: rid.clone(),
        event_type: "state_changed".to_string(),
        data: serde_json::json!({"state": "online"}),
        timestamp: chrono::Utc::now(),
    });
    // Save runner state to disk
    {
        let runners_snapshot = runners.read().await;
        let persisted: Vec<_> = runners_snapshot
            .values()
            .map(|r| PersistedRunner {
                config: r.config.clone(),
                was_running: r.state == RunnerState::Online || r.state == RunnerState::Busy,
            })
            .collect();
        drop(runners_snapshot);
        if let Ok(json) = serde_json::to_string_pretty(&persisted) {
            let path = config_arc.runners_json_path();
            let _ = std::fs::write(&path, json);
        }
    }
}
```

- [ ] **Step 3: Also set `job_started_at` in the `do_register_and_start` stdout reader's `JobEvent::Started` handler**

In the stdout reader path (~line 1100):

```rust
Some(JobEvent::Started(job_name)) => {
    let work_dir = {
        let mut map = runners.write().await;
        if let Some(r) = map.get_mut(&rid) {
            r.state = RunnerState::Busy;
            r.current_job = Some(job_name.clone());
            r.job_started_at = Some(chrono::Utc::now());
            r.last_completed_job = None;
            Some(r.config.work_dir.clone())
        } else {
            None
        }
    };
    // ... existing step_watcher code
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund -- --nocapture 2>&1 | head -40`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "feat: capture job history on completion

Record JobHistoryEntry when jobs complete in both process
monitor paths. Populate last_completed_job for UI display.
Track job_started_at for duration calculation."
```

---

## Task 6: History Cleanup on Deletion

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs`

- [ ] **Step 1: Add history cleanup to `delete()` and `full_delete()`**

In `delete()`, after removing from runners map, add:

```rust
self.delete_job_history(id).await;
```

This covers `full_delete()` too since it calls `self.delete(id).await?` at the end.

- [ ] **Step 2: Run tests**

Run: `cargo test -p homerund -- --nocapture 2>&1 | head -20`
Expected: Pass.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "feat: clean up job history when runner is deleted"
```

---

## Task 7: API Endpoint — GET /runners/{id}/history

**Files:**

- Create: `crates/daemon/src/api/history.rs`
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Create the history API handler**

Create `crates/daemon/src/api/history.rs`:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::types::JobHistoryEntry;
use crate::server::AppState;

pub async fn get_runner_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<JobHistoryEntry>>, (StatusCode, String)> {
    // Verify runner exists
    state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    let history = state.runner_manager.get_job_history(&id).await;
    Ok(Json(history))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_history_runner_not_found() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/nonexistent/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_history_empty_for_new_runner() {
        let state = AppState::new_test_authenticated();

        // Create a runner
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = runner["config"]["id"].as_str().unwrap();

        // Get history — should be empty
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/runners/{id}/history"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let history: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert!(history.is_empty());
    }
}
```

- [ ] **Step 2: Register the module and route**

In `crates/daemon/src/api/mod.rs`, add:

```rust
pub mod history;
```

In `crates/daemon/src/server.rs`, add the route in `create_router()`:

```rust
.route("/runners/{id}/history", get(api::history::get_runner_history))
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p homerund history -- --nocapture`
Expected: All history tests pass (unit + API).

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/api/history.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add GET /runners/{id}/history API endpoint

Returns job history entries newest-first for a given runner."
```

---

## Task 8: Tauri Client + Commands

**Files:**

- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add types and client method**

In `client.rs`, add the new types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job_name: String,
    pub succeeded: bool,
    pub completed_at: String,
    pub duration_secs: u64,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryEntry {
    pub job_name: String,
    pub started_at: String,
    pub completed_at: String,
    pub succeeded: bool,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    pub steps: Vec<StepInfo>,
}
```

Add fields to the existing `RunnerInfo` in `client.rs`:

```rust
#[serde(default)]
pub job_started_at: Option<String>,
#[serde(default)]
pub last_completed_job: Option<CompletedJob>,
```

Add client method:

```rust
pub async fn get_runner_history(&self, runner_id: &str) -> Result<Vec<JobHistoryEntry>, String> {
    let body = self.request("GET", &format!("/runners/{runner_id}/history"), None).await?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Add Tauri command**

In `commands.rs`, add:

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_history(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<Vec<crate::client::JobHistoryEntry>, String> {
    let client = state.client.lock().await;
    client.get_runner_history(&runner_id).await
}
```

Update the import in `commands.rs` to include `JobHistoryEntry` from `client`.

- [ ] **Step 3: Register command in lib.rs**

Add `commands::get_runner_history` to the `generate_handler![]` list.

- [ ] **Step 4: Build to verify**

Run: `cd apps/desktop && npm run build 2>&1 | tail -5`
(This builds the Tauri Rust backend too.)

Note: If the full Tauri build isn't set up locally, verify with: `cd apps/desktop/src-tauri && cargo check`

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/client.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add Tauri IPC for job history

Add get_runner_history command and types to pass history
data from daemon to desktop app."
```

---

## Task 9: Frontend Types + API Client

**Files:**

- Modify: `apps/desktop/src/api/types.ts`
- Modify: `apps/desktop/src/api/commands.ts`

- [ ] **Step 1: Add TypeScript types**

In `types.ts`, add:

```typescript
export interface CompletedJob {
  job_name: string;
  succeeded: boolean;
  completed_at: string;
  duration_secs: number;
  branch?: string | null;
  pr_number?: number | null;
  run_url?: string | null;
}

export interface JobHistoryEntry {
  job_name: string;
  started_at: string;
  completed_at: string;
  succeeded: boolean;
  branch?: string | null;
  pr_number?: number | null;
  run_url?: string | null;
  steps: StepInfo[];
}
```

Update `RunnerInfo` to add the new fields:

```typescript
export interface RunnerInfo {
  // ... existing fields ...
  job_started_at?: string | null;
  last_completed_job?: CompletedJob | null;
}
```

- [ ] **Step 2: Add API command**

In `commands.ts`, add:

```typescript
import type { ..., JobHistoryEntry } from "./types";

// In the api object:
getRunnerHistory: (runnerId: string) =>
  invoke<JobHistoryEntry[]>("get_runner_history", { runner_id: runnerId }),
```

- [ ] **Step 3: Type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/api/types.ts apps/desktop/src/api/commands.ts
git commit -m "feat: add job history types and API call to frontend"
```

---

## Task 10: Job History Hook

**Files:**

- Create: `apps/desktop/src/hooks/useJobHistory.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { useState, useEffect } from "react";
import { api } from "../api/commands";
import type { JobHistoryEntry } from "../api/types";

export function useJobHistory(runnerId: string | undefined) {
  const [history, setHistory] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!runnerId) return;

    let cancelled = false;

    async function fetchHistory() {
      setLoading(true);
      try {
        const entries = await api.getRunnerHistory(runnerId!);
        if (!cancelled) setHistory(entries);
      } catch {
        // ignore errors (runner may not exist yet)
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchHistory();
    // Refresh every 10 seconds (history doesn't change frequently)
    const timer = setInterval(fetchHistory, 10000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [runnerId]);

  return { history, loading };
}
```

- [ ] **Step 2: Type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/hooks/useJobHistory.ts
git commit -m "feat: add useJobHistory hook for fetching runner history"
```

---

## Task 11: Frontend UI — Last Completed Job + History Section

**Files:**

- Modify: `apps/desktop/src/pages/RunnerDetail.tsx`

- [ ] **Step 1: Import hook and update the Current Job card (Part A)**

Import the hook:

```typescript
import { useJobHistory } from "../hooks/useJobHistory";
```

In the component, add:

```typescript
const { history } = useJobHistory(id);
const [showAllHistory, setShowAllHistory] = useState(false);
```

Modify the Current Job card to show `last_completed_job` when not busy. Replace the "View Actions on GitHub" fallback (the `else` branch of `{current_job ? (...) : (...)}`) with:

```tsx
{current_job ? (
  // ... existing busy job display (unchanged) ...
) : runner.last_completed_job ? (
  <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
    <div className="flex items-center gap-8">
      <span
        style={{
          fontSize: 15,
          fontWeight: 500,
          color: "var(--text-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          flex: 1,
        }}
        title={runner.last_completed_job.job_name}
      >
        {runner.last_completed_job.job_name}
      </span>
      <span
        style={{
          fontSize: 11,
          fontWeight: 600,
          padding: "2px 8px",
          borderRadius: 4,
          background: runner.last_completed_job.succeeded
            ? "rgba(63, 185, 80, 0.15)"
            : "rgba(218, 54, 51, 0.15)",
          color: runner.last_completed_job.succeeded
            ? "var(--accent-green)"
            : "var(--accent-red)",
          flexShrink: 0,
        }}
      >
        {runner.last_completed_job.succeeded ? "Succeeded" : "Failed"}
      </span>
    </div>
    <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
      {formatUptime(runner.last_completed_job.duration_secs)}
      {runner.last_completed_job.branch && (
        <>
          {" · "}
          <span style={{ color: "var(--text-primary)" }}>
            {runner.last_completed_job.branch}
          </span>
        </>
      )}
      {runner.last_completed_job.pr_number != null && (
        <span style={{ color: "var(--accent-blue)", marginLeft: 4 }}>
          PR #{runner.last_completed_job.pr_number}
        </span>
      )}
    </div>
    {runner.last_completed_job.run_url && (
      <a
        href="#"
        onClick={(e) => {
          e.preventDefault();
          import("@tauri-apps/plugin-shell").then(({ open }) =>
            open(runner.last_completed_job!.run_url!),
          );
        }}
        style={{ fontSize: 12, color: "var(--accent-blue)" }}
      >
        View on GitHub →
      </a>
    )}
  </div>
) : (
  <a
    href="#"
    onClick={(e) => {
      e.preventDefault();
      import("@tauri-apps/plugin-shell").then(({ open }) => {
        open(`https://github.com/${config.repo_owner}/${config.repo_name}/actions`);
      });
    }}
    style={{ color: "var(--accent-blue)", fontSize: 13 }}
  >
    View Actions on GitHub →
  </a>
)}
```

Reuse the existing `formatUptime` function (already defined at the top of RunnerDetail.tsx) for duration formatting — it produces identical output. Use `formatUptime(runner.last_completed_job.duration_secs)` in the template below.

- [ ] **Step 2: Add History section below the JobProgress component**

After the `{/* Job Progress */}` section and before the `{confirmDelete && ...}` block, add:

```tsx
{
  /* Job History */
}
{
  history.length > 0 && (
    <div style={{ marginTop: 12 }}>
      <h3 className="runner-card-label" style={{ marginBottom: 8, fontSize: 11 }}>
        Job History
      </h3>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 1,
          borderRadius: 8,
          overflow: "hidden",
          border: "1px solid var(--border)",
        }}
      >
        {history.slice(0, showAllHistory ? history.length : 20).map((entry, i) => {
          const duration = Math.round(
            (new Date(entry.completed_at).getTime() - new Date(entry.started_at).getTime()) / 1000,
          );
          return (
            <div
              key={i}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                padding: "8px 12px",
                background: i % 2 === 0 ? "var(--bg-secondary)" : "var(--bg-primary)",
                fontSize: 13,
              }}
            >
              <span
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: entry.succeeded ? "var(--accent-green)" : "var(--accent-red)",
                  flexShrink: 0,
                }}
              />
              <span
                style={{
                  flex: 1,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  color: "var(--text-primary)",
                  fontWeight: 500,
                }}
                title={entry.job_name}
              >
                {entry.job_name}
              </span>
              {entry.branch && (
                <span
                  style={{
                    fontSize: 11,
                    color: "var(--text-secondary)",
                    flexShrink: 0,
                  }}
                >
                  {entry.branch}
                </span>
              )}
              <span
                className="font-mono"
                style={{
                  fontSize: 11,
                  color: "var(--text-secondary)",
                  flexShrink: 0,
                }}
              >
                {formatUptime(duration)}
              </span>
              <span
                style={{
                  fontSize: 11,
                  color: "var(--text-secondary)",
                  flexShrink: 0,
                }}
              >
                {new Date(entry.completed_at).toLocaleTimeString()}
              </span>
              {entry.run_url && (
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    import("@tauri-apps/plugin-shell").then(({ open }) => open(entry.run_url!));
                  }}
                  style={{
                    fontSize: 11,
                    color: "var(--accent-blue)",
                    flexShrink: 0,
                  }}
                >
                  View →
                </a>
              )}
            </div>
          );
        })}
      </div>
      {!showAllHistory && history.length > 20 && (
        <button
          className="btn"
          style={{ marginTop: 8, fontSize: 12 }}
          onClick={() => setShowAllHistory(true)}
        >
          Show all {history.length} entries
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Type check and format**

Run: `cd apps/desktop && npx tsc --noEmit && npx prettier --write src/pages/RunnerDetail.tsx src/hooks/useJobHistory.ts`
Expected: No type errors, files formatted.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/pages/RunnerDetail.tsx apps/desktop/src/hooks/useJobHistory.ts
git commit -m "feat: show last completed job and job history in runner detail

Part A: Display last job result with badge when runner is idle.
Part B: Show scrollable history table below job progress."
```

---

## Task 12: Verify Part C — Logs Persist Across Jobs

**Files:** None (verification only)

- [ ] **Step 1: Verify `recent_logs` is not cleared on job completion**

Search for any clearing of `recent_logs` on state changes:

Run: `grep -n "recent_logs.*clear\|recent_logs.*remove\|clear.*recent_logs" crates/daemon/src/runner/mod.rs`

Expected: No matches. The `recent_logs` ring buffer accumulates continuously with a 500-line cap and is never cleared on job boundaries. Part C is already working.

- [ ] **Step 2: Commit (no code changes, just verification note)**

No commit needed — this is a verification step.

---

## Task 13: Full Test Pass + Clippy

**Files:** None

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run frontend type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Format**

Run: `cargo fmt && cd apps/desktop && npx prettier --write src/`
Expected: No formatting changes (already formatted).
