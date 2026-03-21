# Process Tree Metrics Design

**Issue:** [#7 — Improve per-runner CPU/memory metrics](https://github.com/aGallea/homerun/issues/7)
**Date:** 2026-03-22

## Problem

`MetricsCollector::runner_metrics(pid)` queries sysinfo for only the spawned runner PID (the `run.sh` process). When a GitHub Actions job executes, it spawns child processes (Node.js, Docker, shell scripts, etc.) whose resource usage is invisible to the current metrics. This makes per-runner CPU/memory readings inaccurate during job execution — often showing near-zero while the machine is under heavy load.

## Solution

Walk the full process tree rooted at the runner PID and return aggregated CPU + memory across all descendant processes.

## Implementation

### Change: `MetricsCollector::runner_metrics()` in `crates/daemon/src/metrics.rs`

**Before:** Refreshes and queries a single PID via `ProcessesToUpdate::Some(&[pid])`.

**After:**

1. Refresh all processes with `ProcessesToUpdate::All` using `ProcessRefreshKind::nothing().with_cpu().with_memory()` to keep it lightweight.
2. Collect all PIDs in the process tree rooted at the runner PID:
   - Start with the runner PID in the result set.
   - Iterate all processes; if a process's `parent()` is already in the result set, add it.
   - Repeat until no new PIDs are added (handles arbitrary nesting depth).
3. Sum `cpu_usage()` and `memory()` across all matched PIDs.
4. Return the aggregated values in the existing `RunnerMetrics` struct.

### What does NOT change

- `RunnerMetrics` struct — same fields, same types.
- `/metrics` API response shape — clients see the same JSON.
- TUI and desktop app code — no modifications needed.
- `SystemMetrics` / `system_snapshot()` — unaffected.
- Runner PID tracking in `RunnerManager` — still stores the root PID.

## Testing

Add a test in `metrics.rs` that:

1. Spawns a parent process that itself spawns a child (e.g., `sh -c "sleep 60"` which creates sh → sleep).
2. Calls `runner_metrics()` with the parent PID.
3. Asserts that the returned memory includes contributions from the child process (memory > 0 and the process tree was walked).
4. Cleans up spawned processes.

## Performance

- `ProcessesToUpdate::All` with only CPU+memory refresh kinds is lightweight — sysinfo skips disk, network, and other expensive refreshes.
- At the current 10-second polling interval, the overhead is negligible.
- No caching needed; process trees change rapidly during jobs.
