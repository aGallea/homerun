# Runner Grouping for Batch Actions

**Issue:** #22 — Group multi-instance runners for batch actions
**Date:** 2026-03-22

## Summary

When creating multiple runner instances in a single batch, they should be displayed as a group in the UI, allowing bulk actions (start, stop, restart, delete) on all instances at once while still permitting individual runner control.

## Design Decisions

- **Explicit `group_id`** on `RunnerConfig` (not convention-based or separate table)
- **Server-generated** via a new `POST /runners/batch` endpoint (not client-side loop)
- **Solo runners** (count=1) have no `group_id` and display as flat rows (no group wrapper)
- **Both UIs** — desktop app and TUI get group display and batch actions

## Data Model

### `RunnerConfig` (types.rs)

Add one field:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub group_id: Option<String>,
```

`None` for solo runners, `Some(uuid)` for batch-created runners. Backward compatible with existing `runners.json` files.

### New `CreateBatchRequest` (types.rs)

```rust
#[derive(Debug, Deserialize)]
pub struct CreateBatchRequest {
    pub repo_full_name: String,
    pub count: u16,                        // 2-10
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}
```

No `name` field — names are auto-generated server-side (`{repo}-runner-1` through `{repo}-runner-{count}`). No `group_id` field — server generates it.

### New `BatchCreateResponse`

```rust
#[derive(Debug, Serialize)]
pub struct BatchCreateResponse {
    pub group_id: String,
    pub runners: Vec<RunnerInfo>,
}
```

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

## API Changes

### New Endpoints

| Method   | Endpoint                             | Description                                    |
| -------- | ------------------------------------ | ---------------------------------------------- |
| `POST`   | `/runners/batch`                     | Create multiple runners with shared `group_id` |
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

- `POST /runners/batch`: Generates a single `group_id` UUID. Creates `count` runners in a loop, each with that `group_id`. Auto-generates names as `{repo}-runner-{i}`. Spawns async registration for each. Returns all created runners + the `group_id`.
- Group action endpoints: Iterate runners matching `group_id`, apply the action to each runner in a valid state for that transition, skip others. Return per-runner results. Partial success is expected and normal.

### `RunnerManager` Changes

- `create()` gains an optional `group_id: Option<String>` parameter
- New `create_batch()` method: generates UUID, calls `create()` N times with that `group_id`
- New `group_action()` helper or individual methods for start/stop/restart/delete by group

## Desktop App (React)

### RunnerTable

- Runners with a `group_id` are collected into collapsible group rows
- Solo runners (no `group_id`) render as flat rows, same as today
- **Group row shows:** collapse chevron, group name (derived from first runner's name prefix), instance count, aggregated status summary (e.g. `2 online, 1 busy`), batch action buttons (start all, stop all, restart all, delete all)
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

- `s` start/stop all, `r` restart all, `d` delete all (with confirmation)
- Same keybindings as individual runners, applied in batch

### Actions on Individual Runners Within a Group

- Same as today — `s`, `r`, `d`, `l`, `e` operate on the single selected runner

## Testing

### Daemon (Rust)

- `RunnerConfig` serialization with/without `group_id` (backward compat)
- `POST /runners/batch` — validates `group_id` assignment, correct count, auto-naming
- Group action endpoints — start/stop/restart/delete with partial success scenarios
- `GET /runners?group_id=...` filtering
- `POST /runners` (single) still produces `group_id: None`

### TUI (Rust)

- Group expansion/collapse state management
- Batch action dispatch
