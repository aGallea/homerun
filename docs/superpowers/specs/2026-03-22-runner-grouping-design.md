# Runner Grouping for Batch Actions

**Issue:** #22 — Group multi-instance runners for batch actions
**Date:** 2026-03-22

## Summary

When creating multiple runner instances in a single batch, they should be displayed as a group in the UI, allowing bulk actions (start, stop, restart, delete, scale) on all instances at once while still permitting individual runner control.

## Design Decisions

- **Explicit `group_id`** on `RunnerConfig` (not convention-based or separate table)
- **Server-generated** via a new `POST /runners/batch` endpoint (not client-side loop)
- **Solo runners** (count=1) have no `group_id` and display as flat rows (no group wrapper)
- **Both UIs** — desktop app and TUI get group display and batch actions
- **Declarative scaling** — `PATCH /runners/groups/{group_id}` with target `count` to scale up or down
- **Scale-down strategy** — remove highest-numbered runners first, skip busy runners

## Data Model

### `RunnerConfig` (types.rs)

Add one field:

```rust
#[serde(skip_serializing_if = "Option::is_none", default)]
pub group_id: Option<String>,
```

`None` for solo runners, `Some(uuid)` for batch-created runners. Backward compatible — deserializing existing `runners.json` files without this field produces `None` via the `default` serde attribute.

Note: `group_id` is accessible in API responses via the nested path `config.group_id` on `RunnerInfo`.

### New `CreateBatchRequest` (types.rs)

```rust
#[derive(Debug, Deserialize)]
pub struct CreateBatchRequest {
    pub repo_full_name: String,
    pub count: u8,                         // Validated: 2-10
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}
```

No `name` field — names are auto-generated server-side using a repo-scoped monotonic counter (see Naming Strategy below). No `group_id` field — server generates it.

The endpoint validates `count` is in the range 2-10 and returns 400 Bad Request otherwise.

### New `BatchCreateResponse`

```rust
#[derive(Debug, Serialize)]
pub struct BatchCreateResponse {
    pub group_id: String,
    pub runners: Vec<RunnerInfo>,
    pub errors: Vec<BatchCreateError>,
}

#[derive(Debug, Serialize)]
pub struct BatchCreateError {
    pub index: u8,
    pub error: String,
}
```

Uses the same partial-success pattern as group actions. If some runners fail to create (e.g. disk full), the successfully created runners are kept with their `group_id`, and errors are reported per-index. HTTP status is 201 if all succeed, 207 (Multi-Status) if partial.

### New `GroupActionResponse`

```rust
#[derive(Debug, Serialize)]
pub struct GroupActionResult {
    pub runner_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GroupActionResponse {
    pub group_id: String,
    pub results: Vec<GroupActionResult>,
}
```

### New `ScaleGroupRequest`

```rust
#[derive(Debug, Deserialize)]
pub struct ScaleGroupRequest {
    pub count: u8,                         // Validated: 1-10
}
```

### New `ScaleGroupResponse`

```rust
#[derive(Debug, Serialize)]
pub struct ScaleGroupResponse {
    pub group_id: String,
    pub previous_count: u8,
    pub target_count: u8,
    pub actual_count: u8,                  // May differ from target if busy runners blocked removal
    pub added: Vec<RunnerInfo>,
    pub removed: Vec<String>,              // Runner IDs that were removed
    pub skipped_busy: Vec<String>,         // Runner IDs that couldn't be removed (busy)
}
```

## Naming Strategy

Runner names within a group use a **repo-scoped monotonic counter** to avoid collisions across batches. The daemon tracks the highest runner number ever assigned for each repo (derived from existing runners at startup). For example:

- First batch for `myrepo`: `myrepo-runner-1`, `myrepo-runner-2`, `myrepo-runner-3`
- Second batch for `myrepo`: `myrepo-runner-4`, `myrepo-runner-5`
- Scale-up of first batch: `myrepo-runner-6` (continues from global counter, not from group)

This avoids name collisions without requiring complex uniqueness checks.

## API Changes

### New Endpoints

| Method   | Endpoint                             | Description                                    |
| -------- | ------------------------------------ | ---------------------------------------------- |
| `POST`   | `/runners/batch`                     | Create multiple runners with shared `group_id` |
| `PATCH`  | `/runners/groups/{group_id}`         | Scale group to target count                    |
| `POST`   | `/runners/groups/{group_id}/start`   | Start all startable runners in group           |
| `POST`   | `/runners/groups/{group_id}/stop`    | Stop all stoppable runners in group            |
| `POST`   | `/runners/groups/{group_id}/restart` | Restart all runners in group                   |
| `DELETE` | `/runners/groups/{group_id}`         | Delete all runners in group                    |

### Modified Endpoints

- `GET /runners` gains optional `?group_id=...` query parameter to filter by group

### Unchanged

- `POST /runners` — single runner creation, produces `group_id: None`
- All individual runner endpoints (`/runners/{id}/start`, etc.) — still work for individual control

### Batch Endpoint Behavior

- `POST /runners/batch`: Generates a single `group_id` UUID. Creates `count` runners, each with that `group_id`. Auto-generates names using the repo-scoped monotonic counter. Spawns async registration for each. Returns created runners, `group_id`, and any per-index errors. HTTP 201 if all succeed, 207 if partial.
- Group action endpoints: Iterate runners matching `group_id`, apply the action to each runner in a valid state for that transition, skip others. Return per-runner results. Partial success is expected and normal.

### Scale Endpoint Behavior

`PATCH /runners/groups/{group_id}` with `{ "count": N }`:

- **Scale up** (target > current): Creates `target - current` new runners with the same `group_id`, repo, labels, and mode as existing group members. Names continue from the repo-scoped counter.
- **Scale down** (target < current): Removes runners from highest-numbered name first. **Busy runners are skipped** — they cannot be removed. If skipping busy runners means the target count can't be reached, the response reports the actual count achieved and lists skipped runners. Removed runners go through the same delete flow as `DELETE /runners/{id}`.
- **No change** (target == current): Returns 200 with no additions or removals.
- **Validation**: `count` must be 1-10. Returns 400 if out of range. Scaling to 1 is allowed (leaves one runner, group still exists).

### Group Discovery

UIs derive groups client-side from the `GET /runners` response by collecting distinct `group_id` values and grouping runners accordingly. No separate group listing endpoint is needed.

### WebSocket Events

Real-time events via `/events` continue to fire per-runner as today. UIs correlate group-level changes by matching incoming `runner_id` values against their local group mapping (built from `config.group_id`). No new group-level event type is needed.

### `RunnerManager` Changes

- `create()` gains an optional `group_id: Option<String>` parameter
- New `create_batch()` method: generates UUID, calls `create()` N times with that `group_id`
- New `scale_group()` method: compares target vs current, adds or removes runners
- New `group_action()` helper or individual methods for start/stop/restart/delete by group
- Track repo-scoped runner name counter (initialized from existing runners at startup)

## Desktop App (React)

### RunnerTable

- Runners with a `group_id` are collected into collapsible group rows
- Solo runners (no `group_id`) render as flat rows, same as today
- **Group row shows:** collapse chevron, group name (derived from shared name prefix), instance count, aggregated status summary (e.g. `2 online, 1 busy`), batch action buttons (start all, stop all, restart all, delete all), and a scale control (+/- buttons or input to set target count)
- **Expanded state:** individual runner rows indented below the group row, each with their own actions
- Groups start **collapsed** by default
- Delete all triggers a confirmation dialog showing the count

### NewRunnerWizard Refactor

- When count > 1: calls `POST /runners/batch` instead of client-side loop
- Remove client-side `generateBatchName()` and batch loop logic
- `BatchSummary` reads from the single batch response
- When count === 1: still calls `POST /runners` (no `group_id`)

### Filtering

- Search filter applies to both group names and individual runner names
- If a runner inside a collapsed group matches the filter, the group auto-expands

## TUI (Ratatui)

### Runner List

- Group rows: `▶ myrepo-runner (3)  2● 1●` (colored dots for status summary)
- Enter/Right on group row expands; Enter/Left collapses
- Expanded shows individual runners with tree markers (`├─` / `└─`)
- j/k navigation moves through both group rows and visible individual runners

### Actions on Group Rows

- `s` on a group row applies start to all startable runners (Offline/Error) AND stop to all stoppable runners (Online/Busy) — it normalizes the group toward a toggled state
- `r` restart all, `d` delete all (with confirmation)
- `+` / `-` scale up / scale down by 1

### Actions on Individual Runners Within a Group

- Same as today — `s`, `r`, `d`, `l`, `e` operate on the single selected runner

## Testing

### Daemon (Rust)

- `RunnerConfig` serialization with/without `group_id` (backward compat: old JSON without field deserializes to `None`)
- `POST /runners/batch` — validates `group_id` assignment, correct count, auto-naming with monotonic counter
- `POST /runners/batch` — partial failure returns 207 with errors and successfully created runners
- `POST /runners/batch` — rejects count outside 2-10 range with 400
- Group action endpoints — start/stop/restart/delete with partial success scenarios
- `PATCH /runners/groups/{group_id}` — scale up adds runners with correct `group_id` and naming
- `PATCH /runners/groups/{group_id}` — scale down removes highest-numbered first, skips busy
- `PATCH /runners/groups/{group_id}` — scale down reports skipped busy runners in response
- `GET /runners?group_id=...` filtering
- `POST /runners` (single) still produces `group_id: None`
- Name counter survives daemon restart (derived from existing runners)

### TUI (Rust)

- Group expansion/collapse state management
- Batch action dispatch
- Scale up/down via keybindings
