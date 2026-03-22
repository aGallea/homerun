# Runner Grouping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group multi-instance runners for batch actions, enabling collapsible group display and bulk start/stop/restart/delete/scale across all three UIs.

**Architecture:** Add `group_id: Option<String>` to `RunnerConfig`, a new `POST /runners/batch` endpoint that creates N runners with a shared group_id, group action endpoints, and a declarative `PATCH` scaling endpoint. UIs derive groups client-side from the runner list and render collapsible group rows.

**Tech Stack:** Rust (Axum, Ratatui, serde), React 19 + TypeScript (Tauri desktop app)

**Spec:** `docs/superpowers/specs/2026-03-22-runner-grouping-design.md`

---

## File Map

### Daemon (Rust)

| File                                | Action | Responsibility                                                                                                |
| ----------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------- |
| `crates/daemon/src/runner/types.rs` | Modify | Add `group_id` to `RunnerConfig`, new request/response structs                                                |
| `crates/daemon/src/runner/mod.rs`   | Modify | Add `group_id` param to `create()`, new `create_batch()`, `scale_group()`, group action methods, name counter |
| `crates/daemon/src/api/runners.rs`  | Modify | Add `group_id` query param to `list_runners`                                                                  |
| `crates/daemon/src/api/groups.rs`   | Create | New group endpoints: batch create, start/stop/restart/delete group, scale                                     |
| `crates/daemon/src/api/mod.rs`      | Modify | Add `pub mod groups;`                                                                                         |
| `crates/daemon/src/server.rs`       | Modify | Register new group routes                                                                                     |

### TUI (Rust)

| File                           | Action | Responsibility                                                            |
| ------------------------------ | ------ | ------------------------------------------------------------------------- |
| `crates/tui/src/client.rs`     | Modify | Add `group_id` to `RunnerConfig`, new API methods for batch/group actions |
| `crates/tui/src/app.rs`        | Modify | Add group expand/collapse state, new group actions, navigation logic      |
| `crates/tui/src/ui/runners.rs` | Modify | Render group rows with expand/collapse, tree markers, status summary      |

### Desktop App (TypeScript)

| File                                              | Action | Responsibility                                          |
| ------------------------------------------------- | ------ | ------------------------------------------------------- |
| `apps/desktop/src/api/types.ts`                   | Modify | Add `group_id` to `RunnerConfig`, new batch/group types |
| `apps/desktop/src/api/commands.ts`                | Modify | Add batch create, group action, scale API calls         |
| `apps/desktop/src/hooks/useRunners.ts`            | Modify | Add batch create, group actions, scale methods          |
| `apps/desktop/src/components/RunnerTable.tsx`     | Modify | Render collapsible group rows with batch actions        |
| `apps/desktop/src/components/RunnerGroupRow.tsx`  | Create | Group row component with chevron, summary, actions      |
| `apps/desktop/src/components/NewRunnerWizard.tsx` | Modify | Use `POST /runners/batch` for count > 1                 |

### Tauri Backend

| File                                     | Action | Responsibility                                    |
| ---------------------------------------- | ------ | ------------------------------------------------- |
| `apps/desktop/src-tauri/src/client.rs`   | Modify | Add `group_id` to `RunnerConfig`, new API methods |
| `apps/desktop/src-tauri/src/commands.rs` | Modify | Add Tauri IPC commands for batch/group operations |
| `apps/desktop/src-tauri/src/lib.rs`      | Modify | Register new commands                             |

---

## Task 1: Add `group_id` to data model and types

**Files:**

- Modify: `crates/daemon/src/runner/types.rs`
- Test: `crates/daemon/src/runner/types.rs` (inline tests) and `crates/daemon/src/api/runners.rs` (existing tests)

- [ ] **Step 1: Write test for RunnerConfig backward compatibility**

In `crates/daemon/src/runner/types.rs`, add a `#[cfg(test)]` module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_deserialize_without_group_id() {
        let json = r#"{
            "id": "abc-123",
            "name": "test-runner-1",
            "repo_owner": "owner",
            "repo_name": "repo",
            "labels": ["self-hosted"],
            "mode": "app",
            "work_dir": "/tmp/runners/abc-123"
        }"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.group_id, None);
    }

    #[test]
    fn test_runner_config_deserialize_with_group_id() {
        let json = r#"{
            "id": "abc-123",
            "name": "test-runner-1",
            "repo_owner": "owner",
            "repo_name": "repo",
            "labels": ["self-hosted"],
            "mode": "app",
            "work_dir": "/tmp/runners/abc-123",
            "group_id": "group-uuid-456"
        }"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.group_id, Some("group-uuid-456".to_string()));
    }

    #[test]
    fn test_runner_config_serialize_without_group_id_omits_field() {
        let config = RunnerConfig {
            id: "abc".to_string(),
            name: "test".to_string(),
            repo_owner: "owner".to_string(),
            repo_name: "repo".to_string(),
            labels: vec![],
            mode: RunnerMode::App,
            work_dir: std::path::PathBuf::from("/tmp"),
            group_id: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("group_id"));
    }

    #[test]
    fn test_create_batch_request_rejects_count_below_2() {
        let json = r#"{"repo_full_name":"owner/repo","count":1}"#;
        let req: CreateBatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.count, 1); // Deserialization succeeds; validation is at endpoint level
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund -- types::tests --nocapture`
Expected: FAIL — `RunnerConfig` has no `group_id` field, new structs don't exist

- [ ] **Step 3: Add group_id to RunnerConfig and new types**

In `crates/daemon/src/runner/types.rs`, add `group_id` to `RunnerConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub id: String,
    pub name: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub labels: Vec<String>,
    pub mode: RunnerMode,
    pub work_dir: std::path::PathBuf,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub group_id: Option<String>,
}
```

Add the new request/response types after `UpdateRunnerRequest`:

```rust
#[derive(Debug, Deserialize)]
pub struct CreateBatchRequest {
    pub repo_full_name: String,
    pub count: u8,
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}

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

#[derive(Debug, Deserialize)]
pub struct ScaleGroupRequest {
    pub count: u8,
}

#[derive(Debug, Serialize)]
pub struct ScaleGroupResponse {
    pub group_id: String,
    pub previous_count: u8,
    pub target_count: u8,
    pub actual_count: u8,
    pub added: Vec<RunnerInfo>,
    pub removed: Vec<String>,
    pub skipped_busy: Vec<String>,
}
```

- [ ] **Step 4: Fix existing code that constructs RunnerConfig**

In `crates/daemon/src/runner/mod.rs`, find the `RunnerConfig` construction in `create()` (~line 210) and add `group_id: None`:

```rust
config: RunnerConfig {
    id: id.clone(),
    name,
    repo_owner: owner.to_string(),
    repo_name: repo.to_string(),
    labels: default_labels,
    mode: mode.unwrap_or(RunnerMode::App),
    work_dir,
    group_id: None,
},
```

- [ ] **Step 5: Run all tests to verify everything passes**

Run: `cargo test -p homerund --nocapture`
Expected: All tests PASS including the new type tests and all existing tests

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/types.rs crates/daemon/src/runner/mod.rs
git commit -m "feat: add group_id to RunnerConfig and batch/group types"
```

---

## Task 2: Add `group_id` parameter to `RunnerManager::create()` and name counter

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs`
- Test: `crates/daemon/src/runner/mod.rs` (inline tests)

- [ ] **Step 1: Write tests for create with group_id and name counter**

Add to the existing `#[cfg(test)] mod tests` in `crates/daemon/src/runner/mod.rs`:

```rust
fn create_test_manager() -> RunnerManager {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.path().join(".homerun"));
    config.ensure_dirs().unwrap();
    RunnerManager::new(config)
}

#[tokio::test]
async fn test_create_with_group_id() {
    let manager = create_test_manager();
    let runner = manager
        .create("owner/repo", Some("test-runner".to_string()), None, None, Some("group-123".to_string()))
        .await
        .unwrap();
    assert_eq!(runner.config.group_id, Some("group-123".to_string()));
}

#[tokio::test]
async fn test_create_without_group_id() {
    let manager = create_test_manager();
    let runner = manager
        .create("owner/repo", Some("test-runner".to_string()), None, None, None)
        .await
        .unwrap();
    assert_eq!(runner.config.group_id, None);
}

#[tokio::test]
async fn test_next_runner_number_increments() {
    let manager = create_test_manager();
    let r1 = manager.create("owner/myrepo", None, None, None, None).await.unwrap();
    let r2 = manager.create("owner/myrepo", None, None, None, None).await.unwrap();
    assert_eq!(r1.config.name, "myrepo-runner-1");
    assert_eq!(r2.config.name, "myrepo-runner-2");
}

#[tokio::test]
async fn test_next_runner_number_different_repos() {
    let manager = create_test_manager();
    let r1 = manager.create("owner/repo-a", None, None, None, None).await.unwrap();
    let r2 = manager.create("owner/repo-b", None, None, None, None).await.unwrap();
    assert_eq!(r1.config.name, "repo-a-runner-1");
    assert_eq!(r2.config.name, "repo-b-runner-1");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- test_create_with_group_id test_create_without_group_id test_next_runner_number --nocapture`
Expected: FAIL — `create()` signature doesn't match

- [ ] **Step 3: Add group_id param and repo-scoped name counter**

In `crates/daemon/src/runner/mod.rs`:

1. Add a name counter field to `RunnerManager`:

```rust
pub struct RunnerManager {
    config: Arc<Config>,
    runners: Arc<RwLock<HashMap<String, RunnerInfo>>>,
    processes: Arc<RwLock<HashMap<String, Arc<RwLock<Child>>>>>,
    log_tx: Arc<broadcast::Sender<LogEntry>>,
    event_tx: Arc<broadcast::Sender<RunnerEvent>>,
    recent_logs: Arc<RwLock<HashMap<String, VecDeque<LogEntry>>>>,
    name_counters: Arc<RwLock<HashMap<String, u32>>>,
}
```

1. Initialize it in `new()`:

```rust
name_counters: Arc::new(RwLock::new(HashMap::new())),
```

1. Add a helper method to get the next runner number:

```rust
async fn next_runner_number(&self, repo_name: &str) -> u32 {
    let mut counters = self.name_counters.write().await;
    let counter = counters.entry(repo_name.to_string()).or_insert(0);
    *counter += 1;
    *counter
}
```

1. Initialize counters in `load_from_disk()` by parsing existing runner names:

After loading configs, add:

```rust
// Initialize name counters from loaded runners
let mut counters = self.name_counters.write().await;
for info in runners.values() {
    let repo = &info.config.repo_name;
    // Parse "{repo}-runner-{N}" pattern
    let prefix = format!("{}-runner-", repo);
    if let Some(num_str) = info.config.name.strip_prefix(&prefix) {
        if let Ok(num) = num_str.parse::<u32>() {
            let entry = counters.entry(repo.clone()).or_insert(0);
            *entry = (*entry).max(num);
        }
    }
}
```

1. Update `create()` signature and body:

```rust
pub async fn create(
    &self,
    repo_full_name: &str,
    name: Option<String>,
    labels: Option<Vec<String>>,
    mode: Option<RunnerMode>,
    group_id: Option<String>,
) -> Result<RunnerInfo> {
```

Replace the old count-based name generation with:

```rust
let name = match name {
    Some(n) => n,
    None => {
        let num = self.next_runner_number(repo).await;
        format!("{repo}-runner-{num}")
    }
};
```

And set `group_id` on the config:

```rust
group_id,
```

- [ ] **Step 4: Update all callers of `create()`**

In `crates/daemon/src/api/runners.rs`, update `create_runner()`:

```rust
let runner = state
    .runner_manager
    .create(&req.repo_full_name, req.name, req.labels, req.mode, None)
    .await
```

Update all existing tests that call `create()` with 4 args to pass `None` as the 5th parameter. These are in `crates/daemon/src/runner/mod.rs` tests:

- `test_create_runner_generates_id_and_name` (calls `create("aGallea/gifted", None, None, None)`)
- `test_list_runners` (2 calls)
- `test_delete_runner`
- `test_state_transitions`
- `test_create_runner_with_labels_no_duplicate_macOS`
- `test_save_and_load_from_disk`

Also refactor the existing boilerplate in those tests to use the new `create_test_manager()` helper added in Step 1.

- [ ] **Step 5: Run all tests**

Run: `cargo test -p homerund --nocapture`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/mod.rs crates/daemon/src/api/runners.rs
git commit -m "feat: add group_id parameter to create() and repo-scoped name counter"
```

---

## Task 3: Implement `create_batch()` and batch endpoint

**Files:**

- Create: `crates/daemon/src/api/groups.rs`
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/runner/mod.rs`
- Modify: `crates/daemon/src/server.rs`
- Test: `crates/daemon/src/api/groups.rs` (inline tests)

- [ ] **Step 1: Write tests for batch create endpoint**

Create `crates/daemon/src/api/groups.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_batch_create_returns_group_id_and_runners() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/myrepo","count":3}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(resp["group_id"].is_string());
        assert_eq!(resp["runners"].as_array().unwrap().len(), 3);
        assert_eq!(resp["errors"].as_array().unwrap().len(), 0);

        // All runners share the same group_id
        let gid = resp["group_id"].as_str().unwrap();
        for runner in resp["runners"].as_array().unwrap() {
            assert_eq!(runner["config"]["group_id"].as_str().unwrap(), gid);
        }
    }

    #[tokio::test]
    async fn test_batch_create_auto_names_with_counter() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/myrepo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let names: Vec<&str> = resp["runners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["config"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["myrepo-runner-1", "myrepo-runner-2"]);
    }

    #[tokio::test]
    async fn test_batch_create_rejects_count_below_2() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_create_rejects_count_above_10() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":11}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- groups::tests --nocapture`
Expected: FAIL — module doesn't exist yet

- [ ] **Step 3: Implement create_batch on RunnerManager**

In `crates/daemon/src/runner/mod.rs`, add:

```rust
pub async fn create_batch(
    &self,
    repo_full_name: &str,
    count: u8,
    labels: Option<Vec<String>>,
    mode: Option<RunnerMode>,
) -> Result<(String, Vec<RunnerInfo>, Vec<types::BatchCreateError>)> {
    let group_id = uuid::Uuid::new_v4().to_string();
    let mut runners = Vec::new();
    let mut errors = Vec::new();

    for i in 0..count {
        match self
            .create(repo_full_name, None, labels.clone(), mode.clone(), Some(group_id.clone()))
            .await
        {
            Ok(runner) => runners.push(runner),
            Err(e) => errors.push(types::BatchCreateError {
                index: i,
                error: e.to_string(),
            }),
        }
    }

    Ok((group_id, runners, errors))
}
```

Also add a helper to list runners by group:

```rust
pub async fn list_by_group(&self, group_id: &str) -> Vec<RunnerInfo> {
    self.runners
        .read()
        .await
        .values()
        .filter(|r| r.config.group_id.as_deref() == Some(group_id))
        .cloned()
        .map(Self::with_computed_uptime)
        .collect()
}
```

- [ ] **Step 4: Implement batch create endpoint**

Create `crates/daemon/src/api/groups.rs`:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::types::{
    BatchCreateResponse, CreateBatchRequest, GroupActionResponse, GroupActionResult,
    ScaleGroupRequest, ScaleGroupResponse,
};
use crate::server::AppState;

pub async fn create_batch(
    State(state): State<AppState>,
    Json(req): Json<CreateBatchRequest>,
) -> Result<(StatusCode, Json<BatchCreateResponse>), (StatusCode, String)> {
    if req.count < 2 || req.count > 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            "count must be between 2 and 10".to_string(),
        ));
    }

    let (group_id, runners, errors) = state
        .runner_manager
        .create_batch(&req.repo_full_name, req.count, req.labels, req.mode)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Spawn background registration for each runner
    for runner in &runners {
        let manager = state.runner_manager.clone();
        let auth = state.auth.clone();
        let runner_id = runner.config.id.clone();
        tokio::spawn(async move {
            let token = match auth.token().await {
                Some(t) => t,
                None => {
                    tracing::error!("No auth token available for runner registration");
                    let _ = manager
                        .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                        .await;
                    return;
                }
            };
            if let Err(e) = manager.register_and_start(&runner_id, &token).await {
                tracing::error!("Failed to register runner {}: {}", runner_id, e);
                let _ = manager
                    .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                    .await;
            }
        });
    }

    let status = if errors.is_empty() {
        StatusCode::CREATED
    } else {
        StatusCode::MULTI_STATUS
    };

    Ok((status, Json(BatchCreateResponse { group_id, runners, errors })))
}
```

- [ ] **Step 5: Register the module and route**

In `crates/daemon/src/api/mod.rs`, add:

```rust
pub mod groups;
```

In `crates/daemon/src/server.rs`, add the route:

```rust
.route("/runners/batch", post(api::groups::create_batch))
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p homerund --nocapture`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/api/groups.rs crates/daemon/src/api/mod.rs crates/daemon/src/runner/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add POST /runners/batch endpoint for batch runner creation"
```

---

## Task 4: Implement group action endpoints (start/stop/restart/delete)

**Files:**

- Modify: `crates/daemon/src/api/groups.rs`
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Write tests for group action endpoints**

Add to `crates/daemon/src/api/groups.rs` tests module:

```rust
#[tokio::test]
async fn test_group_start_returns_results() {
    let state = AppState::new_test();

    // Create a batch
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = batch["group_id"].as_str().unwrap();

    // Try to start the group (runners are in Creating state, so all should fail with conflict)
    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/runners/groups/{group_id}/start"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(resp["group_id"].as_str().unwrap(), group_id);
    assert_eq!(resp["results"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_group_action_404_for_nonexistent_group() {
    let state = AppState::new_test();
    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/groups/nonexistent-group/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_group_delete_removes_runners() {
    let state = AppState::new_test();

    // Create a batch
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = batch["group_id"].as_str().unwrap();

    // Delete the group
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/runners/groups/{group_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Verify runners are gone
    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/runners")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(runners.len(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- groups::tests --nocapture`
Expected: FAIL — group action handlers don't exist

- [ ] **Step 3: Implement group action handlers**

In `crates/daemon/src/api/groups.rs`, add the handler functions:

```rust
pub async fn start_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("Group '{group_id}' not found")));
    }

    let mut results = Vec::new();
    for runner in &runners {
        let can_start = runner.state == crate::runner::state::RunnerState::Offline
            || runner.state == crate::runner::state::RunnerState::Error;
        if can_start {
            let manager = state.runner_manager.clone();
            let auth = state.auth.clone();
            let rid = runner.config.id.clone();
            tokio::spawn(async move {
                let token = match auth.token().await {
                    Some(t) => t,
                    None => return,
                };
                let _ = manager.update_state(&rid, crate::runner::state::RunnerState::Registering).await;
                let _ = manager.register_and_start_from_registering(&rid, &token).await;
            });
            results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: true, error: None });
        } else {
            results.push(GroupActionResult {
                runner_id: runner.config.id.clone(),
                success: false,
                error: Some(format!("Runner is in {:?} state, cannot start", runner.state)),
            });
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn stop_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("Group '{group_id}' not found")));
    }

    let mut results = Vec::new();
    for runner in &runners {
        let can_stop = runner.state == crate::runner::state::RunnerState::Online
            || runner.state == crate::runner::state::RunnerState::Busy;
        if can_stop {
            match state.runner_manager.stop_process(&runner.config.id).await {
                Ok(_) => results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: true, error: None }),
                Err(e) => results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: false, error: Some(e.to_string()) }),
            }
        } else {
            results.push(GroupActionResult {
                runner_id: runner.config.id.clone(),
                success: false,
                error: Some(format!("Runner is in {:?} state, cannot stop", runner.state)),
            });
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn restart_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("Group '{group_id}' not found")));
    }

    let mut results = Vec::new();
    for runner in &runners {
        // Stop if running
        if runner.state == crate::runner::state::RunnerState::Online
            || runner.state == crate::runner::state::RunnerState::Busy
        {
            let _ = state.runner_manager.stop_process(&runner.config.id).await;
        }

        let manager = state.runner_manager.clone();
        let auth = state.auth.clone();
        let rid = runner.config.id.clone();
        tokio::spawn(async move {
            let token = match auth.token().await {
                Some(t) => t,
                None => return,
            };
            let _ = manager.update_state(&rid, crate::runner::state::RunnerState::Registering).await;
            let _ = manager.register_and_start_from_registering(&rid, &token).await;
        });
        results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: true, error: None });
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn delete_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("Group '{group_id}' not found")));
    }

    let token = state.auth.token().await;
    let mut results = Vec::new();

    for runner in &runners {
        // Skip busy runners
        if runner.state == crate::runner::state::RunnerState::Busy {
            results.push(GroupActionResult {
                runner_id: runner.config.id.clone(),
                success: false,
                error: Some("Runner is busy, skipped".to_string()),
            });
            continue;
        }

        let delete_result = if let Some(ref token) = token {
            if runner.state == crate::runner::state::RunnerState::Online
                || runner.state == crate::runner::state::RunnerState::Offline
            {
                state.runner_manager.full_delete(&runner.config.id, token).await
            } else {
                state.runner_manager.delete(&runner.config.id).await
            }
        } else {
            state.runner_manager.delete(&runner.config.id).await
        };

        match delete_result {
            Ok(_) => results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: true, error: None }),
            Err(e) => results.push(GroupActionResult { runner_id: runner.config.id.clone(), success: false, error: Some(e.to_string()) }),
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}
```

- [ ] **Step 4: Register routes**

In `crates/daemon/src/server.rs`, add:

```rust
.route("/runners/groups/{group_id}/start", post(api::groups::start_group))
.route("/runners/groups/{group_id}/stop", post(api::groups::stop_group))
.route("/runners/groups/{group_id}/restart", post(api::groups::restart_group))
.route("/runners/groups/{group_id}", delete(api::groups::delete_group))
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -p homerund --nocapture`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/api/groups.rs crates/daemon/src/server.rs
git commit -m "feat: add group action endpoints (start/stop/restart/delete)"
```

---

## Task 5: Implement scale endpoint and list_runners group_id filter

**Files:**

- Modify: `crates/daemon/src/api/groups.rs`
- Modify: `crates/daemon/src/api/runners.rs`
- Modify: `crates/daemon/src/runner/mod.rs`
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Write tests for scale and filter**

Add to `groups::tests`:

```rust
#[tokio::test]
async fn test_scale_up_adds_runners() {
    let state = AppState::new_test();

    // Create batch of 2
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = batch["group_id"].as_str().unwrap();

    // Scale to 4
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/runners/groups/{group_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"count":4}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(resp["previous_count"].as_u64().unwrap(), 2);
    assert_eq!(resp["actual_count"].as_u64().unwrap(), 4);
    assert_eq!(resp["added"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_scale_down_removes_runners() {
    let state = AppState::new_test();

    // Create batch of 3
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_full_name":"owner/repo","count":3}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = batch["group_id"].as_str().unwrap();

    // Scale to 1
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/runners/groups/{group_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(resp["previous_count"].as_u64().unwrap(), 3);
    assert_eq!(resp["actual_count"].as_u64().unwrap(), 1);
    assert_eq!(resp["removed"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_list_runners_filter_by_group_id() {
    let state = AppState::new_test();

    // Create a batch
    let app = create_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners/batch")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let group_id = batch["group_id"].as_str().unwrap();

    // Create a solo runner
    let app = create_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/runners")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"repo_full_name":"owner/repo","name":"solo-runner"}"#))
            .unwrap(),
    )
    .await
    .unwrap();

    // Filter by group_id
    let app = create_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/runners?group_id={group_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(runners.len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund -- test_scale test_list_runners_filter --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement scale_group on RunnerManager**

In `crates/daemon/src/runner/mod.rs`, add:

```rust
pub async fn scale_group(
    &self,
    group_id: &str,
    target_count: u8,
) -> Result<types::ScaleGroupResponse> {
    let runners = self.list_by_group(group_id).await;
    if runners.is_empty() {
        bail!("Group '{group_id}' not found");
    }

    let previous_count = runners.len() as u8;
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut skipped_busy = Vec::new();

    if target_count > previous_count {
        // Scale up — use config from first runner sorted by name
        let mut sorted = runners.clone();
        sorted.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        let template = &sorted[0];
        let repo_full_name = format!("{}/{}", template.config.repo_owner, template.config.repo_name);
        let to_add = target_count - previous_count;

        for _ in 0..to_add {
            match self
                .create(
                    &repo_full_name,
                    None,
                    Some(template.config.labels.clone()),
                    Some(template.config.mode.clone()),
                    Some(group_id.to_string()),
                )
                .await
            {
                Ok(runner) => added.push(runner),
                Err(e) => {
                    tracing::error!("Failed to create runner during scale-up: {e}");
                    break;
                }
            }
        }
    } else if target_count < previous_count {
        // Scale down — remove highest-numbered first, skip busy
        // Use numeric sort: parse trailing number from "{repo}-runner-{N}" pattern
        let mut sorted = runners.clone();
        sorted.sort_by(|a, b| {
            let num_a = a.config.name.rsplit('-').next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let num_b = b.config.name.rsplit('-').next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            num_b.cmp(&num_a) // reverse: highest number first
        });

        let to_remove = (previous_count - target_count) as usize;
        let mut removed_count = 0;

        for runner in &sorted {
            if removed_count >= to_remove {
                break;
            }
            if runner.state == RunnerState::Busy {
                skipped_busy.push(runner.config.id.clone());
                continue;
            }
            if let Err(e) = self.delete(&runner.config.id).await {
                tracing::error!("Failed to delete runner {} during scale-down: {e}", runner.config.id);
                continue;
            }
            removed.push(runner.config.id.clone());
            removed_count += 1;
        }
    }

    let actual_count = (previous_count as i16 + added.len() as i16 - removed.len() as i16) as u8;

    Ok(types::ScaleGroupResponse {
        group_id: group_id.to_string(),
        previous_count,
        target_count,
        actual_count,
        added,
        removed,
        skipped_busy,
    })
}
```

- [ ] **Step 4: Implement scale endpoint and list filter**

In `crates/daemon/src/api/groups.rs`, add the scale handler:

```rust
pub async fn scale_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<ScaleGroupRequest>,
) -> Result<Json<ScaleGroupResponse>, (StatusCode, String)> {
    if req.count < 1 || req.count > 10 {
        return Err((StatusCode::BAD_REQUEST, "count must be between 1 and 10".to_string()));
    }

    let response = state
        .runner_manager
        .scale_group(&group_id, req.count)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    // Spawn registration for added runners
    for runner in &response.added {
        let manager = state.runner_manager.clone();
        let auth = state.auth.clone();
        let runner_id = runner.config.id.clone();
        tokio::spawn(async move {
            let token = match auth.token().await {
                Some(t) => t,
                None => {
                    let _ = manager.update_state(&runner_id, crate::runner::state::RunnerState::Error).await;
                    return;
                }
            };
            if let Err(e) = manager.register_and_start(&runner_id, &token).await {
                tracing::error!("Failed to register runner {}: {}", runner_id, e);
                let _ = manager.update_state(&runner_id, crate::runner::state::RunnerState::Error).await;
            }
        });
    }

    Ok(Json(response))
}
```

In `crates/daemon/src/api/runners.rs`, update `list_runners` to accept a query param:

```rust
use axum::extract::Query;

#[derive(Debug, Deserialize)]
pub struct ListRunnersQuery {
    pub group_id: Option<String>,
}

pub async fn list_runners(
    State(state): State<AppState>,
    Query(query): Query<ListRunnersQuery>,
) -> Json<Vec<RunnerInfo>> {
    match query.group_id {
        Some(gid) => Json(state.runner_manager.list_by_group(&gid).await),
        None => Json(state.runner_manager.list().await),
    }
}
```

Add `use serde::Deserialize;` to the imports in `runners.rs`.

- [ ] **Step 5: Register scale route**

In `crates/daemon/src/server.rs`, add (note: PATCH and DELETE on same path need a combined route):

```rust
.route(
    "/runners/groups/{group_id}",
    axum::routing::patch(api::groups::scale_group)
        .delete(api::groups::delete_group),
)
```

Remove the standalone `.route("/runners/groups/{group_id}", delete(...))` added in Task 4.

- [ ] **Step 6: Run all tests**

Run: `cargo test -p homerund --nocapture`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/api/groups.rs crates/daemon/src/api/runners.rs crates/daemon/src/runner/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add scale endpoint and group_id filter on list_runners"
```

---

## Task 6: Update TUI client types and API methods

**Files:**

- Modify: `crates/tui/src/client.rs`

- [ ] **Step 1: Add group_id to RunnerConfig**

In `crates/tui/src/client.rs`, add to the `RunnerConfig` struct:

```rust
pub struct RunnerConfig {
    pub id: String,
    pub name: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub labels: Vec<String>,
    pub mode: String,
    pub work_dir: PathBuf,
    #[serde(default)]
    pub group_id: Option<String>,
}
```

- [ ] **Step 2: Add batch/group response types**

Add after the existing types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateResponse {
    pub group_id: String,
    pub runners: Vec<RunnerInfo>,
    pub errors: Vec<BatchCreateError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateError {
    pub index: u8,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupActionResult {
    pub runner_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupActionResponse {
    pub group_id: String,
    pub results: Vec<GroupActionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleGroupResponse {
    pub group_id: String,
    pub previous_count: u8,
    pub target_count: u8,
    pub actual_count: u8,
    pub added: Vec<RunnerInfo>,
    pub removed: Vec<String>,
    pub skipped_busy: Vec<String>,
}
```

- [ ] **Step 3: Add API methods to DaemonClient**

Add to the `// --- API methods ---` section:

```rust
pub async fn create_batch(
    &self,
    repo_full_name: &str,
    count: u8,
    labels: Option<Vec<String>>,
    mode: Option<String>,
) -> Result<BatchCreateResponse> {
    let body = serde_json::json!({
        "repo_full_name": repo_full_name,
        "count": count,
        "labels": labels,
        "mode": mode,
    });
    let text = self.request("POST", "/runners/batch", Some(body.to_string())).await?;
    Ok(serde_json::from_str(&text)?)
}

pub async fn start_group(&self, group_id: &str) -> Result<GroupActionResponse> {
    let text = self.request("POST", &format!("/runners/groups/{group_id}/start"), None).await?;
    Ok(serde_json::from_str(&text)?)
}

pub async fn stop_group(&self, group_id: &str) -> Result<GroupActionResponse> {
    let text = self.request("POST", &format!("/runners/groups/{group_id}/stop"), None).await?;
    Ok(serde_json::from_str(&text)?)
}

pub async fn restart_group(&self, group_id: &str) -> Result<GroupActionResponse> {
    let text = self.request("POST", &format!("/runners/groups/{group_id}/restart"), None).await?;
    Ok(serde_json::from_str(&text)?)
}

pub async fn delete_group(&self, group_id: &str) -> Result<GroupActionResponse> {
    let text = self.request("DELETE", &format!("/runners/groups/{group_id}"), None).await?;
    Ok(serde_json::from_str(&text)?)
}

pub async fn scale_group(&self, group_id: &str, count: u8) -> Result<ScaleGroupResponse> {
    let body = serde_json::json!({ "count": count });
    let text = self.request("PATCH", &format!("/runners/groups/{group_id}"), Some(body.to_string())).await?;
    Ok(serde_json::from_str(&text)?)
}
```

- [ ] **Step 4: Run TUI tests**

Run: `cargo test -p homerun --nocapture`
Expected: All tests PASS (existing tests should still compile and pass)

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/client.rs
git commit -m "feat: add group_id and batch/group API methods to TUI client"
```

---

## Task 7: Update TUI app state and UI for group display

**Files:**

- Modify: `crates/tui/src/app.rs`
- Modify: `crates/tui/src/ui/runners.rs`
- Modify: `crates/tui/src/main.rs`

- [ ] **Step 1: Write tests for group expand/collapse state**

Add to `crates/tui/src/app.rs` tests:

```rust
#[test]
fn test_group_expand_collapse() {
    let mut app = App::new();
    app.runners = vec![
        make_test_runner_with_group("r1", "online", Some("g1")),
        make_test_runner_with_group("r2", "online", Some("g1")),
        make_test_runner_with_group("r3", "offline", None),
    ];
    app.rebuild_display_items();

    // Group should start collapsed — display shows group row + solo runner
    assert_eq!(app.display_items.len(), 2); // group row + solo

    // Expand group
    app.toggle_group("g1");
    app.rebuild_display_items();
    assert_eq!(app.display_items.len(), 4); // group row + 2 runners + solo
}
```

Also add the helper:

```rust
fn make_test_runner_with_group(id: &str, state: &str, group_id: Option<&str>) -> crate::client::RunnerInfo {
    let mut r = make_test_runner(id, state);
    r.config.group_id = group_id.map(String::from);
    r
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerun -- test_group_expand --nocapture`
Expected: FAIL

- [ ] **Step 3: Add group state and display items to App**

In `crates/tui/src/app.rs`, add:

```rust
use std::collections::{HashMap, HashSet};
```

Add an enum for display items:

```rust
#[derive(Debug, Clone)]
pub enum DisplayItem {
    GroupRow {
        group_id: String,
        name_prefix: String,
        runner_count: usize,
        status_summary: HashMap<String, usize>,
    },
    RunnerRow {
        runner_index: usize,  // index into app.runners
        group_id: Option<String>,
    },
}
```

Add fields to `App`:

```rust
pub expanded_groups: HashSet<String>,
pub display_items: Vec<DisplayItem>,
```

Initialize in `new()`:

```rust
expanded_groups: HashSet::new(),
display_items: Vec::new(),
```

Add methods:

```rust
pub fn toggle_group(&mut self, group_id: &str) {
    if self.expanded_groups.contains(group_id) {
        self.expanded_groups.remove(group_id);
    } else {
        self.expanded_groups.insert(group_id.to_string());
    }
}

pub fn rebuild_display_items(&mut self) {
    let mut items = Vec::new();
    let mut seen_groups: HashSet<String> = HashSet::new();

    // Collect grouped runners
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut solo_indices: Vec<usize> = Vec::new();

    for (i, runner) in self.runners.iter().enumerate() {
        if let Some(ref gid) = runner.config.group_id {
            groups.entry(gid.clone()).or_default().push(i);
        } else {
            solo_indices.push(i);
        }
    }

    // Sort group IDs by the name prefix of their first runner for stable ordering
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| {
        let name_a = &self.runners[a.1[0]].config.name;
        let name_b = &self.runners[b.1[0]].config.name;
        name_a.cmp(name_b)
    });

    // Add group rows (and their children if expanded)
    for (group_id, indices) in &sorted_groups {

        // Derive name prefix from first runner
        let first_runner = &self.runners[indices[0]];
        let name_prefix = first_runner.config.name
            .rsplit_once('-')
            .map(|(prefix, _)| prefix.to_string())
            .unwrap_or_else(|| first_runner.config.name.clone());

        let mut status_summary = HashMap::new();
        for &idx in indices {
            *status_summary.entry(self.runners[idx].state.clone()).or_insert(0) += 1;
        }

        items.push(DisplayItem::GroupRow {
            group_id: group_id.clone(),
            name_prefix,
            runner_count: indices.len(),
            status_summary,
        });

        if self.expanded_groups.contains(group_id) {
            for &idx in indices {
                items.push(DisplayItem::RunnerRow {
                    runner_index: idx,
                    group_id: Some(group_id.clone()),
                });
            }
        }
    }

    // Add solo runners
    for idx in solo_indices {
        items.push(DisplayItem::RunnerRow {
            runner_index: idx,
            group_id: None,
        });
    }

    self.display_items = items;
}
```

- [ ] **Step 4: Update navigation and key handling**

Replace `selected_runner_index` navigation with `selected_display_index: usize` in `App`. Update `select_next_runner` / `select_prev_runner` to work on `display_items`. Add group-specific key handling:

- Enter/Right on group row → expand
- Enter/Left on group row → collapse
- `S` (Shift-s) on group row → `Action::StartGroup(group_id)`
- `X` on group row → `Action::StopGroup(group_id)`
- `r` on group row → `Action::RestartGroup(group_id)`
- `d` on group row → `Action::DeleteGroup(group_id)`
- `+` on group row → `Action::ScaleUp(group_id)`
- `-` on group row → `Action::ScaleDown(group_id)`

Add new action variants to the `Action` enum:

```rust
pub enum Action {
    StartRunner(String),
    StopRunner(String),
    RestartRunner(String),
    DeleteRunner(String),
    StartGroup(String),
    StopGroup(String),
    RestartGroup(String),
    DeleteGroup(String),
    ScaleUp(String),
    ScaleDown(String),
    RefreshRunners,
    RefreshRepos,
    RefreshMetrics,
}
```

- [ ] **Step 5: Update runners.rs UI rendering**

In `crates/tui/src/ui/runners.rs`, update `draw_runner_list` to render `app.display_items` instead of flat `app.runners`. Group rows show `▶`/`▼` chevron, name prefix, count, and colored status dots. Individual runners in expanded groups show with `├─`/`└─` tree markers.

- [ ] **Step 6: Update main.rs to handle new actions**

In `crates/tui/src/main.rs`, update `handle_action` to handle the new group actions by calling the corresponding client methods. Also call `app.rebuild_display_items()` after refreshing runners.

- [ ] **Step 7: Run all TUI tests**

Run: `cargo test -p homerun --nocapture`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/ui/runners.rs crates/tui/src/main.rs
git commit -m "feat: add group display with expand/collapse and batch actions to TUI"
```

---

## Task 8: Update desktop app types and API

**Files:**

- Modify: `apps/desktop/src/api/types.ts`
- Modify: `apps/desktop/src/api/commands.ts`
- Modify: `apps/desktop/src/hooks/useRunners.ts`
- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add types to `apps/desktop/src/api/types.ts`**

Add `group_id` to `RunnerConfig`:

```typescript
export interface RunnerConfig {
  id: string;
  name: string;
  repo_owner: string;
  repo_name: string;
  labels: string[];
  mode: string;
  work_dir: string;
  group_id?: string;
}
```

Add new types:

```typescript
export interface CreateBatchRequest {
  repo_full_name: string;
  count: number;
  labels?: string[];
  mode?: string;
}

export interface BatchCreateResponse {
  group_id: string;
  runners: RunnerInfo[];
  errors: { index: number; error: string }[];
}

export interface GroupActionResult {
  runner_id: string;
  success: boolean;
  error?: string;
}

export interface GroupActionResponse {
  group_id: string;
  results: GroupActionResult[];
}

export interface ScaleGroupResponse {
  group_id: string;
  previous_count: number;
  target_count: number;
  actual_count: number;
  added: RunnerInfo[];
  removed: string[];
  skipped_busy: string[];
}
```

- [ ] **Step 2: Add Tauri backend client methods**

In `apps/desktop/src-tauri/src/client.rs`, add `group_id` to `RunnerConfig`:

```rust
pub struct RunnerConfig {
    // ... existing fields ...
    pub group_id: Option<String>,
}
```

Add new request/response types and API methods for batch create, group actions, and scale. Follow the same pattern as existing methods (HTTP request to daemon socket).

- [ ] **Step 3: Add Tauri IPC commands**

In `apps/desktop/src-tauri/src/commands.rs`, add new commands:

```rust
#[tauri::command]
pub async fn create_batch(state: State<'_, AppState>, req: CreateBatchRequest) -> Result<BatchCreateResponse, String> { ... }

#[tauri::command(rename_all = "snake_case")]
pub async fn start_group(state: State<'_, AppState>, group_id: String) -> Result<GroupActionResponse, String> { ... }

#[tauri::command(rename_all = "snake_case")]
pub async fn stop_group(state: State<'_, AppState>, group_id: String) -> Result<GroupActionResponse, String> { ... }

#[tauri::command(rename_all = "snake_case")]
pub async fn restart_group(state: State<'_, AppState>, group_id: String) -> Result<GroupActionResponse, String> { ... }

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_group(state: State<'_, AppState>, group_id: String) -> Result<GroupActionResponse, String> { ... }

#[tauri::command(rename_all = "snake_case")]
pub async fn scale_group(state: State<'_, AppState>, group_id: String, count: u8) -> Result<ScaleGroupResponse, String> { ... }
```

Register in `lib.rs`.

- [ ] **Step 4: Add frontend API calls**

In `apps/desktop/src/api/commands.ts`, add:

```typescript
createBatch: (req: CreateBatchRequest) => invoke<BatchCreateResponse>("create_batch", { req }),
startGroup: (groupId: string) => invoke<GroupActionResponse>("start_group", { group_id: groupId }),
stopGroup: (groupId: string) => invoke<GroupActionResponse>("stop_group", { group_id: groupId }),
restartGroup: (groupId: string) => invoke<GroupActionResponse>("restart_group", { group_id: groupId }),
deleteGroup: (groupId: string) => invoke<GroupActionResponse>("delete_group", { group_id: groupId }),
scaleGroup: (groupId: string, count: number) => invoke<ScaleGroupResponse>("scale_group", { group_id: groupId, count }),
```

- [ ] **Step 5: Add hook methods to useRunners**

In `apps/desktop/src/hooks/useRunners.ts`, add:

```typescript
const createBatch = useCallback(
  async (req: CreateBatchRequest) => {
    const result = await api.createBatch(req);
    await refresh();
    return result;
  },
  [refresh],
);

const startGroup = useCallback(
  async (groupId: string) => {
    const result = await api.startGroup(groupId);
    await refresh();
    return result;
  },
  [refresh],
);

// ... same pattern for stopGroup, restartGroup, deleteGroup, scaleGroup
```

Return them from the hook.

- [ ] **Step 6: Type check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No type errors

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/api/types.ts apps/desktop/src/api/commands.ts apps/desktop/src/hooks/useRunners.ts apps/desktop/src-tauri/src/client.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add batch/group/scale API types and commands to desktop app"
```

---

## Task 9: Refactor NewRunnerWizard to use batch endpoint

**Files:**

- Modify: `apps/desktop/src/components/NewRunnerWizard.tsx`

- [ ] **Step 1: Replace client-side batch loop with server-side batch call**

In `NewRunnerWizard.tsx`:

1. Import the new type: `import type { CreateBatchRequest, BatchCreateResponse } from "../api/types";`
2. Update the `NewRunnerWizardProps` to accept a `onCreateBatch` prop:

```typescript
interface NewRunnerWizardProps {
  onClose: () => void;
  onCreate: (req: CreateRunnerRequest) => Promise<RunnerInfo>;
  onCreateBatch: (req: CreateBatchRequest) => Promise<BatchCreateResponse>;
  preselectedRepo?: string;
}
```

1. Replace the batch creation loop in `handleLaunch()`:

```typescript
} else {
  // Batch creation via server endpoint
  try {
    const result = await onCreateBatch({
      repo_full_name: selectedRepo.full_name,
      count,
      labels,
      mode,
    });
    const results: BatchResult[] = result.runners.map((r) => ({
      name: r.config.name,
      success: true,
    }));
    for (const err of result.errors) {
      results.push({ name: `runner-${err.index + 1}`, success: false, error: err.error });
    }
    setBatchResults(results);
    setLaunching(false);
    setLaunched(true);
  } catch (e) {
    setLaunchError(String(e));
    setLaunching(false);
  }
}
```

1. Remove the `generateBatchName` function and `setBatchProgress` logic for batch (no longer needed — server handles naming).

1. Update the `StepConfigure` name hint for batch mode to say "Names auto-generated by server" instead of showing specific range.

- [ ] **Step 2: Update callers to pass `onCreateBatch`**

`NewRunnerWizard` is rendered in both `apps/desktop/src/pages/Dashboard.tsx` (line 73) and `apps/desktop/src/pages/Repositories.tsx` (line 164). Both must receive the `onCreateBatch` prop, wired to `createBatch` from the `useRunners` hook.

- [ ] **Step 3: Type check and format**

Run: `cd apps/desktop && npx tsc --noEmit && npx prettier --write src/`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/NewRunnerWizard.tsx apps/desktop/src/pages/
git commit -m "refactor: use POST /runners/batch in NewRunnerWizard instead of client-side loop"
```

---

## Task 10: Implement collapsible group rows in desktop RunnerTable

**Files:**

- Create: `apps/desktop/src/components/RunnerGroupRow.tsx`
- Modify: `apps/desktop/src/components/RunnerTable.tsx`

- [ ] **Step 1: Create RunnerGroupRow component**

Create `apps/desktop/src/components/RunnerGroupRow.tsx`:

```typescript
import { useState } from "react";
import type { RunnerInfo, RunnerState } from "../api/types";
import { StatusBadge } from "./StatusBadge";
import { ConfirmDialog } from "./ConfirmDialog";

interface RunnerGroupRowProps {
  groupId: string;
  runners: RunnerInfo[];
  expanded: boolean;
  onToggle: () => void;
  onStartGroup: (groupId: string) => void;
  onStopGroup: (groupId: string) => void;
  onRestartGroup: (groupId: string) => void;
  onDeleteGroup: (groupId: string) => void;
  onScaleGroup: (groupId: string, count: number) => void;
}

export function RunnerGroupRow({
  groupId,
  runners,
  expanded,
  onToggle,
  onStartGroup,
  onStopGroup,
  onRestartGroup,
  onDeleteGroup,
  onScaleGroup,
}: RunnerGroupRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Derive name prefix from first runner
  const namePrefix = runners[0]?.config.name.replace(/-\d+$/, "") ?? "group";

  // Status summary
  const statusCounts = new Map<string, number>();
  for (const r of runners) {
    statusCounts.set(r.state, (statusCounts.get(r.state) ?? 0) + 1);
  }

  const hasRunning = runners.some(
    (r) => r.state === "online" || r.state === "busy",
  );
  const hasStopped = runners.some(
    (r) => r.state === "offline" || r.state === "error",
  );

  return (
    <>
      <tr className="group-row" onClick={onToggle} style={{ cursor: "pointer" }}>
        <td colSpan={2}>
          <span style={{ marginRight: 8 }}>{expanded ? "▼" : "▶"}</span>
          <span className="font-mono" style={{ fontWeight: 600 }}>
            {namePrefix}
          </span>
          <span className="text-muted" style={{ marginLeft: 8 }}>
            ({runners.length} instances)
          </span>
        </td>
        <td>
          {Array.from(statusCounts.entries()).map(([state, count]) => (
            <span key={state} style={{ marginRight: 8 }}>
              <StatusBadge state={state as RunnerState} /> {count}
            </span>
          ))}
        </td>
        <td></td>
        <td></td>
        <td></td>
        <td onClick={(e) => e.stopPropagation()}>
          <div style={{ display: "flex", gap: 4 }}>
            {hasStopped && (
              <button className="btn btn-sm" onClick={() => onStartGroup(groupId)} title="Start all">
                ▶
              </button>
            )}
            {hasRunning && (
              <button className="btn btn-sm" onClick={() => onStopGroup(groupId)} title="Stop all">
                ■
              </button>
            )}
            <button className="btn btn-sm" onClick={() => onRestartGroup(groupId)} title="Restart all">
              ↻
            </button>
            <button
              className="btn btn-sm"
              onClick={() => onScaleGroup(groupId, runners.length + 1)}
              title="Scale up"
              disabled={runners.length >= 10}
            >
              +
            </button>
            <button
              className="btn btn-sm"
              onClick={() => onScaleGroup(groupId, runners.length - 1)}
              title="Scale down"
              disabled={runners.length <= 1}
            >
              −
            </button>
            <button
              className="btn btn-sm"
              style={{ color: "var(--accent-red)" }}
              onClick={() => setConfirmDelete(true)}
              title="Delete all"
            >
              ✕
            </button>
          </div>
        </td>
      </tr>
      {confirmDelete && (
        <ConfirmDialog
          title="Delete Group"
          message={`Delete all ${runners.length} runners in this group? Busy runners will be skipped.`}
          confirmLabel="Delete All"
          danger
          onConfirm={() => {
            onDeleteGroup(groupId);
            setConfirmDelete(false);
          }}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </>
  );
}
```

- [ ] **Step 2: Update RunnerTable to group runners**

In `apps/desktop/src/components/RunnerTable.tsx`:

1. Add group-related props:

```typescript
interface RunnerTableProps {
  runners: RunnerInfo[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
  onStartGroup: (groupId: string) => void;
  onStopGroup: (groupId: string) => void;
  onRestartGroup: (groupId: string) => void;
  onDeleteGroup: (groupId: string) => void;
  onScaleGroup: (groupId: string, count: number) => void;
  metrics?: Map<string, number>;
}
```

1. Add state for expanded groups:

```typescript
const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
```

1. In the render logic, collect runners into groups and solo runners, then render `RunnerGroupRow` for groups and regular rows for solo runners. Expanded groups show individual runner rows indented below.

- [ ] **Step 3: Update parent component to pass group action props**

Update the `Dashboard.tsx` page to pass group actions from `useRunners` hook through to `RunnerTable`.

- [ ] **Step 4: Type check and format**

Run: `cd apps/desktop && npx tsc --noEmit && npx prettier --write src/`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/RunnerGroupRow.tsx apps/desktop/src/components/RunnerTable.tsx apps/desktop/src/pages/Dashboard.tsx
git commit -m "feat: add collapsible group rows with batch actions to desktop RunnerTable"
```

---

## Task 11: Add group-aware search filtering to desktop app

**Files:**

- Modify: `apps/desktop/src/pages/Dashboard.tsx`

- [ ] **Step 1: Update filter logic for group awareness**

In `apps/desktop/src/pages/Dashboard.tsx`, update the search filter to:

1. Match group name prefixes (derived from first runner name) in addition to individual runner names
2. If a runner inside a collapsed group matches the filter, auto-expand that group

Pass a `forceExpandedGroups` set to `RunnerTable` based on the current filter:

```typescript
const forceExpandedGroups = useMemo(() => {
  if (!filter) return new Set<string>();
  const forced = new Set<string>();
  const q = filter.toLowerCase();
  for (const runner of runners) {
    if (runner.config.group_id && runner.config.name.toLowerCase().includes(q)) {
      forced.add(runner.config.group_id);
    }
  }
  return forced;
}, [runners, filter]);
```

Pass `forceExpandedGroups` to `RunnerTable` and merge it with user-toggled expansion state.

- [ ] **Step 2: Type check and format**

Run: `cd apps/desktop && npx tsc --noEmit && npx prettier --write src/`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/pages/Dashboard.tsx apps/desktop/src/components/RunnerTable.tsx
git commit -m "feat: add group-aware search filtering with auto-expand in desktop app"
```

---

## Task 12: Run full test suite, lint, and verify

**Files:** All

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test --nocapture`
Expected: All daemon and TUI tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run cargo fmt**

Run: `cargo fmt --check`
Expected: No formatting issues (run `cargo fmt` if not)

- [ ] **Step 4: Type check desktop app**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No type errors

- [ ] **Step 5: Format desktop app**

Run: `cd apps/desktop && npx prettier --write src/`
Expected: Files formatted

- [ ] **Step 6: Commit any formatting fixes**

```bash
git add -A
git commit -m "chore: formatting and lint fixes"
```
