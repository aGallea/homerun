# Job Progress Steps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show real-time workflow job step progress on the runner detail page, with step names/status/timing parsed locally from Worker logs and step log content fetched on-demand from GitHub API.

**Architecture:** The daemon watches `_diag/Worker_*.log` for step events, exposes two new REST endpoints (`/steps` and `/steps/{n}/logs`), and both Tauri + TUI clients consume them. Step progress is fully local; step logs are fetched from GitHub API and cached per job.

**Tech Stack:** Rust/Axum (daemon), React 19/TypeScript (Tauri), Ratatui (TUI), octocrab (GitHub API)

**Spec:** `docs/superpowers/specs/2026-03-23-job-progress-steps-design.md`

---

## File Structure

### New Files

| File                                          | Responsibility                                            |
| --------------------------------------------- | --------------------------------------------------------- |
| `crates/daemon/src/runner/steps.rs`           | WorkerLogWatcher, StepInfo, StepStatus, Worker log parser |
| `crates/daemon/src/runner/step_log_cache.rs`  | In-memory job log cache with TTL                          |
| `crates/daemon/src/api/steps.rs`              | REST endpoints: `get_steps`, `get_step_logs`              |
| `apps/desktop/src/components/JobProgress.tsx` | Job steps UI component for Tauri                          |
| `apps/desktop/src/hooks/useJobSteps.ts`       | Hook for polling step progress + fetching step logs       |

### Modified Files

| File                                         | Changes                                                                |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `crates/daemon/src/runner/mod.rs`            | Integrate WorkerLogWatcher into stdout reader task                     |
| `crates/daemon/src/runner/types.rs`          | Add `job_id` to JobContext                                             |
| `crates/daemon/src/runner/step_log_cache.rs` | Job log cache keyed by job_id, 5s refresh for running, 5min TTL        |
| `crates/daemon/src/github/mod.rs`            | Add `id` to RunJob, capture job_id; add `get_job_logs` method          |
| `crates/daemon/src/api/mod.rs`               | Add `pub mod steps;`                                                   |
| `crates/daemon/src/server.rs`                | Add step routes                                                        |
| `apps/desktop/src-tauri/src/client.rs`       | Add StepInfo/StepsResponse types + `get_steps`/`get_step_logs` methods |
| `apps/desktop/src-tauri/src/commands.rs`     | Add `get_runner_steps`/`get_step_logs` Tauri commands                  |
| `apps/desktop/src-tauri/src/lib.rs`          | Register new commands                                                  |
| `apps/desktop/src/api/types.ts`              | Add StepInfo, StepStatus, StepsResponse, StepLogsResponse types        |
| `apps/desktop/src/api/commands.ts`           | Add `getRunnerSteps`/`getStepLogs` API methods                         |
| `apps/desktop/src/pages/RunnerDetail.tsx`    | Insert JobProgress component, rename Logs header                       |
| `apps/desktop/src/index.css`                 | Add step-spinner animation CSS                                         |
| `crates/tui/src/client.rs`                   | Add StepInfo types + `get_steps`/`get_step_logs` methods               |
| `crates/tui/src/app.rs`                      | Add `selected_runner_steps` field to App state                         |
| `crates/tui/src/ui/runners.rs`               | Add step progress display to runner detail                             |

---

## Task 1: Step Types and Worker Log Parser

**Files:**

- Create: `crates/daemon/src/runner/steps.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add `pub mod steps;`)

- [ ] **Step 1: Write tests for Worker log line parsing**

In `crates/daemon/src/runner/steps.rs`, define the types and write tests first:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: StepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}

/// Events parsed from Worker log lines.
#[derive(Debug, PartialEq)]
pub enum StepEvent {
    /// A new step was discovered: DisplayName extracted.
    Discovered { name: String, timestamp: DateTime<Utc> },
    /// The most recently discovered step has started.
    Started { timestamp: DateTime<Utc> },
    /// A step completed with the given result.
    Completed { result: StepStatus, timestamp: DateTime<Utc> },
}

/// Parse a single line from the Worker log into a StepEvent, if it matches.
pub fn parse_step_event(line: &str) -> Option<StepEvent> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_step_discovered() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Run actions/checkout@v6'";
        let event = parse_step_event(line);
        assert!(matches!(
            event,
            Some(StepEvent::Discovered { ref name, .. }) if name == "Run actions/checkout@v6"
        ));
    }

    #[test]
    fn test_parse_step_started() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.";
        let event = parse_step_event(line);
        assert!(matches!(event, Some(StepEvent::Started { .. })));
    }

    #[test]
    fn test_parse_step_succeeded() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.";
        let event = parse_step_event(line);
        assert!(matches!(
            event,
            Some(StepEvent::Completed { result: StepStatus::Succeeded, .. })
        ));
    }

    #[test]
    fn test_parse_step_failed() {
        let line = "[2026-03-23 07:55:22Z INFO StepsRunner] Updating job result with current step result 'Failed'.";
        let event = parse_step_event(line);
        assert!(matches!(
            event,
            Some(StepEvent::Completed { result: StepStatus::Failed, .. })
        ));
    }

    #[test]
    fn test_parse_step_skipped() {
        let line = "[2026-03-23 07:55:22Z INFO StepsRunner] Updating job result with current step result 'Skipped'.";
        let event = parse_step_event(line);
        assert!(matches!(
            event,
            Some(StepEvent::Completed { result: StepStatus::Skipped, .. })
        ));
    }

    #[test]
    fn test_parse_unrelated_line_returns_none() {
        let line = "[2026-03-23 07:54:50Z INFO Worker] Version: 2.333.0";
        assert!(parse_step_event(line).is_none());
    }

    #[test]
    fn test_parse_timestamp_extracted_correctly() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Check formatting'";
        if let Some(StepEvent::Discovered { timestamp, .. }) = parse_step_event(line) {
            assert_eq!(timestamp.format("%H:%M:%S").to_string(), "07:54:53");
        } else {
            panic!("expected Discovered event");
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- steps::tests --nocapture`
Expected: FAIL — `parse_step_event` has `todo!()`

- [ ] **Step 3: Implement `parse_step_event`**

Replace the `todo!()` in `parse_step_event`:

```rust
pub fn parse_step_event(line: &str) -> Option<StepEvent> {
    // All relevant lines contain "StepsRunner]"
    if !line.contains("StepsRunner]") {
        return None;
    }

    let timestamp = parse_worker_timestamp(line)?;

    // Pattern: Processing step: DisplayName='...'
    if let Some(start) = line.find("DisplayName='") {
        let rest = &line[start + "DisplayName='".len()..];
        let end = rest.find('\'')?;
        let name = rest[..end].to_string();
        return Some(StepEvent::Discovered { name, timestamp });
    }

    // Pattern: Starting the step.
    if line.contains("Starting the step.") {
        return Some(StepEvent::Started { timestamp });
    }

    // Pattern: ...current step result 'Succeeded|Failed|Skipped'
    if line.contains("step result '") {
        let result = if line.contains("'Succeeded'") {
            StepStatus::Succeeded
        } else if line.contains("'Failed'") {
            StepStatus::Failed
        } else if line.contains("'Skipped'") {
            StepStatus::Skipped
        } else {
            StepStatus::Failed // unknown = failed
        };
        return Some(StepEvent::Completed { result, timestamp });
    }

    None
}

/// Parse timestamp from Worker log line format: [YYYY-MM-DD HH:MM:SSZ ...]
fn parse_worker_timestamp(line: &str) -> Option<DateTime<Utc>> {
    // Format: [2026-03-23 07:54:53Z INFO ...]
    let start = line.find('[')?;
    let end = line.find('Z')?;
    let ts_str = &line[start + 1..=end];
    ts_str
        .parse::<DateTime<Utc>>()
        .ok()
        .or_else(|| {
            // Try with explicit format
            chrono::NaiveDateTime::parse_from_str(
                &ts_str[..ts_str.len() - 1],
                "%Y-%m-%d %H:%M:%S",
            )
            .ok()
            .map(|ndt| ndt.and_utc())
        })
}
```

- [ ] **Step 4: Add `pub mod steps;` to runner/mod.rs**

In `crates/daemon/src/runner/mod.rs`, add after the existing module declarations:

```rust
pub mod steps;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p homerund -- steps::tests --nocapture`
Expected: All 7 tests PASS

- [ ] **Step 6: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/runner/steps.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add Worker log step event parser with types and tests"
```

---

## Task 2: WorkerLogWatcher

**Files:**

- Modify: `crates/daemon/src/runner/steps.rs`

- [ ] **Step 1: Write tests for WorkerLogWatcher state tracking**

Add to `crates/daemon/src/runner/steps.rs`:

```rust
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use std::sync::Arc;
use std::collections::HashMap;

/// Tracks step progress for runners by watching their Worker log files.
pub struct WorkerLogWatcher {
    /// Per-runner step state: runner_id -> (steps, job_name)
    step_state: Arc<RwLock<HashMap<String, RunnerStepState>>>,
}

struct RunnerStepState {
    job_name: String,
    steps: Vec<StepInfo>,
    file_offset: u64,
    log_path: Option<PathBuf>,
}

impl WorkerLogWatcher {
    pub fn new() -> Self {
        Self {
            step_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start tracking steps for a runner that just started a job.
    pub async fn start_watching(&self, runner_id: &str, job_name: &str, work_dir: &Path) {
        todo!()
    }

    /// Stop tracking steps for a runner (job completed).
    pub async fn stop_watching(&self, runner_id: &str) {
        todo!()
    }

    /// Poll the Worker log for new step events. Call this periodically (~500ms).
    pub async fn poll(&self, runner_id: &str) {
        todo!()
    }

    /// Get current step state for a runner.
    pub async fn get_steps(&self, runner_id: &str) -> Option<StepsResponse> {
        todo!()
    }
}

// Add to existing tests module:
#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[tokio::test]
    async fn test_watcher_processes_step_events() {
        use std::io::Write;

        let watcher = WorkerLogWatcher::new();
        let dir = tempfile::tempdir().unwrap();
        let diag_dir = dir.path().join("_diag");
        std::fs::create_dir_all(&diag_dir).unwrap();

        // Write a Worker log with step events
        let log_path = diag_dir.join("Worker_20260323-075450-utc.log");
        let mut f = std::fs::File::create(&log_path).unwrap();
        writeln!(f, "[2026-03-23 07:54:50Z INFO Worker] Version: 2.333.0").unwrap();
        writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Run actions/checkout@v6'").unwrap();
        writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.").unwrap();
        writeln!(f, "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.").unwrap();
        writeln!(f, "[2026-03-23 07:54:55Z INFO StepsRunner] Processing step: DisplayName='Check formatting'").unwrap();
        writeln!(f, "[2026-03-23 07:54:55Z INFO StepsRunner] Starting the step.").unwrap();

        watcher.start_watching("runner-1", "Build Job", dir.path()).await;
        watcher.poll("runner-1").await;

        let response = watcher.get_steps("runner-1").await.unwrap();
        assert_eq!(response.job_name, "Build Job");
        assert_eq!(response.steps_discovered, 2);
        assert_eq!(response.steps[0].name, "Run actions/checkout@v6");
        assert_eq!(response.steps[0].status, StepStatus::Succeeded);
        assert_eq!(response.steps[1].name, "Check formatting");
        assert_eq!(response.steps[1].status, StepStatus::Running);
    }

    #[tokio::test]
    async fn test_watcher_incremental_reads() {
        use std::io::Write;

        let watcher = WorkerLogWatcher::new();
        let dir = tempfile::tempdir().unwrap();
        let diag_dir = dir.path().join("_diag");
        std::fs::create_dir_all(&diag_dir).unwrap();

        let log_path = diag_dir.join("Worker_20260323-075450-utc.log");
        let mut f = std::fs::File::create(&log_path).unwrap();
        writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Step 1'").unwrap();
        writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.").unwrap();

        watcher.start_watching("runner-1", "Job", dir.path()).await;
        watcher.poll("runner-1").await;

        let response = watcher.get_steps("runner-1").await.unwrap();
        assert_eq!(response.steps_discovered, 1);
        assert_eq!(response.steps[0].status, StepStatus::Running);

        // Append more lines (simulates file growing)
        let mut f = std::fs::OpenOptions::new().append(true).open(&log_path).unwrap();
        writeln!(f, "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.").unwrap();

        watcher.poll("runner-1").await;

        let response = watcher.get_steps("runner-1").await.unwrap();
        assert_eq!(response.steps[0].status, StepStatus::Succeeded);
    }

    #[tokio::test]
    async fn test_watcher_stop_clears_state() {
        let watcher = WorkerLogWatcher::new();
        let dir = tempfile::tempdir().unwrap();
        let diag_dir = dir.path().join("_diag");
        std::fs::create_dir_all(&diag_dir).unwrap();
        let log_path = diag_dir.join("Worker_20260323-075450-utc.log");
        std::fs::write(&log_path, "").unwrap();

        watcher.start_watching("runner-1", "Job", dir.path()).await;
        assert!(watcher.get_steps("runner-1").await.is_some());

        watcher.stop_watching("runner-1").await;
        assert!(watcher.get_steps("runner-1").await.is_none());
    }

    #[tokio::test]
    async fn test_watcher_no_diag_dir_returns_empty_steps() {
        let watcher = WorkerLogWatcher::new();
        let dir = tempfile::tempdir().unwrap();
        // No _diag directory

        watcher.start_watching("runner-1", "Job", dir.path()).await;
        watcher.poll("runner-1").await;

        let response = watcher.get_steps("runner-1").await.unwrap();
        assert_eq!(response.steps_discovered, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- steps::tests --nocapture`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement WorkerLogWatcher**

Replace the `todo!()` implementations:

```rust
impl WorkerLogWatcher {
    pub fn new() -> Self {
        Self {
            step_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_watching(&self, runner_id: &str, job_name: &str, work_dir: &Path) {
        let log_path = find_latest_worker_log(work_dir);
        let mut state = self.step_state.write().await;
        state.insert(
            runner_id.to_string(),
            RunnerStepState {
                job_name: job_name.to_string(),
                steps: Vec::new(),
                file_offset: 0,
                log_path,
            },
        );
    }

    pub async fn stop_watching(&self, runner_id: &str) {
        let mut state = self.step_state.write().await;
        state.remove(runner_id);
    }

    pub async fn poll(&self, runner_id: &str) {
        let mut state = self.step_state.write().await;
        let Some(runner_state) = state.get_mut(runner_id) else {
            return;
        };

        // If we don't have a log path yet, try to find one
        if runner_state.log_path.is_none() {
            // Reconstruct work_dir from existing info isn't possible here,
            // so we set it during start_watching. If None, skip.
            return;
        }

        let log_path = runner_state.log_path.as_ref().unwrap();
        if !log_path.exists() {
            return;
        }

        // Read new bytes from the file
        let Ok(content) = std::fs::read_to_string(log_path) else {
            return;
        };

        let new_content = if (runner_state.file_offset as usize) < content.len() {
            &content[runner_state.file_offset as usize..]
        } else {
            return;
        };

        for line in new_content.lines() {
            if let Some(event) = parse_step_event(line) {
                apply_step_event(&mut runner_state.steps, event);
            }
        }

        runner_state.file_offset = content.len() as u64;
    }

    pub async fn get_steps(&self, runner_id: &str) -> Option<StepsResponse> {
        let state = self.step_state.read().await;
        let runner_state = state.get(runner_id)?;
        Some(StepsResponse {
            job_name: runner_state.job_name.clone(),
            steps: runner_state.steps.clone(),
            steps_discovered: runner_state.steps.len(),
        })
    }
}

fn apply_step_event(steps: &mut Vec<StepInfo>, event: StepEvent) {
    match event {
        StepEvent::Discovered { name, timestamp: _ } => {
            let number = (steps.len() + 1) as u16;
            steps.push(StepInfo {
                number,
                name,
                status: StepStatus::Pending,
                started_at: None,
                completed_at: None,
            });
        }
        StepEvent::Started { timestamp } => {
            // Apply to the last Pending step
            if let Some(step) = steps.iter_mut().rev().find(|s| s.status == StepStatus::Pending) {
                step.status = StepStatus::Running;
                step.started_at = Some(timestamp);
            }
        }
        StepEvent::Completed { result, timestamp } => {
            // Apply to the last Running step
            if let Some(step) = steps.iter_mut().rev().find(|s| s.status == StepStatus::Running) {
                step.status = result;
                step.completed_at = Some(timestamp);
            }
        }
    }
}

/// Find the latest Worker_*.log file in {work_dir}/_diag/
fn find_latest_worker_log(work_dir: &Path) -> Option<PathBuf> {
    let diag_dir = work_dir.join("_diag");
    if !diag_dir.exists() {
        return None;
    }
    let mut logs: Vec<_> = std::fs::read_dir(&diag_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("Worker_")
                && e.file_name().to_string_lossy().ends_with(".log")
        })
        .collect();
    logs.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
    logs.first().map(|e| e.path())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p homerund -- steps::tests --nocapture`
Expected: All tests PASS

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/steps.rs
git commit -m "feat: add WorkerLogWatcher for tracking step progress from Worker logs"
```

---

## Task 3: Integrate WorkerLogWatcher into RunnerManager

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs`
- Modify: `crates/daemon/src/server.rs` (AppState)

- [ ] **Step 1: Add WorkerLogWatcher to RunnerManager**

In `crates/daemon/src/runner/mod.rs`, add a `step_watcher` field to `RunnerManager`:

```rust
// Near the top, add import:
use steps::WorkerLogWatcher;

// In RunnerManager struct, add field:
pub step_watcher: WorkerLogWatcher,

// In RunnerManager::new(), initialize:
step_watcher: WorkerLogWatcher::new(),
```

- [ ] **Step 2: Hook into job start/stop events**

In the stdout reader task (around line 720 where `JobEvent::Started` is handled), add watcher activation:

```rust
Some(JobEvent::Started(job_name)) => {
    // existing code to set state = Busy, current_job = Some(job_name)
    // ...

    // Start watching Worker log for step events
    let work_dir = {
        let map = runners.read().await;
        map.get(&rid).map(|r| r.config.work_dir.clone())
    };
    if let Some(work_dir) = work_dir {
        step_watcher.start_watching(&rid, &job_name, &work_dir).await;
    }
}
Some(JobEvent::Completed { succeeded }) => {
    // existing code to update state, clear current_job
    // ...

    // Stop watching
    step_watcher.stop_watching(&rid).await;
}
```

Note: `step_watcher` needs to be cloned/shared into the spawned task. Clone `Arc<WorkerLogWatcher>` or make it accessible via `RunnerManager`.

- [ ] **Step 3: Add periodic polling task with cancellation**

After the stdout/stderr reader tasks are spawned, add a polling loop. Store the `JoinHandle` so it can be aborted on stop:

```rust
// Spawn step watcher poll task
let step_watcher_poll = self.step_watcher.clone();
let rid_poll = id.to_string();
let poll_handle = tokio::spawn(async move {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // poll returns early if runner_id was removed from state
        if !step_watcher_poll.poll(&rid_poll).await {
            break;
        }
    }
});
// Store poll_handle so it can be aborted when runner stops
// (e.g., in a HashMap<String, JoinHandle> on RunnerManager)
```

Update `WorkerLogWatcher::poll` to return `bool` (true = keep going, false = stop):

```rust
pub async fn poll(&self, runner_id: &str) -> bool {
    let mut state = self.step_state.write().await;
    let Some(runner_state) = state.get_mut(runner_id) else {
        return false; // No longer watching, signal task to stop
    };
    // ... existing poll logic ...
    true
}
```

- [ ] **Step 4: Add public accessor method to RunnerManager**

```rust
impl RunnerManager {
    pub async fn get_steps(&self, runner_id: &str) -> Option<steps::StepsResponse> {
        self.step_watcher.get_steps(runner_id).await
    }
}
```

- [ ] **Step 5: Write integration test for RunnerManager.get_steps**

Add test in `crates/daemon/src/runner/mod.rs` tests section:

```rust
#[tokio::test]
async fn test_get_steps_returns_data_after_watcher_started() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.path().join(".homerun"));
    config.ensure_dirs().unwrap();
    let manager = RunnerManager::new(config);

    // Create a fake runner work dir with a Worker log
    let work_dir = dir.path().join("runner-work");
    let diag_dir = work_dir.join("_diag");
    std::fs::create_dir_all(&diag_dir).unwrap();
    let log_path = diag_dir.join("Worker_20260323-075450-utc.log");
    let mut f = std::fs::File::create(&log_path).unwrap();
    writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Checkout'").unwrap();
    writeln!(f, "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.").unwrap();
    writeln!(f, "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.").unwrap();

    // Before watching, get_steps returns None
    assert!(manager.get_steps("runner-1").await.is_none());

    // Start watching and poll
    manager.step_watcher.start_watching("runner-1", "Test Job", &work_dir).await;
    manager.step_watcher.poll("runner-1").await;

    // Now get_steps returns data
    let steps = manager.get_steps("runner-1").await.unwrap();
    assert_eq!(steps.steps.len(), 1);
    assert_eq!(steps.steps[0].name, "Checkout");
    assert_eq!(steps.steps[0].status, steps::StepStatus::Succeeded);

    // After stop_watching, get_steps returns None
    manager.step_watcher.stop_watching("runner-1").await;
    assert!(manager.get_steps("runner-1").await.is_none());
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p homerund --nocapture`
Expected: All existing + new tests PASS

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 8: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "feat: integrate WorkerLogWatcher into RunnerManager with polling"
```

---

## Task 4: Add `job_id` to JobContext

**Files:**

- Modify: `crates/daemon/src/runner/types.rs`
- Modify: `crates/daemon/src/github/mod.rs`
- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src/api/types.ts`
- Modify: `crates/tui/src/client.rs`

- [ ] **Step 1: Add `job_id` field to daemon's JobContext**

In `crates/daemon/src/runner/types.rs`, line 24-30:

```rust
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<u64>,
}
```

- [ ] **Step 2: Add `id` to RunJob and capture it**

In `crates/daemon/src/github/mod.rs`, update `RunJob` struct (line 166-170):

```rust
#[derive(Deserialize)]
struct RunJob {
    id: u64,
    name: String,
    runner_name: Option<String>,
}
```

Update the match logic (around line 191-216) to capture the matched job's ID:

```rust
let matched_job = jobs.jobs.iter().find(|j| {
    if let Some(rn) = j.runner_name.as_deref() {
        rn == runner_name
    } else {
        j.name == job_name
    }
});

if let Some(job) = matched_job {
    let (pr_number, pr_url) = if let Some(pr) = run.pull_requests.first() {
        let html_url = pr
            .url
            .replace("api.github.com/repos", "github.com")
            .replace("/pulls/", "/pull/");
        (Some(pr.number), Some(html_url))
    } else {
        (None, None)
    };

    return Ok(Some(crate::runner::types::JobContext {
        branch: run.head_branch,
        pr_number,
        pr_url,
        run_url: run.html_url,
        job_id: Some(job.id),
    }));
}
```

- [ ] **Step 3: Update Tauri client types**

In `apps/desktop/src-tauri/src/client.rs`, update `JobContext` (line 40-45):

```rust
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
    #[serde(default)]
    pub job_id: Option<u64>,
}
```

- [ ] **Step 4: Update TUI client types**

In `crates/tui/src/client.rs`, update `JobContext` (line 43-49):

```rust
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
    #[serde(default)]
    pub job_id: Option<u64>,
}
```

- [ ] **Step 5: Update TypeScript types**

In `apps/desktop/src/api/types.ts`, update `JobContext` (line 33-38):

```typescript
export interface JobContext {
  branch: string;
  pr_number: number | null;
  pr_url: string | null;
  run_url: string;
  job_id?: number | null;
}
```

- [ ] **Step 6: Run all tests across the workspace**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 8: Commit**

```bash
git add crates/daemon/src/runner/types.rs crates/daemon/src/github/mod.rs apps/desktop/src-tauri/src/client.rs apps/desktop/src/api/types.ts crates/tui/src/client.rs
git commit -m "feat: add job_id to JobContext for step log fetching"
```

---

## Task 5: GitHub API — Fetch Job Logs

**Files:**

- Modify: `crates/daemon/src/github/mod.rs`

- [ ] **Step 1: Write test for `get_job_logs`**

Add a test in `crates/daemon/src/github/mod.rs` tests section:

```rust
#[test]
fn test_parse_job_log_into_steps() {
    let raw_log = "2026-03-23T07:54:51.0000000Z ##[group]Run actions/checkout@v6\n\
        2026-03-23T07:54:52.0000000Z Syncing repository: owner/repo\n\
        2026-03-23T07:54:53.0000000Z ##[endgroup]\n\
        2026-03-23T07:54:53.0000000Z ##[group]Check formatting\n\
        2026-03-23T07:54:54.0000000Z + cargo fmt --check\n\
        2026-03-23T07:54:55.0000000Z ##[endgroup]\n";

    let sections = parse_job_log_sections(raw_log);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "Run actions/checkout@v6");
    assert!(sections[0].1.contains("Syncing repository"));
    assert_eq!(sections[1].0, "Check formatting");
    assert!(sections[1].1.contains("cargo fmt"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund -- test_parse_job_log_into_steps --nocapture`
Expected: FAIL — function doesn't exist

- [ ] **Step 3: Implement `parse_job_log_sections` and `get_job_logs`**

Add to `crates/daemon/src/github/mod.rs`:

```rust
/// Parse raw job log text into sections by step name.
/// Returns Vec<(step_name, log_content)>.
pub fn parse_job_log_sections(raw_log: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in raw_log.lines() {
        // Strip the timestamp prefix (format: 2026-03-23T07:54:51.0000000Z)
        let content = if line.len() > 29 && line.as_bytes()[28] == b'Z' {
            line[29..].trim_start()
        } else {
            line
        };

        if let Some(name) = content.strip_prefix("##[group]") {
            // Save previous section
            if let Some(prev_name) = current_name.take() {
                sections.push((prev_name, current_lines.join("\n")));
                current_lines.clear();
            }
            current_name = Some(name.to_string());
        } else if content == "##[endgroup]" {
            if let Some(name) = current_name.take() {
                sections.push((name, current_lines.join("\n")));
                current_lines.clear();
            }
        } else if current_name.is_some() {
            // Strip timestamp from the line for cleaner display
            let display_line = if line.len() > 29 && line.as_bytes()[28] == b'Z' {
                line[29..].trim_start().to_string()
            } else {
                line.to_string()
            };
            current_lines.push(display_line);
        }
    }

    // Handle unterminated section (step still running)
    if let Some(name) = current_name {
        sections.push((name, current_lines.join("\n")));
    }

    sections
}

impl GitHubClient {
    /// Fetch raw log content for a specific job.
    /// The endpoint returns a 302 redirect to blob storage serving plain text.
    /// We use reqwest directly since octocrab expects JSON responses.
    pub async fn get_job_logs(
        &self,
        owner: &str,
        repo: &str,
        job_id: u64,
    ) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/actions/jobs/{job_id}/logs"
        );
        // Access the token from the octocrab instance or store it on GitHubClient.
        // The existing GitHubClient::new(Some(token)) stores the token — add a
        // `token: Option<String>` field if not already present.
        let token = self.token.as_deref()
            .ok_or_else(|| anyhow::anyhow!("No auth token for job logs"))?;

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "homerun")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch job logs: {e}"))?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API returned {}: job logs", response.status());
        }

        response.text().await
            .map_err(|e| anyhow::anyhow!("Failed to read job log body: {e}"))
    }
}
```

Note: `reqwest` must be added to `crates/daemon/Cargo.toml` dev-dependencies (or regular deps). Check if it's already pulled in transitively via octocrab. Also store the raw token string on `GitHubClient` alongside the `octocrab` instance.

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund -- test_parse_job_log_sections --nocapture`
Expected: PASS

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/github/mod.rs
git commit -m "feat: add GitHub API job log fetching and section parser"
```

---

## Task 6: Job Log Cache

**Files:**

- Create: `crates/daemon/src/runner/step_log_cache.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add `pub mod step_log_cache;`)

- [ ] **Step 1: Write tests for the cache**

Create `crates/daemon/src/runner/step_log_cache.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cached job log entry.
struct CacheEntry {
    raw_log: String,
    fetched_at: Instant,
    job_completed: bool,
}

/// In-memory cache for job logs fetched from GitHub API.
/// Keyed by job_id. One fetch covers all steps of a job.
pub struct StepLogCache {
    entries: Arc<RwLock<HashMap<u64, CacheEntry>>>,
    refresh_interval: Duration,
    ttl_after_completion: Duration,
}

impl StepLogCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            refresh_interval: Duration::from_secs(5),
            ttl_after_completion: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Get cached log or fetch from GitHub API.
    pub async fn get_or_fetch(
        &self,
        job_id: u64,
        gh: &crate::github::GitHubClient,
        owner: &str,
        repo: &str,
    ) -> anyhow::Result<String> {
        // Check cache
        {
            let cache = self.entries.read().await;
            if let Some(entry) = cache.get(&job_id) {
                let age = entry.fetched_at.elapsed();
                // For completed jobs, serve from cache if within TTL
                if entry.job_completed && age < self.ttl_after_completion {
                    return Ok(entry.raw_log.clone());
                }
                // For running jobs, serve if younger than refresh_interval
                if !entry.job_completed && age < self.refresh_interval {
                    return Ok(entry.raw_log.clone());
                }
            }
        }

        // Cache miss or stale — fetch
        let raw_log = gh.get_job_logs(owner, repo, job_id).await?;

        let mut cache = self.entries.write().await;
        cache.insert(
            job_id,
            CacheEntry {
                raw_log: raw_log.clone(),
                fetched_at: Instant::now(),
                job_completed: false,
            },
        );

        Ok(raw_log)
    }

    /// Mark a job as completed (starts the 5-minute TTL countdown).
    pub async fn mark_completed(&self, job_id: u64) {
        let mut cache = self.entries.write().await;
        if let Some(entry) = cache.get_mut(&job_id) {
            entry.job_completed = true;
        }
    }

    /// Evict expired entries.
    pub async fn evict_expired(&self) {
        let mut cache = self.entries.write().await;
        cache.retain(|_, entry| {
            if entry.job_completed {
                entry.fetched_at.elapsed() < self.ttl_after_completion
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_returns_cached_value_within_interval() {
        let cache = StepLogCache::new();

        // Manually insert a cache entry
        {
            let mut entries = cache.entries.write().await;
            entries.insert(
                123,
                CacheEntry {
                    raw_log: "cached log content".to_string(),
                    fetched_at: Instant::now(),
                    job_completed: false,
                },
            );
        }

        // Reading from cache should return the cached value
        // (We can't test get_or_fetch without a real GitHubClient,
        //  but we can test the cache check logic directly)
        let entries = cache.entries.read().await;
        let entry = entries.get(&123).unwrap();
        assert!(entry.fetched_at.elapsed() < cache.refresh_interval);
        assert_eq!(entry.raw_log, "cached log content");
    }

    #[tokio::test]
    async fn test_mark_completed_and_evict() {
        let cache = StepLogCache {
            entries: Arc::new(RwLock::new(HashMap::new())),
            refresh_interval: Duration::from_secs(5),
            ttl_after_completion: Duration::from_millis(1), // Very short for testing
        };

        {
            let mut entries = cache.entries.write().await;
            entries.insert(
                456,
                CacheEntry {
                    raw_log: "log".to_string(),
                    fetched_at: Instant::now() - Duration::from_secs(1), // Already old
                    job_completed: false,
                },
            );
        }

        cache.mark_completed(456).await;
        cache.evict_expired().await;

        let entries = cache.entries.read().await;
        assert!(entries.get(&456).is_none()); // Should be evicted
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p homerund -- step_log_cache::tests --nocapture`
Expected: PASS

- [ ] **Step 3: Add module to runner/mod.rs**

```rust
pub mod step_log_cache;
```

- [ ] **Step 4: Add StepLogCache to RunnerManager**

In `crates/daemon/src/runner/mod.rs`, add field and initialization:

```rust
pub step_log_cache: step_log_cache::StepLogCache,

// In RunnerManager::new():
step_log_cache: step_log_cache::StepLogCache::new(),
```

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/step_log_cache.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add in-memory job log cache with TTL for step logs"
```

---

## Task 7: Daemon REST Endpoints for Steps

**Files:**

- Create: `crates/daemon/src/api/steps.rs`
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Create the steps API module with tests**

Create `crates/daemon/src/api/steps.rs`:

```rust
use crate::server::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::steps::StepsResponse;

#[derive(serde::Serialize)]
pub struct StepLogsResponse {
    pub step_number: u16,
    pub step_name: String,
    pub lines: Vec<String>,
}

pub async fn get_steps(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StepsResponse>, StatusCode> {
    state
        .runner_manager
        .get_steps(&id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn get_step_logs(
    State(state): State<AppState>,
    Path((id, step_number)): Path<(String, u16)>,
) -> Result<Json<StepLogsResponse>, StatusCode> {
    state
        .runner_manager
        .get_step_logs(&id, step_number)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_steps_returns_404_for_unknown_runner() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/unknown-id/steps")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_step_logs_returns_404_for_unknown_runner() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/unknown-id/steps/1/logs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
```

- [ ] **Step 2: Add `get_step_logs` to RunnerManager**

In `crates/daemon/src/runner/mod.rs`, add the method that fetches logs from GitHub API and parses them:

```rust
impl RunnerManager {
    pub async fn get_step_logs(
        &self,
        runner_id: &str,
        step_number: u16,
    ) -> Option<crate::api::steps::StepLogsResponse> {
        // Get step info to find the name
        let steps_response = self.get_steps(runner_id).await?;
        let step = steps_response
            .steps
            .iter()
            .find(|s| s.number == step_number)?;
        let step_name = step.name.clone();

        // Get job_id from job_context
        let runners = self.runners.read().await;
        let runner = runners.get(runner_id)?;
        let job_id = runner.job_context.as_ref()?.job_id?;
        let owner = runner.config.repo_owner.clone();
        let repo = runner.config.repo_name.clone();
        drop(runners);

        // Get auth token (same pattern as start_job_context_poller)
        let token = {
            let t = self.auth_token.read().await;
            t.clone()
        };
        let token = token?;
        let gh = crate::github::GitHubClient::new(Some(token)).ok()?;

        // Check cache first; fetch from GitHub API if miss
        let raw_log = self.step_log_cache.get_or_fetch(
            job_id, &gh, &owner, &repo,
        ).await.ok()?;
        let sections = crate::github::parse_job_log_sections(&raw_log);

        // Match by step name (not index) since Worker log and API may differ in order
        let section = sections.iter().find(|(name, _)| name == &step_name)?;
        let lines: Vec<String> = section.1.lines().map(|l| l.to_string()).collect();

        Some(crate::api::steps::StepLogsResponse {
            step_number,
            step_name,
            lines,
        })
    }
}
```

- [ ] **Step 3: Register the module and routes**

In `crates/daemon/src/api/mod.rs`, add:

```rust
pub mod steps;
```

In `crates/daemon/src/server.rs`, add routes after line 108 (after the logs routes):

```rust
.route("/runners/{id}/steps", get(api::steps::get_steps))
.route("/runners/{id}/steps/{step_number}/logs", get(api::steps::get_step_logs))
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund --nocapture`
Expected: All PASS

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/api/steps.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add /steps and /steps/{n}/logs daemon API endpoints"
```

---

## Task 8: Tauri Backend — IPC Commands for Steps

**Files:**

- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add types and client methods**

In `apps/desktop/src-tauri/src/client.rs`, add types after `LogEntry` (line 65):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLogsResponse {
    pub step_number: u16,
    pub step_name: String,
    pub lines: Vec<String>,
}
```

Add client methods (after `get_runner_logs`):

```rust
pub async fn get_runner_steps(&self, runner_id: &str) -> Result<StepsResponse, String> {
    let body = self
        .request("GET", &format!("/runners/{runner_id}/steps"), None)
        .await?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

pub async fn get_step_logs(
    &self,
    runner_id: &str,
    step_number: u16,
) -> Result<StepLogsResponse, String> {
    let body = self
        .request(
            "GET",
            &format!("/runners/{runner_id}/steps/{step_number}/logs"),
            None,
        )
        .await?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Add Tauri commands**

In `apps/desktop/src-tauri/src/commands.rs`, add imports and commands:

```rust
// Add to imports at top:
use crate::client::{StepsResponse, StepLogsResponse};

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_steps(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<StepsResponse, String> {
    let client = state.client.lock().await;
    client.get_runner_steps(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_step_logs(
    state: State<'_, AppState>,
    runner_id: String,
    step_number: u16,
) -> Result<StepLogsResponse, String> {
    let client = state.client.lock().await;
    client.get_step_logs(&runner_id, step_number).await
}
```

- [ ] **Step 3: Register commands**

In `apps/desktop/src-tauri/src/lib.rs`, add to the `generate_handler!` macro:

```rust
commands::get_runner_steps,
commands::get_step_logs,
```

- [ ] **Step 4: Build to verify**

Run: `cd apps/desktop && npm run build`
Expected: Builds successfully (tsc + vite)

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/client.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add Tauri IPC commands for step progress and step logs"
```

---

## Task 9: Tauri Frontend — Types, API, and Hook

**Files:**

- Modify: `apps/desktop/src/api/types.ts`
- Modify: `apps/desktop/src/api/commands.ts`
- Create: `apps/desktop/src/hooks/useJobSteps.ts`

- [ ] **Step 1: Add TypeScript types**

In `apps/desktop/src/api/types.ts`, add after `LogEntry` (line 45):

```typescript
export type StepStatus = "pending" | "running" | "succeeded" | "failed" | "skipped";

export interface StepInfo {
  number: number;
  name: string;
  status: StepStatus;
  started_at: string | null;
  completed_at: string | null;
}

export interface StepsResponse {
  job_name: string;
  steps: StepInfo[];
  steps_discovered: number;
}

export interface StepLogsResponse {
  step_number: number;
  step_name: string;
  lines: string[];
}
```

- [ ] **Step 2: Add API commands**

In `apps/desktop/src/api/commands.ts`, add imports and methods:

```typescript
// Add to imports:
import type { StepsResponse, StepLogsResponse } from "./types";

// Add to api object, after getRunnerLogs:
getRunnerSteps: (runnerId: string) =>
  invoke<StepsResponse>("get_runner_steps", { runner_id: runnerId }),
getStepLogs: (runnerId: string, stepNumber: number) =>
  invoke<StepLogsResponse>("get_step_logs", { runner_id: runnerId, step_number: stepNumber }),
```

- [ ] **Step 3: Create useJobSteps hook**

Create `apps/desktop/src/hooks/useJobSteps.ts`:

```typescript
import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/commands";
import type { StepInfo, StepsResponse, StepLogsResponse } from "../api/types";

interface UseJobStepsResult {
  steps: StepInfo[];
  stepsDiscovered: number;
  jobName: string | null;
  loading: boolean;
  expandedStep: number | null;
  stepLogs: Record<number, string[]>;
  toggleStep: (stepNumber: number) => void;
}

export function useJobSteps(runnerId: string | undefined, isBusy: boolean): UseJobStepsResult {
  const [stepsResponse, setStepsResponse] = useState<StepsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [expandedStep, setExpandedStep] = useState<number | null>(null);
  const [stepLogs, setStepLogs] = useState<Record<number, string[]>>({});
  const logCacheRef = useRef<Record<number, string[]>>({});

  // Poll step progress
  useEffect(() => {
    if (!runnerId || !isBusy) {
      setStepsResponse(null);
      setExpandedStep(null);
      setStepLogs({});
      logCacheRef.current = {};
      return;
    }

    let cancelled = false;

    async function fetchSteps() {
      try {
        const response = await api.getRunnerSteps(runnerId!);
        if (!cancelled) {
          setStepsResponse(response);
          setLoading(false);
        }
      } catch {
        // Runner may not have step data yet
        if (!cancelled) setLoading(false);
      }
    }

    setLoading(true);
    fetchSteps();
    const timer = setInterval(fetchSteps, 1000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [runnerId, isBusy]);

  // Fetch step logs when a step is expanded
  useEffect(() => {
    if (!runnerId || expandedStep === null) return;

    // If already cached and step is completed, don't re-fetch
    const step = stepsResponse?.steps.find((s) => s.number === expandedStep);
    const isCompleted =
      step?.status === "succeeded" || step?.status === "failed" || step?.status === "skipped";
    if (isCompleted && logCacheRef.current[expandedStep]) return;

    let cancelled = false;

    async function fetchLogs() {
      try {
        const response = await api.getStepLogs(runnerId!, expandedStep!);
        if (!cancelled) {
          logCacheRef.current[expandedStep!] = response.lines;
          setStepLogs({ ...logCacheRef.current });
        }
      } catch {
        // Logs not available yet
      }
    }

    fetchLogs();

    // For running steps, refresh every 5s
    if (step?.status === "running") {
      const timer = setInterval(fetchLogs, 5000);
      return () => {
        cancelled = true;
        clearInterval(timer);
      };
    }

    return () => {
      cancelled = true;
    };
  }, [runnerId, expandedStep, stepsResponse]);

  const toggleStep = useCallback((stepNumber: number) => {
    setExpandedStep((prev) => (prev === stepNumber ? null : stepNumber));
  }, []);

  return {
    steps: stepsResponse?.steps ?? [],
    stepsDiscovered: stepsResponse?.steps_discovered ?? 0,
    jobName: stepsResponse?.job_name ?? null,
    loading,
    expandedStep,
    stepLogs,
    toggleStep,
  };
}
```

- [ ] **Step 4: Type-check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 5: Format**

Run: `cd apps/desktop && npx prettier --write src/`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/api/types.ts apps/desktop/src/api/commands.ts apps/desktop/src/hooks/useJobSteps.ts
git commit -m "feat: add TypeScript types, API commands, and useJobSteps hook"
```

---

## Task 10: Tauri Frontend — JobProgress Component

**Files:**

- Create: `apps/desktop/src/components/JobProgress.tsx`
- Modify: `apps/desktop/src/pages/RunnerDetail.tsx`

- [ ] **Step 1: Create JobProgress component**

Create `apps/desktop/src/components/JobProgress.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import type { StepInfo } from "../api/types";

function formatDuration(startedAt: string | null, completedAt: string | null): string {
  if (!startedAt) return "—";
  const start = new Date(startedAt).getTime();
  const end = completedAt ? new Date(completedAt).getTime() : Date.now();
  const secs = Math.round((end - start) / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const remainSecs = secs % 60;
  return `${mins}m ${remainSecs}s`;
}

function StepIcon({ status }: { status: string }) {
  switch (status) {
    case "succeeded":
      return (
        <span
          style={{
            color: "var(--accent-green)",
            fontSize: 14,
            width: 18,
            textAlign: "center",
            display: "inline-block",
          }}
        >
          ✓
        </span>
      );
    case "failed":
      return (
        <span
          style={{
            color: "var(--accent-red)",
            fontSize: 14,
            width: 18,
            textAlign: "center",
            display: "inline-block",
          }}
        >
          ✕
        </span>
      );
    case "skipped":
      return (
        <span
          style={{
            color: "var(--text-secondary)",
            fontSize: 14,
            width: 18,
            textAlign: "center",
            display: "inline-block",
          }}
        >
          ⊘
        </span>
      );
    case "running":
      return (
        <span
          style={{
            width: 18,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <span className="step-spinner" />
        </span>
      );
    default:
      return (
        <span
          style={{
            color: "var(--text-secondary)",
            fontSize: 14,
            width: 18,
            textAlign: "center",
            display: "inline-block",
            opacity: 0.5,
          }}
        >
          ○
        </span>
      );
  }
}

interface JobProgressProps {
  steps: StepInfo[];
  stepsDiscovered: number;
  jobName: string | null;
  expandedStep: number | null;
  stepLogs: Record<number, string[]>;
  onToggleStep: (stepNumber: number) => void;
  elapsedSecs?: number;
}

export function JobProgress({
  steps,
  stepsDiscovered,
  jobName,
  expandedStep,
  stepLogs,
  onToggleStep,
  elapsedSecs,
}: JobProgressProps) {
  const logContainerRef = useRef<HTMLDivElement>(null);
  const completedCount = steps.filter(
    (s) => s.status === "succeeded" || s.status === "failed" || s.status === "skipped",
  ).length;

  // Auto-scroll expanded step logs
  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [stepLogs, expandedStep]);

  // Live elapsed timer for running steps
  const [, setTick] = useState(0);
  const hasRunningStep = steps.some((s) => s.status === "running");
  useEffect(() => {
    if (!hasRunningStep) return;
    const timer = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(timer);
  }, [hasRunningStep]);

  if (steps.length === 0) return null;

  const formatElapsed = (secs?: number) => {
    if (secs == null) return "";
    if (secs < 60) return `${secs}s`;
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}m ${s}s`;
  };

  return (
    <div className="runner-card" style={{ marginBottom: 16, padding: 0, overflow: "hidden" }}>
      <div
        style={{
          padding: "10px 14px",
          borderBottom: "1px solid var(--border)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="runner-card-label" style={{ margin: 0 }}>
            Job Progress
          </span>
          <span
            style={{
              fontSize: 11,
              color: "var(--text-secondary)",
              background: "var(--bg-secondary)",
              padding: "1px 6px",
              borderRadius: 4,
            }}
          >
            {completedCount}/{stepsDiscovered} steps
          </span>
        </div>
        {elapsedSecs != null && (
          <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
            Elapsed: {formatElapsed(elapsedSecs)}
          </span>
        )}
      </div>

      <div style={{ fontSize: 13 }}>
        {steps.map((step) => {
          const isExpanded = expandedStep === step.number;
          const isRunning = step.status === "running";
          const isPending = step.status === "pending";
          const logs = stepLogs[step.number];

          return (
            <div key={step.number}>
              <div
                onClick={() => !isPending && onToggleStep(step.number)}
                style={{
                  padding: "6px 14px",
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  cursor: isPending ? "default" : "pointer",
                  opacity: isPending ? 0.5 : 1,
                  background: isRunning ? "var(--bg-secondary)" : "transparent",
                  borderLeft: isRunning
                    ? "2px solid var(--accent-yellow)"
                    : "2px solid transparent",
                }}
                onMouseOver={(e) => {
                  if (!isPending) e.currentTarget.style.background = "var(--bg-secondary)";
                }}
                onMouseOut={(e) => {
                  if (!isRunning) e.currentTarget.style.background = "transparent";
                }}
              >
                <StepIcon status={step.status} />
                <span
                  style={{
                    flex: 1,
                    color: isRunning ? "var(--accent-yellow)" : "var(--text-primary)",
                    fontWeight: isRunning ? 500 : 400,
                  }}
                >
                  {step.name}
                </span>
                <span
                  className="font-mono"
                  style={{
                    fontSize: 12,
                    color: isRunning ? "var(--accent-yellow)" : "var(--text-secondary)",
                  }}
                >
                  {isPending ? "—" : formatDuration(step.started_at, step.completed_at)}
                  {isRunning && "…"}
                </span>
                {!isPending && (
                  <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                    {isExpanded ? "▾" : "▸"}
                  </span>
                )}
              </div>

              {isExpanded && logs && (
                <div
                  ref={isExpanded ? logContainerRef : undefined}
                  className="font-mono"
                  style={{
                    padding: "4px 14px 10px 42px",
                    fontSize: 11,
                    lineHeight: 1.7,
                    color: "var(--text-secondary)",
                    maxHeight: 200,
                    overflowY: "auto",
                    background: "var(--bg-secondary)",
                    borderLeft: isRunning
                      ? "2px solid var(--accent-yellow)"
                      : "2px solid transparent",
                  }}
                >
                  {logs.map((line, i) => (
                    <div key={i}>{line || "\u00A0"}</div>
                  ))}
                  {isRunning && !logs.length && (
                    <div style={{ color: "var(--text-secondary)", fontStyle: "italic" }}>
                      Fetching logs...
                    </div>
                  )}
                </div>
              )}

              {isExpanded && !logs && (
                <div
                  style={{
                    padding: "8px 14px 8px 42px",
                    fontSize: 11,
                    color: "var(--text-secondary)",
                    fontStyle: "italic",
                    background: "var(--bg-secondary)",
                  }}
                >
                  Loading logs...
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS for the spinner**

Add to `apps/desktop/src/index.css`:

```css
.step-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--accent-yellow);
  border-top-color: transparent;
  border-radius: 50%;
  animation: step-spin 1s linear infinite;
  display: inline-block;
}

@keyframes step-spin {
  to {
    transform: rotate(360deg);
  }
}
```

- [ ] **Step 3: Integrate into RunnerDetail.tsx**

In `apps/desktop/src/pages/RunnerDetail.tsx`:

Add imports:

```typescript
import { useJobSteps } from "../hooks/useJobSteps";
import { JobProgress } from "../components/JobProgress";
```

Add hook call (after the existing hooks, around line 133):

```typescript
const { steps, stepsDiscovered, jobName, expandedStep, stepLogs, toggleStep } = useJobSteps(
  id,
  runner?.state === "busy",
);
```

Insert JobProgress component between the cards row and the Logs panel (after the Labels section, around line 451, before the `{/* Logs panel */}` comment):

```tsx
{
  /* Job Progress panel — visible when busy */
}
{
  runner.state === "busy" && steps.length > 0 && (
    <JobProgress
      steps={steps}
      stepsDiscovered={stepsDiscovered}
      jobName={jobName}
      expandedStep={expandedStep}
      stepLogs={stepLogs}
      onToggleStep={toggleStep}
    />
  );
}
```

Rename the Logs panel header from "Logs" to "Runner Process Logs":

In the logs panel section (around line 456):

```tsx
<h3 className="runner-card-label" style={{ margin: 0 }}>
  Runner Process Logs
</h3>
```

- [ ] **Step 4: Type-check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 5: Format**

Run: `cd apps/desktop && npx prettier --write src/`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/JobProgress.tsx apps/desktop/src/pages/RunnerDetail.tsx
git commit -m "feat: add JobProgress component to runner detail page"
```

---

## Task 11: TUI — Step Progress Display

**Files:**

- Modify: `crates/tui/src/client.rs`
- Modify: `crates/tui/src/ui/runners.rs`

- [ ] **Step 1: Add types and client method to TUI**

In `crates/tui/src/client.rs`, add types after `LogEntry`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}
```

Add client method (after `get_runner_logs`):

```rust
pub async fn get_runner_steps(&self, runner_id: &str) -> Result<StepsResponse> {
    let body = self
        .request("GET", &format!("/runners/{runner_id}/steps"), None)
        .await?;
    serde_json::from_str(&body).context("Failed to parse steps response")
}
```

- [ ] **Step 2: Add step display to runner detail view**

In `crates/tui/src/ui/runners.rs`, in the `format_runner_detail()` function (or the runner detail rendering section), add step progress display when runner is busy:

```rust
// After current job display, add steps:
if let Some(steps_response) = &app.selected_runner_steps {
    lines.push_str("\n Job Steps:\n");
    for step in &steps_response.steps {
        let (icon, style) = match step.status.as_str() {
            "succeeded" => ("✓", Style::default().fg(Color::Green)),
            "failed" => ("✕", Style::default().fg(Color::Red)),
            "running" => ("⟳", Style::default().fg(Color::Yellow)),
            "skipped" => ("⊘", Style::default().fg(Color::DarkGray)),
            _ => ("○", Style::default().fg(Color::DarkGray)),
        };
        let duration = format_step_duration(&step.started_at, &step.completed_at);
        lines.push_str(&format!("  {} {} {}\n", icon, step.name, duration));
    }
}
```

Note: The exact integration depends on how `App` state is updated. Add a field `selected_runner_steps: Option<StepsResponse>` to `App` and fetch it in the update loop when the selected runner is busy.

- [ ] **Step 3: Add step fetching to the TUI update loop**

In the TUI's main update/tick handler, add step polling for the selected runner when busy:

```rust
// In the periodic update (tick handler), fetch steps for selected busy runner
if let Some(runner) = selected_runner {
    if runner.state == "busy" {
        if let Ok(steps) = client.get_runner_steps(&runner.config.id).await {
            app.selected_runner_steps = Some(steps);
        }
    } else {
        app.selected_runner_steps = None;
    }
}
```

- [ ] **Step 4: Run TUI tests**

Run: `cargo test -p homerun --nocapture`
Expected: All PASS

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt -p homerun && cargo clippy -p homerun --all-targets --all-features -- -D warnings`
Expected: Clean

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/client.rs crates/tui/src/ui/runners.rs crates/tui/src/app.rs
git commit -m "feat: add job step progress display to TUI runner detail view"
```

---

## Task 12: Manual Testing and Polish

- [ ] **Step 1: Start daemon and trigger a workflow**

Run: `cargo run -p homerund` in one terminal, create/start a runner in another, trigger a workflow that uses it.

- [ ] **Step 2: Test step progress in Tauri app**

Run: `cd apps/desktop && npm run tauri dev`

Verify:

- Steps appear when runner goes Busy
- Steps update in real-time (new steps appear, running → succeeded)
- Duration counts up for running step
- Clicking a completed step fetches and shows logs
- Clicking the running step shows logs that refresh
- Panel disappears when job completes
- "Runner Process Logs" label is correct

- [ ] **Step 3: Test step progress in TUI**

Run: `cargo run -p homerun`

Verify:

- Steps display in runner detail when busy
- Icons and colors are correct
- Steps update as job progresses

- [ ] **Step 4: Run full test suite**

Run: `cargo test && cd apps/desktop && npx tsc --noEmit`
Expected: All PASS

- [ ] **Step 5: Run linters**

Run: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cd apps/desktop && npx prettier --check src/`
Expected: All clean

- [ ] **Step 6: Final commit if any polish changes**

```bash
git add -A
git commit -m "fix: polish job progress step display"
```
