# Fix Missing Job Steps in Dashboard — Design Spec

**Issue:** #52 — Job steps in dashboard don't match GitHub Actions UI
**Date:** 2026-03-24

## Problem

The HomeRun dashboard shows fewer job steps than the GitHub Actions UI for the same job. Steps like "Ensure Rust is available", "Check formatting", "Run clippy" are missing entirely, and separate coverage steps are collapsed into one.

## Root Cause

`start_watching()` is called when `JobEvent::Started` is detected from the Runner log, but the Worker process hasn't spawned yet. `find_latest_worker_log()` runs once at init time and either:

1. Returns `None` — the Worker log doesn't exist yet → `poll()` never retries, so no steps are ever discovered.
2. Returns a stale log from a previous job → reads old steps, never detects the new log file.

Secondary issue: `parse_step_event` silently drops `Cancelled` step results (the match arm falls through to `return None`).

## Approach

Improve local Worker log parsing — no additional GitHub API calls.

## Changes

### 1. Data Model (`steps.rs`)

Add `work_dir` to `RunnerStepState` so `poll()` can retry log discovery:

```rust
struct RunnerStepState {
    job_name: String,
    steps: Vec<StepInfo>,
    file_offset: u64,
    log_path: Option<PathBuf>,
    work_dir: PathBuf,  // NEW
}
```

`start_watching()` stores `work_dir` and does NOT call `find_latest_worker_log()` eagerly — the first `poll()` will find the log, eliminating the race.

Add `Cancelled` variant to `StepStatus`:

```rust
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,  // NEW
}
```

### 2. `poll()` Logic

Updated `poll()` handles two cases before reading:

1. **`log_path` is `None`**: call `find_latest_worker_log(work_dir)`. If found, set it and continue. If not, return early (keep polling).

2. **`log_path` is `Some` but no new bytes**: call `find_latest_worker_log(work_dir)` and compare paths. If a different (newer) log file is found, switch to it — reset `file_offset` to 0 and clear `steps`.

The "check for newer log" only runs when no new bytes have been written to the current file, avoiding unnecessary filesystem calls when bytes are actively flowing.

### 3. `parse_step_event` Fix

Add `"Cancelled"` to the result match in `parse_step_event()`:

```rust
"Cancelled" => StepStatus::Cancelled,
```

### 4. TypeScript Types (`apps/desktop/src/api/types.ts`)

Add `"cancelled"` to the `StepStatus` union type.

## Files Modified

- `crates/daemon/src/runner/steps.rs` — `RunnerStepState`, `StepStatus`, `start_watching()`, `poll()`, `parse_step_event()`
- `apps/desktop/src/api/types.ts` — `StepStatus` type

## Test Plan

**New unit tests in `steps.rs`:**

- `test_poll_retries_log_discovery` — start watching with no `_diag/` dir, poll returns true but no steps. Create the log file, poll again → steps discovered.
- `test_poll_detects_newer_log_file` — start with an old Worker log containing steps. Create a newer Worker log with different steps. Poll with no new bytes → switches to new file, steps reset.
- `test_parse_step_cancelled` — verify `"Cancelled"` result string parses to `StepStatus::Cancelled`.

Existing tests pass unchanged.
