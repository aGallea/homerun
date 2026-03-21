# Process Tree Metrics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make per-runner CPU/memory metrics aggregate the full process tree, not just the root PID.

**Architecture:** Replace the single-PID sysinfo query in `MetricsCollector::runner_metrics()` with a full process list refresh + parent-chain walk to collect all descendant PIDs, then sum their CPU and memory.

**Tech Stack:** Rust, sysinfo 0.33, std::collections::HashSet

**Spec:** `docs/superpowers/specs/2026-03-22-process-tree-metrics-design.md`

---

## Task 1: Write tests for process tree aggregation

**Files:**

- Modify: `crates/daemon/src/metrics.rs` (test module, line 104+)

- [ ] **Step 1: Write tests for process tree metrics**

Add these tests to the `#[cfg(test)] mod tests` block in `metrics.rs`:

```rust
#[test]
fn test_runner_metrics_includes_child_processes() {
    use std::process::Command;

    // Spawn a shell that itself spawns children — creates a process tree.
    // Use short sleep durations since we kill the process group after.
    let mut parent = Command::new("sh")
        .arg("-c")
        .arg("sleep 10 & sleep 10 & sleep 10 & wait")
        .spawn()
        .expect("failed to spawn parent process");

    let parent_pid = parent.id();

    // Give children time to spawn
    std::thread::sleep(std::time::Duration::from_millis(500));

    let collector = MetricsCollector::new();
    let metrics = collector.runner_metrics(parent_pid);

    // Kill parent and orphaned children via pkill
    let _ = std::process::Command::new("pkill")
        .args(["-P", &parent_pid.to_string()])
        .status();
    let _ = parent.kill();
    let _ = parent.wait();

    let metrics = metrics.expect("should find the parent process");
    // The aggregated memory should be > 0 (includes parent + children)
    assert!(metrics.memory_bytes > 0, "aggregated memory should be > 0");
}
```

- [ ] **Step 2: Run tests (they will fail — `runner_metrics` doesn't walk the tree yet)**

Run: `cargo test -p homerund test_runner_metrics_includes_child -- --nocapture`

Expected: Test may pass with just parent memory > 0, but the implementation doesn't yet aggregate children. The tree walk is the behavioral change in Task 2.

## Task 2: Implement process tree walking in `runner_metrics()`

**Files:**

- Modify: `crates/daemon/src/metrics.rs:1` (change `use std::collections::VecDeque;` to include `HashSet`)
- Modify: `crates/daemon/src/metrics.rs:82-95` (rewrite `runner_metrics()`)

- [ ] **Step 1: Add HashSet import**

Change line 1 from `use std::collections::VecDeque;` to:

```rust
use std::collections::{HashSet, VecDeque};
```

- [ ] **Step 2: Rewrite `runner_metrics()` to walk process tree**

Replace the `runner_metrics` method (lines 82-95) with:

```rust
pub fn runner_metrics(&self, pid: u32) -> Option<RunnerMetrics> {
    let mut sys = self.system.lock().unwrap();
    let root_pid = Pid::from_u32(pid);

    // Refresh all processes (CPU + memory only) so children are visible
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );

    // Check the root process exists
    if sys.process(root_pid).is_none() {
        return None;
    }

    // Collect all PIDs in the tree rooted at root_pid
    let mut tree_pids = HashSet::new();
    tree_pids.insert(root_pid);

    // Iterate until no new children are found
    loop {
        let mut found_new = false;
        for (pid, process) in sys.processes() {
            if !tree_pids.contains(pid) {
                if let Some(parent) = process.parent() {
                    if tree_pids.contains(&parent) {
                        tree_pids.insert(*pid);
                        found_new = true;
                    }
                }
            }
        }
        if !found_new {
            break;
        }
    }

    // Aggregate CPU and memory across the tree
    let mut total_cpu: f64 = 0.0;
    let mut total_memory: u64 = 0;
    for pid in &tree_pids {
        if let Some(process) = sys.process(*pid) {
            total_cpu += process.cpu_usage() as f64;
            total_memory += process.memory();
        }
    }

    Some(RunnerMetrics {
        runner_id: String::new(),
        cpu_percent: total_cpu,
        memory_bytes: total_memory,
    })
}
```

- [ ] **Step 3: Run all tests to verify everything passes**

Run: `cargo test -p homerund -- --nocapture`

Expected: All tests PASS including the two new process tree tests.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p homerund --all-targets --all-features -- -D warnings`

Expected: No warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/metrics.rs
git commit -m "feat(daemon): aggregate process tree for runner metrics

Walk the full process tree rooted at the runner PID to sum CPU and
memory across all child processes. Fixes #7."
```

## Task 3: Verify existing API and integration tests still pass

**Files:**

- No changes — verification only

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: All tests PASS. The API endpoint tests in `api/metrics.rs` should pass unchanged since the response shape is identical.

- [ ] **Step 2: Run TypeScript type check**

Run: `cd apps/desktop && npx tsc --noEmit`

Expected: No errors (no frontend changes needed).

- [ ] **Step 3: Run formatting check**

Run: `cargo fmt --check`

Expected: No formatting issues.
