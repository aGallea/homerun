# Job Progress Steps — Design Spec

**Date:** 2026-03-23
**Status:** Approved

## Overview

Show real-time workflow job step progress on the runner detail page when a runner is busy executing a GitHub Actions job. Step progress (names, status, timing) is parsed locally from the runner's `_diag/Worker_*.log` file. Step log content is fetched on-demand from the GitHub Actions API when a user expands a step.

## Motivation

HomeRun currently shows that a runner is busy and what job it's running, but provides no visibility into which step is executing, which steps have completed, or how long each step took. GitHub's workflow run UI shows this (steps with checkmarks, spinners, durations, expandable logs). We want the same experience locally in HomeRun.

## Data Sources

### Step Progress — Local Worker Log

The GitHub Actions runner's Worker process writes structured log lines to `{runner_work_dir}/_diag/Worker_*.log`. Three patterns provide full step lifecycle:

```
[timestamp INFO StepsRunner] Processing step: DisplayName='Step Name'
[timestamp INFO StepsRunner] Starting the step.
[timestamp INFO StepsRunner] ...current step result 'Succeeded|Failed'
```

These are parsed locally with zero API calls.

### Step Logs — GitHub Actions API (On-Demand)

The actual step output (e.g., "Compiling foo v1.0") is not retained locally — the runner uploads it to GitHub and empties the `_diag/blocks/` files. Step log content is fetched from:

```
GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs
```

This returns the full job log with step markers. One API call covers all steps of a job; results are cached in memory.

**Future enhancement:** Capture step logs locally from `_diag/blocks/` before upload. See [issue #44](https://github.com/aGallea/homerun/issues/44) for risks and approach.

## Architecture

### 1. Worker Log Watcher (Daemon)

New `WorkerLogWatcher` struct in `crates/daemon/src/runner/`:

- **Activates** when `JobEvent::Started` fires — finds the latest `Worker_*.log` in `{work_dir}/_diag/` by modification time
- **Tails the file** using a polling interval (~500ms) — tracks byte offset, reads only new content appended since last read
- **Parses step events** from new lines:
  - `Processing step: DisplayName='(.*)'` → step discovered, added as Pending
  - `Starting the step.` → most recently discovered step transitions to Running
  - `step result '(Succeeded|Failed)'` → running step transitions to Succeeded/Failed, timestamps captured for duration calculation
- **Stores step state** in a `Vec<StepInfo>` per runner, behind an `RwLock`
- **Deactivates** when `JobEvent::Completed` fires — keeps last job's steps briefly, then clears

**Step state machine:** `Pending → Running → Succeeded | Failed`

**Data structure:**

```rust
pub struct StepInfo {
    pub number: u16,        // 1-based step index
    pub name: String,       // from DisplayName
    pub status: StepStatus, // Pending | Running | Succeeded | Failed
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}
```

**Edge cases:**

- Runner restarts mid-job → watcher resets, re-parses from beginning of latest Worker log
- Multiple Worker logs → always pick newest by modification time
- Worker log doesn't exist yet when job starts → retry finding it for a few seconds

### 2. JobContext Enhancement

The existing `JobContext` struct and job context poller (`get_active_run_for_runner()`) already fetch workflow run + jobs data from GitHub API. Enhancement: store `job_id: Option<u64>` in `JobContext` when the poller matches the current job. This ID is needed for the step logs API call.

```rust
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: Option<String>,
    pub job_id: Option<u64>,  // NEW — GitHub Actions job ID
}
```

### 3. Daemon API Endpoints

**`GET /runners/{id}/steps`** — Step progress for a runner

Returns current step state parsed from Worker log. No API call, instant response.

```json
{
  "job_name": "Rust (fmt + clippy + test + coverage)",
  "steps": [
    {
      "number": 1,
      "name": "Set up job",
      "status": "succeeded",
      "started_at": "2026-03-23T07:54:51Z",
      "completed_at": "2026-03-23T07:54:53Z"
    },
    {
      "number": 7,
      "name": "Run tests with coverage (daemon)",
      "status": "running",
      "started_at": "2026-03-23T07:55:22Z",
      "completed_at": null
    },
    {
      "number": 8,
      "name": "Run tests with coverage (tui)",
      "status": "pending",
      "started_at": null,
      "completed_at": null
    }
  ],
  "total_steps": 9
}
```

Returns 404 if no step data is available (runner not busy, no Worker log found).

**`GET /runners/{id}/steps/{step_number}/logs`** — Log content for a specific step

```json
{
  "step_number": 7,
  "step_name": "Run tests with coverage (daemon)",
  "lines": [
    "Run source \"$HOME/.cargo/env\" 2>/dev/null || true",
    "info: cargo-llvm-cov currently setting cfg(coverage)",
    "   Compiling proc-macro v1.0.106",
    "   Compiling unicode-ident v1.0.24"
  ]
}
```

- First call for a job fetches full job log from GitHub API, caches in memory
- Subsequent calls for other steps of the same job use cache
- Cache evicted 5 minutes after job completes
- For running steps, re-fetches every 5s to get new output
- Returns 404 if `job_id` not yet available (poller hasn't matched yet)
- Returns 503 if GitHub API call fails (with error message)

### 4. Tauri Desktop App

**New hook: `useJobSteps(runnerId)`**

- Polls `GET /runners/{id}/steps` every 1s when runner is busy
- Stops polling when runner state is not busy
- Returns `{ steps, totalSteps, jobName, loading }`

**New component: `JobProgress`**

- Renders step list matching the approved mockup design
- Step states: green checkmark (succeeded), red X (failed), yellow spinner (running), grey circle (pending)
- Active step auto-expanded, shows live elapsed timer
- Completed/failed steps clickable to expand — triggers `GET /runners/{id}/steps/{n}/logs`
- Step logs cached in component state per step number
- For running step, log content refreshes every 5s while expanded
- Duration shown: completed steps show final duration, running step shows live elapsed

**Placement:** Between the cards row and the existing Logs panel in `RunnerDetail.tsx`. Only visible when runner is busy (or briefly after job completion).

**Existing Logs panel:** Renamed header from "Logs" to "Runner Process Logs" for clarity. No other changes.

### 5. TUI

**New widget: `JobProgress`** in runner detail view

- Compact step list format:
  - `✓ Step Name ·················· 2s` (succeeded, green)
  - `✗ Step Name ·················· 5s` (failed, red)
  - `⟳ Step Name ················ 43s…` (running, yellow)
  - `○ Step Name` (pending, grey)
- Active step highlighted with yellow background
- Step selection via arrow keys, Enter to expand/collapse
- Expanded step shows log lines inline below
- Keybinding: `j` to toggle job progress panel visibility

## Step Log Content Parsing

The GitHub API returns the full job log as plain text with step boundaries marked by `##[group]Step Name` and `##[endgroup]`. To extract logs for a specific step:

1. Fetch full log from `GET /repos/{owner}/{repo}/actions/jobs/{job_id}/logs`
2. Parse sections between `##[group]` and `##[endgroup]` markers
3. Map sections to step numbers by order of appearance
4. Cache the parsed result keyed by `job_id`

## Not In Scope

- Showing steps for past/completed jobs (only current or most recent job)
- Workflow-level progress (multiple jobs) — this is per-runner, single-job view
- Local step log capture from `_diag/blocks/` (see issue #44)
- Step log search/filter (can be added later)
