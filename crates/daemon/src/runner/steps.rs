use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::warn;

/// Status of a single job step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

/// Information about a single job step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: StepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// API response for step progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}

/// Events parsed from Worker log lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepEvent {
    Discovered {
        name: String,
        timestamp: DateTime<Utc>,
    },
    Started {
        timestamp: DateTime<Utc>,
    },
    Completed {
        result: StepStatus,
        timestamp: DateTime<Utc>,
    },
}

/// Parse a Worker log timestamp in `YYYY-MM-DD HH:MM:SSZ` format.
fn parse_worker_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Expected: "2026-03-23 07:54:53Z" — strip trailing 'Z' for NaiveDateTime parse
    let trimmed = s.strip_suffix('Z')?;
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(naive.and_utc())
}

/// Parse a single Worker log line into a [`StepEvent`], if it matches a known pattern.
///
/// Expected patterns:
/// - `[<ts> INFO StepsRunner] Processing step: DisplayName='<name>'` → Discovered
/// - `[<ts> INFO StepsRunner] Starting the step.` → Started
/// - `[<ts> INFO StepsRunner] … current step result '<Result>'.` → Completed
pub fn parse_step_event(line: &str) -> Option<StepEvent> {
    // Early return if not a StepsRunner line
    if !line.contains("StepsRunner]") {
        return None;
    }

    // Extract timestamp from the leading bracket: [YYYY-MM-DD HH:MM:SSZ ...]
    let ts_start = line.find('[')? + 1;
    let ts_end = ts_start + "2026-03-23 07:54:53Z".len();
    if ts_end > line.len() {
        return None;
    }
    let timestamp = parse_worker_timestamp(&line[ts_start..ts_end])?;

    // Match patterns
    if let Some(idx) = line.find("DisplayName='") {
        let name_start = idx + "DisplayName='".len();
        let name_end = line[name_start..].find('\'')?;
        let name = line[name_start..name_start + name_end].to_string();
        return Some(StepEvent::Discovered { name, timestamp });
    }

    if line.contains("Starting the step.") {
        return Some(StepEvent::Started { timestamp });
    }

    if let Some(idx) = line.find("current step result '") {
        let result_start = idx + "current step result '".len();
        let result_end = line[result_start..].find('\'')?;
        let result_str = &line[result_start..result_start + result_end];
        let result = match result_str {
            "Succeeded" => StepStatus::Succeeded,
            "Failed" => StepStatus::Failed,
            "Skipped" => StepStatus::Skipped,
            "Cancelled" => StepStatus::Cancelled,
            _ => return None,
        };
        return Some(StepEvent::Completed { result, timestamp });
    }

    None
}

/// Internal state for tracking a single runner's step progress.
struct RunnerStepState {
    job_name: String,
    steps: Vec<StepInfo>,
    file_offset: u64,
    log_path: Option<PathBuf>,
    work_dir: PathBuf,
}

/// Watches Worker log files to track step progress for runners.
///
/// Designed to be shared across async tasks via `Arc<RwLock<>>` interior.
#[derive(Clone)]
pub struct WorkerLogWatcher {
    step_state: Arc<RwLock<HashMap<String, RunnerStepState>>>,
}

impl Default for WorkerLogWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerLogWatcher {
    /// Create a new empty watcher.
    pub fn new() -> Self {
        Self {
            step_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Begin watching a runner's Worker log for step progress.
    ///
    /// Stores the work directory; the actual log file is discovered lazily
    /// during the first `poll()` call, avoiding races when the Worker
    /// process hasn't spawned yet.
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

    /// Stop watching a runner and remove its state.
    pub async fn stop_watching(&self, runner_id: &str) {
        self.step_state.write().await.remove(runner_id);
    }

    /// Poll a runner's Worker log for new step events.
    ///
    /// Returns `false` if the runner is not being watched (signal to stop polling).
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

        {
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
                        // Fall through to read the new file immediately
                    } else {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }

        let log_path = state.log_path.as_ref().unwrap();
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

    /// Get the current step state for a runner.
    pub async fn get_steps(&self, runner_id: &str) -> Option<StepsResponse> {
        let map = self.step_state.read().await;
        let state = map.get(runner_id)?;
        Some(StepsResponse {
            job_name: state.job_name.clone(),
            steps: state.steps.clone(),
            steps_discovered: state.steps.len(),
        })
    }
}

/// Apply a step event to the steps list.
fn apply_step_event(steps: &mut Vec<StepInfo>, event: StepEvent) {
    match event {
        StepEvent::Discovered { name, timestamp: _ } => {
            let number = steps.len() as u16 + 1;
            steps.push(StepInfo {
                number,
                name,
                status: StepStatus::Pending,
                started_at: None,
                completed_at: None,
            });
        }
        StepEvent::Started { timestamp } => {
            if let Some(step) = steps
                .iter_mut()
                .rev()
                .find(|s| s.status == StepStatus::Pending)
            {
                step.status = StepStatus::Running;
                step.started_at = Some(timestamp);
            }
        }
        StepEvent::Completed { result, timestamp } => {
            if let Some(step) = steps
                .iter_mut()
                .rev()
                .find(|s| s.status == StepStatus::Running)
            {
                step.status = result;
                step.completed_at = Some(timestamp);
            }
        }
    }
}

/// Find the newest `Worker_*.log` file in `{work_dir}/_diag/`.
fn find_latest_worker_log(work_dir: &Path) -> Option<PathBuf> {
    let diag_dir = work_dir.join("_diag");
    let read_dir = match std::fs::read_dir(&diag_dir) {
        Ok(rd) => rd,
        Err(e) => {
            warn!(
                "Failed to read _diag directory at {}: {}",
                diag_dir.display(),
                e
            );
            return None;
        }
    };

    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with("Worker_") && file_name.ends_with(".log") {
            if let Ok(meta) = entry.metadata() {
                let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().is_none_or(|(_, t)| modified > *t) {
                    best = Some((path, modified));
                }
            }
        }
    }

    best.map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_step_discovered() {
        let line =
            "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Run actions/checkout@v6'";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Discovered { name, .. } => {
                assert_eq!(name, "Run actions/checkout@v6");
            }
            other => panic!("Expected Discovered, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_started() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        assert!(matches!(event.unwrap(), StepEvent::Started { .. }));
    }

    #[test]
    fn test_parse_step_succeeded() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Succeeded),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_failed() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] Updating job result with current step result 'Failed'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Failed),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_skipped() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Skipped'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Skipped),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unrelated_line_returns_none() {
        let line = "[2026-03-23 07:54:53Z INFO JobRunner] Starting job execution";
        assert!(parse_step_event(line).is_none());
    }

    #[test]
    fn test_parse_timestamp_extracted_correctly() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Build'";
        let event = parse_step_event(line).unwrap();
        let expected = Utc.with_ymd_and_hms(2026, 3, 23, 7, 54, 53).unwrap();
        match event {
            StepEvent::Discovered { timestamp, .. } => assert_eq!(timestamp, expected),
            other => panic!("Expected Discovered, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_watcher_processes_step_events() {
        let dir = tempfile::tempdir().unwrap();
        let diag = dir.path().join("_diag");
        std::fs::create_dir_all(&diag).unwrap();

        let log_path = diag.join("Worker_20260323-075453-utc.log");
        let log_content = "\
[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Checkout'\n\
[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.\n\
[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.\n\
[2026-03-23 07:54:55Z INFO StepsRunner] Processing step: DisplayName='Build'\n\
[2026-03-23 07:54:55Z INFO StepsRunner] Starting the step.\n";
        std::fs::write(&log_path, log_content).unwrap();

        let watcher = WorkerLogWatcher::new();
        watcher
            .start_watching("runner-1", "test-job", dir.path())
            .await;
        let result = watcher.poll("runner-1").await;
        assert!(result);

        let resp = watcher.get_steps("runner-1").await.unwrap();
        assert_eq!(resp.job_name, "test-job");
        assert_eq!(resp.steps.len(), 2);
        assert_eq!(resp.steps[0].name, "Checkout");
        assert_eq!(resp.steps[0].status, StepStatus::Succeeded);
        assert!(resp.steps[0].completed_at.is_some());
        assert_eq!(resp.steps[1].name, "Build");
        assert_eq!(resp.steps[1].status, StepStatus::Running);
        assert!(resp.steps[1].started_at.is_some());
    }

    #[tokio::test]
    async fn test_watcher_incremental_reads() {
        let dir = tempfile::tempdir().unwrap();
        let diag = dir.path().join("_diag");
        std::fs::create_dir_all(&diag).unwrap();

        let log_path = diag.join("Worker_20260323-075453-utc.log");
        let initial =
            "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Setup'\n\
[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.\n";
        std::fs::write(&log_path, initial).unwrap();

        let watcher = WorkerLogWatcher::new();
        watcher
            .start_watching("runner-2", "inc-job", dir.path())
            .await;
        watcher.poll("runner-2").await;

        let resp = watcher.get_steps("runner-2").await.unwrap();
        assert_eq!(resp.steps.len(), 1);
        assert_eq!(resp.steps[0].status, StepStatus::Running);

        // Append more content
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .unwrap();
        writeln!(
            file,
            "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'."
        )
        .unwrap();

        watcher.poll("runner-2").await;

        let resp = watcher.get_steps("runner-2").await.unwrap();
        assert_eq!(resp.steps.len(), 1);
        assert_eq!(resp.steps[0].status, StepStatus::Succeeded);
        assert!(resp.steps[0].completed_at.is_some());
    }

    #[tokio::test]
    async fn test_watcher_stop_clears_state() {
        let dir = tempfile::tempdir().unwrap();
        let diag = dir.path().join("_diag");
        std::fs::create_dir_all(&diag).unwrap();
        std::fs::write(diag.join("Worker_20260323-075453-utc.log"), "").unwrap();

        let watcher = WorkerLogWatcher::new();
        watcher
            .start_watching("runner-3", "stop-job", dir.path())
            .await;
        assert!(watcher.get_steps("runner-3").await.is_some());

        watcher.stop_watching("runner-3").await;
        assert!(watcher.get_steps("runner-3").await.is_none());
    }

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

    #[tokio::test]
    async fn test_watcher_no_diag_dir_returns_empty_steps() {
        let dir = tempfile::tempdir().unwrap();
        // No _diag directory created

        let watcher = WorkerLogWatcher::new();
        watcher
            .start_watching("runner-4", "no-diag-job", dir.path())
            .await;
        watcher.poll("runner-4").await;

        let resp = watcher.get_steps("runner-4").await.unwrap();
        assert_eq!(resp.steps.len(), 0);
        assert_eq!(resp.job_name, "no-diag-job");
    }

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
}
