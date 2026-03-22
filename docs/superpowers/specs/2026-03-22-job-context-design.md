# Job Context: Branch Name and PR Number for Running Jobs

**Issue:** #12
**Date:** 2026-03-22

## Problem

When a runner is busy executing a job, HomeRun shows only the job name (parsed from stdout). Users cannot see which branch or PR triggered the job without leaving the app.

## Solution

When a job starts, query the GitHub API for in-progress workflow runs, match by runner name, and store branch/PR info alongside the job. Display this context in both TUI and desktop app.

## Data Model

New struct in `crates/daemon/src/runner/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
}
```

Add to `RunnerInfo`:

```rust
pub job_context: Option<JobContext>,
```

Serialized with `#[serde(skip_serializing_if = "Option::is_none")]`. Set when job starts, cleared when job completes (same lifecycle as `current_job`).

## GitHub API

New method on `GitHubClient`:

```rust
pub async fn get_active_run_for_runner(
    &self, owner: &str, repo: &str, runner_name: &str
) -> Result<Option<JobContext>>
```

Steps:

1. `GET /repos/{owner}/{repo}/actions/runs?status=in_progress` to list active runs.
2. For each run, `GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs` to find which runner is executing.
3. Match job's `runner_name` field against our runner's name.
4. Extract `head_branch`, `pull_requests[0].number`, `pull_requests[0].html_url`, and run `html_url`.
5. Return first match or None.

## Runner Lifecycle

In `crates/daemon/src/runner/mod.rs`, when `JobEvent::Started` fires:

1. Transition to Busy and set `current_job` immediately (existing behavior).
2. Spawn a tokio task to call `get_active_run_for_runner`.
3. On success, update `runner.job_context` on the runner. Emit a `"job_context_updated"` event via WebSocket.
4. On failure, log a warning. Leave `job_context` as None. The feature degrades gracefully.

When `JobEvent::Completed` fires:

1. Clear `current_job` (existing behavior).
2. Clear `job_context`.

## TUI Changes

In `crates/tui/src/client.rs`, add to TUI's `RunnerInfo`:

```rust
pub current_job: Option<String>,
pub job_context: Option<JobContext>,
```

In `crates/tui/src/ui/runners.rs`, in the detail panel for busy runners, display:

```
Current Job: CI / Rust (fmt + clippy)
Branch: feat/my-feature (PR #11)
```

If `job_context` is None but runner is busy, show only the job name.

## Desktop App Changes

In `apps/desktop/src/api/types.ts`, add:

```typescript
export interface JobContext {
  branch: string;
  pr_number: number | null;
  pr_url: string | null;
  run_url: string;
}
```

Add to `RunnerInfo`:

```typescript
job_context?: JobContext | null;
```

In `RunnerTable.tsx`, update the Current Job column to show branch info below the job name.

In `RunnerDetail.tsx`, update the Current Job card to show branch and PR link.

## Scope Exclusions

- No caching of workflow run data.
- No periodic refresh — single fetch per job start.
- No retry on API failure — graceful degradation to showing job name only.
- No new tests for GitHub API calls (external dependency). Unit tests for data model serialization.
