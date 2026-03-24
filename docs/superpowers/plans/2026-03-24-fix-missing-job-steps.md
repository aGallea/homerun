# Fix Missing Job Steps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the Worker log watcher so all job steps are discovered, even when the Worker log file doesn't exist yet at job start time.

**Architecture:** Store `work_dir` in watcher state so `poll()` can retry log file discovery. Defer `find_latest_worker_log()` from `start_watching()` to `poll()`, and detect when a newer log file replaces a stale one.

**Tech Stack:** Rust (daemon crate), TypeScript (desktop app types)

**Spec:** `docs/superpowers/specs/2026-03-24-fix-missing-job-steps-design.md`

---

## File Map

- Modify: `crates/daemon/src/runner/steps.rs` — all core changes (data model, poll logic, parsing)
- Modify: `apps/desktop/src/api/types.ts:22` — add `"cancelled"` to `StepStatus` union

---

## Task 1: Add `Cancelled` variant to `StepStatus` and parse it

**Files:**

- Modify: `crates/daemon/src/runner/steps.rs:13-19` (enum), `crates/daemon/src/runner/steps.rs:99-103` (parser)

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/daemon/src/runner/steps.rs`:

```rust
#[test]
fn test_parse_step_cancelled() {
    let line = "[2026-03-23 07:54:55Z INFO StepsRunner] Updating job result with current step result 'Cancelled'.";
    let event = parse_step_event(line);
    assert!(event.is_some());
    match event.unwrap() {
        StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Cancelled),
        other => panic!("Expected Completed, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund test_parse_step_cancelled -- --nocapture`
Expected: FAIL — `Cancelled` variant doesn't exist.

- [ ] **Step 3: Add `Cancelled` variant and update parser**

In `crates/daemon/src/runner/steps.rs`, add `Cancelled` to `StepStatus` enum (after `Skipped`):

```rust
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}
```

And add the match arm in `parse_step_event` (line 102, before the `_ =>` wildcard):

```rust
"Cancelled" => StepStatus::Cancelled,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p homerund test_parse_step_cancelled -- --nocapture`
Expected: PASS

- [ ] **Step 5: Update TypeScript type**

In `apps/desktop/src/api/types.ts:22`, change:

```typescript
export type StepStatus = "pending" | "running" | "succeeded" | "failed" | "skipped" | "cancelled";
```

- [ ] **Step 6: Run all tests and checks**

Run: `cargo test -p homerund && cd apps/desktop && npx tsc --noEmit`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/runner/steps.rs apps/desktop/src/api/types.ts
git commit -m "fix: add Cancelled variant to StepStatus (#52)"
```

---

## Task 2: Add `work_dir` to `RunnerStepState` and defer log discovery

**Files:**

- Modify: `crates/daemon/src/runner/steps.rs:112-117` (struct), `crates/daemon/src/runner/steps.rs:144-156` (`start_watching`)

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/daemon/src/runner/steps.rs`:

```rust
#[tokio::test]
async fn test_poll_retries_log_discovery() {
    let dir = tempfile::tempdir().unwrap();
    // No _diag directory — simulates Worker not spawned yet

    let watcher = WorkerLogWatcher::new();
    watcher
        .start_watching("runner-retry", "retry-job", dir.path())
        .await;

    // First poll: no log file yet, returns true (keep polling), no steps
    assert!(watcher.poll("runner-retry").await);
    let resp = watcher.get_steps("runner-retry").await.unwrap();
    assert_eq!(resp.steps.len(), 0);

    // Now create the _diag dir and Worker log (simulates Worker spawning)
    let diag = dir.path().join("_diag");
    std::fs::create_dir_all(&diag).unwrap();
    let log_content = "\
[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Checkout'\n\
[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.\n\
[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.\n";
    std::fs::write(diag.join("Worker_20260323-080000-utc.log"), log_content).unwrap();

    // Second poll: should discover the log and parse steps
    assert!(watcher.poll("runner-retry").await);
    let resp = watcher.get_steps("runner-retry").await.unwrap();
    assert_eq!(resp.steps.len(), 1);
    assert_eq!(resp.steps[0].name, "Checkout");
    assert_eq!(resp.steps[0].status, StepStatus::Succeeded);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund test_poll_retries_log_discovery -- --nocapture`
Expected: FAIL — `poll()` currently does not retry log discovery when `log_path` is `None`.

- [ ] **Step 3: Add `work_dir` to `RunnerStepState` and update `start_watching()`**

In `crates/daemon/src/runner/steps.rs`, update the struct:

```rust
struct RunnerStepState {
    job_name: String,
    steps: Vec<StepInfo>,
    file_offset: u64,
    log_path: Option<PathBuf>,
    work_dir: PathBuf,
}
```

Update `start_watching()` to store `work_dir` and skip eager log discovery:

```rust
pub async fn start_watching(&self, runner_id: &str, job_name: &str, work_dir: &Path) {
    let state = RunnerStepState {
        job_name: job_name.to_string(),
        steps: Vec::new(),
        file_offset: 0,
        log_path: None,
        work_dir: work_dir.to_path_buf(),
    };
    self.step_state
        .write()
        .await
        .insert(runner_id.to_string(), state);
}
```

- [ ] **Step 4: Update `poll()` to retry log discovery when `log_path` is `None`**

Replace the current `poll()` method with:

```rust
pub async fn poll(&self, runner_id: &str) -> bool {
    let mut map = self.step_state.write().await;
    let Some(state) = map.get_mut(runner_id) else {
        return false;
    };

    // If we don't have a log path yet, try to find one
    if state.log_path.is_none() {
        state.log_path = find_latest_worker_log(&state.work_dir);
        if state.log_path.is_none() {
            return true; // Keep polling, log not created yet
        }
    }

    let log_path = state.log_path.as_ref().unwrap();

    let Ok(metadata) = std::fs::metadata(log_path) else {
        return true;
    };

    let file_len = metadata.len();
    if file_len <= state.file_offset {
        // No new bytes — check if a newer log file appeared
        if let Some(newer) = find_latest_worker_log(&state.work_dir) {
            if newer != *log_path {
                state.log_path = Some(newer);
                state.file_offset = 0;
                state.steps.clear();
            }
        }
        return true;
    }

    let Ok(content) = std::fs::read_to_string(log_path) else {
        return true;
    };

    // Read only the new portion of the file.
    let new_bytes = &content.as_bytes()[state.file_offset as usize..];
    let new_text = String::from_utf8_lossy(new_bytes);

    for line in new_text.lines() {
        if let Some(event) = parse_step_event(line) {
            apply_step_event(&mut state.steps, event);
        }
    }

    state.file_offset = content.len() as u64;
    true
}
```

- [ ] **Step 5: Update `start_watching()` doc comment**

Replace the doc comment on `start_watching()`:

```rust
/// Begin watching a runner's Worker log for step progress.
///
/// Stores the work directory; the actual log file is discovered lazily
/// during the first `poll()` call, avoiding races when the Worker
/// process hasn't spawned yet.
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p homerund test_poll_retries_log_discovery -- --nocapture`
Expected: PASS

- [ ] **Step 7: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass (existing tests still work because `start_watching` with an existing log file will be found on first `poll()`).

- [ ] **Step 8: Commit**

```bash
git add crates/daemon/src/runner/steps.rs
git commit -m "fix: defer Worker log discovery to poll() for reliable step tracking (#52)"
```

---

## Task 3: Detect newer Worker log files (stale log switchover)

**Files:**

- Modify: `crates/daemon/src/runner/steps.rs` (test only — logic already added in Task 2)

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/daemon/src/runner/steps.rs`:

```rust
#[tokio::test]
async fn test_poll_detects_newer_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let diag = dir.path().join("_diag");
    std::fs::create_dir_all(&diag).unwrap();

    // Create an "old" Worker log with one step
    let old_log = diag.join("Worker_20260323-070000-utc.log");
    let old_content = "\
[2026-03-23 07:00:00Z INFO StepsRunner] Processing step: DisplayName='OldStep'\n\
[2026-03-23 07:00:00Z INFO StepsRunner] Starting the step.\n\
[2026-03-23 07:00:01Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.\n";
    std::fs::write(&old_log, old_content).unwrap();

    let watcher = WorkerLogWatcher::new();
    watcher
        .start_watching("runner-stale", "stale-job", dir.path())
        .await;

    // First poll reads old log
    assert!(watcher.poll("runner-stale").await);
    let resp = watcher.get_steps("runner-stale").await.unwrap();
    assert_eq!(resp.steps.len(), 1);
    assert_eq!(resp.steps[0].name, "OldStep");

    // Create a newer Worker log (simulates new job's Worker process)
    // Sleep briefly so the new file has a strictly newer mtime
    std::thread::sleep(std::time::Duration::from_millis(50));
    let new_log = diag.join("Worker_20260323-080000-utc.log");
    let new_content = "\
[2026-03-23 08:00:00Z INFO StepsRunner] Processing step: DisplayName='NewStep'\n\
[2026-03-23 08:00:00Z INFO StepsRunner] Starting the step.\n";
    std::fs::write(&new_log, new_content).unwrap();

    // Second poll: no new bytes on old file → checks for newer log → switches
    assert!(watcher.poll("runner-stale").await);
    let resp = watcher.get_steps("runner-stale").await.unwrap();
    assert_eq!(resp.steps.len(), 1);
    assert_eq!(resp.steps[0].name, "NewStep");
    assert_eq!(resp.steps[0].status, StepStatus::Running);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p homerund test_poll_detects_newer_log_file -- --nocapture`
Expected: PASS (the switchover logic was already implemented in Task 2's `poll()` update).

- [ ] **Step 3: Run full test suite and lint**

Run: `cargo test -p homerund && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/runner/steps.rs
git commit -m "test: add stale log switchover test for step watcher (#52)"
```

---

## Task 4: Final verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 2: Run desktop app type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings.

- [ ] **Step 4: Run formatting checks**

Run: `cargo fmt --check && cd apps/desktop && npx prettier --check src/`
Expected: No formatting issues.
