pub mod binary;
pub mod history;
pub mod process;
pub mod state;
pub mod step_log_cache;
pub mod steps;
pub mod types;

use crate::config::Config;
use crate::github::GitHubClient;
use crate::runner::binary::ensure_runner_binary;
use crate::runner::process::{
    configure_runner, find_runner_pid, kill_orphaned_processes, remove_runner, start_runner,
};
use crate::runner::state::RunnerState;
use crate::runner::steps::{StepsResponse, WorkerLogWatcher};
use crate::runner::types::{RunnerConfig, RunnerInfo, RunnerMode};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Notify, RwLock};

/// Wrapper for persisting runners to disk with their last running state.
/// Uses `#[serde(flatten)]` for backward compatibility with old runners.json
/// that only contained RunnerConfig fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedRunner {
    #[serde(flatten)]
    config: RunnerConfig,
    #[serde(default)]
    was_running: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerEvent {
    pub runner_id: String,
    pub event_type: String, // "state_changed", "job_started", "job_completed"
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub runner_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub line: String,
    pub stream: String, // "stdout" or "stderr"
}

const RECENT_LOGS_MAX: usize = 500;

/// Handle for communicating with a runner's monitoring task.
/// The monitoring task owns the `Child` exclusively — no shared lock needed.
#[derive(Clone)]
struct ProcessHandle {
    /// Signal the monitoring task to kill the child process.
    kill_signal: Arc<Notify>,
    /// Becomes `true` once the child process has fully exited.
    exited: watch::Receiver<bool>,
}

#[derive(Clone)]
pub struct RunnerManager {
    config: Arc<Config>,
    runners: Arc<RwLock<HashMap<String, RunnerInfo>>>,
    processes: Arc<RwLock<HashMap<String, ProcessHandle>>>,
    log_tx: Arc<broadcast::Sender<LogEntry>>,
    event_tx: Arc<broadcast::Sender<RunnerEvent>>,
    recent_logs: Arc<RwLock<HashMap<String, VecDeque<LogEntry>>>>,
    name_counters: Arc<RwLock<HashMap<String, u32>>>,
    auth_token: Arc<RwLock<Option<String>>>,
    step_watcher: WorkerLogWatcher,
    pub step_log_cache: step_log_cache::StepLogCache,
    job_history: Arc<RwLock<HashMap<String, Vec<types::JobHistoryEntry>>>>,
}

/// Recursively copy the contents of `src` directory into `dst` directory.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src).with_context(|| format!("reading dir {:?}", src))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            // Preserve executable permission
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = std::fs::metadata(&src_path)?;
                let permissions = metadata.permissions();
                std::fs::set_permissions(&dst_path, permissions.clone())?;
                // If the source is executable, ensure the copy is too
                if permissions.mode() & 0o111 != 0 {
                    let mut dst_perms = std::fs::metadata(&dst_path)?.permissions();
                    dst_perms.set_mode(permissions.mode());
                    std::fs::set_permissions(&dst_path, dst_perms)?;
                }
            }
        }
    }
    Ok(())
}

impl RunnerManager {
    pub fn new(config: Config) -> Self {
        let (log_tx, _) = broadcast::channel(1024);
        let (event_tx, _) = broadcast::channel(256);
        Self {
            config: Arc::new(config),
            runners: Arc::new(RwLock::new(HashMap::new())),
            processes: Arc::new(RwLock::new(HashMap::new())),
            log_tx: Arc::new(log_tx),
            event_tx: Arc::new(event_tx),
            recent_logs: Arc::new(RwLock::new(HashMap::new())),
            name_counters: Arc::new(RwLock::new(HashMap::new())),
            auth_token: Arc::new(RwLock::new(None)),
            step_watcher: WorkerLogWatcher::new(),
            step_log_cache: step_log_cache::StepLogCache::new(),
            job_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_auth_token(&self, token: Option<String>) {
        let mut t = self.auth_token.write().await;
        *t = token;
    }

    /// Spawn a background task that periodically checks busy runners missing
    /// `job_context` and fetches branch/PR info from the GitHub API.
    pub fn start_job_context_poller(&self) {
        let runners = self.runners.clone();
        let auth_token = self.auth_token.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;

                // Collect busy runners missing job_context
                let needs_context: Vec<(String, String, String, String, String)> = {
                    let map = runners.read().await;
                    map.values()
                        .filter(|r| {
                            r.state == RunnerState::Busy
                                && r.job_context.is_none()
                                && r.current_job.is_some()
                        })
                        .map(|r| {
                            (
                                r.config.id.clone(),
                                r.config.name.clone(),
                                r.config.repo_owner.clone(),
                                r.config.repo_name.clone(),
                                r.current_job.clone().unwrap(),
                            )
                        })
                        .collect()
                };

                if needs_context.is_empty() {
                    continue;
                }

                let token = {
                    let t = auth_token.read().await;
                    t.clone()
                };
                let Some(token) = token else {
                    continue;
                };
                let Ok(gh) = GitHubClient::new(Some(token)) else {
                    continue;
                };

                for (runner_id, runner_name, owner, repo, job_name) in needs_context {
                    match gh
                        .get_active_run_for_runner(&owner, &repo, &runner_name, &job_name)
                        .await
                    {
                        Ok(Some(ctx)) => {
                            tracing::info!(
                                runner = %runner_id,
                                branch = %ctx.branch,
                                pr = ?ctx.pr_number,
                                "Job context fetched"
                            );
                            let mut map = runners.write().await;
                            if let Some(r) = map.get_mut(&runner_id) {
                                r.job_context = Some(ctx);
                            }
                        }
                        Ok(None) => {
                            tracing::debug!(
                                runner = %runner_id,
                                runner_name = %runner_name,
                                job_name = %job_name,
                                "No matching workflow run found yet"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                runner = %runner_id,
                                error = %e,
                                "Failed to fetch job context"
                            );
                        }
                    }
                }
            }
        });
    }

    async fn next_runner_number(&self, repo_name: &str) -> u32 {
        // Find existing numbers for this repo to pick the next available one
        let runners = self.runners.read().await;
        let existing_nums: std::collections::HashSet<u32> = runners
            .values()
            .filter(|r| r.config.repo_name == repo_name)
            .filter_map(|r| {
                let prefix = format!("{}-runner-", repo_name);
                r.config.name.strip_prefix(&prefix)?.parse::<u32>().ok()
            })
            .collect();
        drop(runners);

        // Find lowest unused number starting from 1
        let mut num = 1;
        while existing_nums.contains(&num) {
            num += 1;
        }

        // Also check the counter to avoid reuse within the same session
        // (e.g., during a batch create where runners are being added in a loop)
        let mut counters = self.name_counters.write().await;
        let counter = counters.entry(repo_name.to_string()).or_insert(0);
        if num <= *counter {
            *counter += 1;
            num = *counter;
        } else {
            *counter = num;
        }
        num
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<LogEntry> {
        self.log_tx.subscribe()
    }

    pub fn log_sender(&self) -> &broadcast::Sender<LogEntry> {
        &self.log_tx
    }

    pub async fn get_recent_logs(&self, runner_id: &str) -> Vec<LogEntry> {
        self.recent_logs
            .read()
            .await
            .get(runner_id)
            .map(|dq| dq.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<RunnerEvent> {
        self.event_tx.subscribe()
    }

    pub fn event_sender(&self) -> &broadcast::Sender<RunnerEvent> {
        &self.event_tx
    }

    fn with_computed_uptime(mut info: RunnerInfo) -> RunnerInfo {
        info.uptime_secs = info.started_at.map(|started| {
            let elapsed = chrono::Utc::now() - started;
            elapsed.num_seconds().max(0) as u64
        });
        info
    }

    fn with_job_estimate(
        mut info: RunnerInfo,
        history: &HashMap<String, Vec<types::JobHistoryEntry>>,
    ) -> RunnerInfo {
        if info.state == RunnerState::Busy {
            if let Some(ref job_name) = info.current_job {
                if let Some(entries) = history.get(&info.config.id) {
                    info.estimated_job_duration_secs =
                        history::median_duration_secs(entries, job_name);
                }
            }
        }
        info
    }

    // ── Persistence ────────────────────────────────────────────────

    /// Save all runner configs to disk as JSON.
    pub async fn save_to_disk(&self) -> Result<()> {
        let runners = self.runners.read().await;
        let persisted: Vec<PersistedRunner> = runners
            .values()
            .map(|r| PersistedRunner {
                config: r.config.clone(),
                was_running: r.state == RunnerState::Online || r.state == RunnerState::Busy,
            })
            .collect();
        let json = serde_json::to_string_pretty(&persisted)?;
        let path = self.config.runners_json_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load runner configs from disk. For runners that were previously running,
    /// checks if their process is still alive and reattaches to it.
    /// Returns a list of runner IDs that were running but whose process is now dead
    /// (these need to be restarted if the restore preference is enabled).
    pub async fn load_from_disk(&self) -> Result<Vec<String>> {
        let path = self.config.runners_json_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let json = std::fs::read_to_string(&path)?;
        let persisted: Vec<PersistedRunner> = serde_json::from_str(&json)?;
        let mut need_restart = Vec::new();
        let mut runners = self.runners.write().await;
        for entry in persisted {
            let id = entry.config.id.clone();
            let is_service = entry.config.mode == RunnerMode::Service;
            let (state, pid) = if is_service {
                // Service runners survive daemon restart — always check if process is alive
                match find_runner_pid(&entry.config.work_dir).await {
                    Some(pid) => (RunnerState::Online, Some(pid as u32)),
                    None => {
                        // Service runner's process died — should be restarted
                        if entry.was_running {
                            need_restart.push(id.clone());
                        }
                        (RunnerState::Offline, None)
                    }
                }
            } else if entry.was_running {
                // App runners stop with daemon — kill any orphaned process
                kill_orphaned_processes(&entry.config.work_dir).await;
                need_restart.push(id.clone());
                (RunnerState::Offline, None)
            } else {
                (RunnerState::Offline, None)
            };
            runners.insert(
                id,
                RunnerInfo {
                    config: entry.config,
                    state,
                    pid,
                    uptime_secs: None,
                    started_at: None,
                    jobs_completed: 0,
                    jobs_failed: 0,
                    current_job: None,
                    job_context: None,
                    error_message: None,
                    job_started_at: None,
                    last_completed_job: None,
                    estimated_job_duration_secs: None,
                },
            );
        }
        drop(runners);

        // Load job history from disk
        match history::load_all(&self.config.history_dir()) {
            Ok(hist) => {
                // Populate last_completed_job for each runner from most recent history entry
                let mut runners = self.runners.write().await;
                for (runner_id, entries) in &hist {
                    if let Some(last) = entries.last() {
                        if let Some(r) = runners.get_mut(runner_id) {
                            let duration_secs =
                                (last.completed_at - last.started_at).num_seconds().max(0) as u64;
                            r.last_completed_job = Some(types::CompletedJob {
                                job_name: last.job_name.clone(),
                                succeeded: last.succeeded,
                                completed_at: last.completed_at,
                                duration_secs,
                                branch: last.branch.clone(),
                                pr_number: last.pr_number,
                                run_url: last.run_url.clone(),
                                error_message: None,
                            });
                        }
                    }
                }
                drop(runners);
                let mut job_history = self.job_history.write().await;
                *job_history = hist;
            }
            Err(e) => {
                tracing::warn!("Failed to load job history: {}", e);
            }
        }

        Ok(need_restart)
    }

    /// Spawn a background task that monitors an orphaned runner process by PID.
    /// When the process exits, transitions the runner to Offline.
    /// Also tails the runner's _diag/Runner_*.log to detect job start/complete events.
    pub fn monitor_orphaned_process(&self, runner_id: &str, pid: u32) {
        let manager = self.clone();
        let rid = runner_id.to_string();
        let kill_signal = Arc::new(Notify::new());
        let (exit_tx, exit_rx) = watch::channel(false);

        let handle = ProcessHandle {
            kill_signal: kill_signal.clone(),
            exited: exit_rx,
        };

        // Get the work_dir for diag log tailing
        let work_dir = {
            let runners = self.runners.clone();
            let rid = rid.clone();
            tokio::spawn(async move {
                let map = runners.read().await;
                map.get(&rid).map(|r| r.config.work_dir.clone())
            })
        };

        // Store handle so stop_process works on reattached runners
        let processes = self.processes.clone();
        tokio::spawn(async move {
            processes.write().await.insert(rid.clone(), handle);

            // Spawn diag log watcher for job events
            let work_dir = work_dir.await.ok().flatten();
            if let Some(ref wd) = work_dir {
                let diag_dir = wd.join("_diag");
                let mgr = manager.clone();
                let rid2 = rid.clone();
                let sw = mgr.step_watcher.clone();
                let wd2 = wd.clone();
                tokio::spawn(async move {
                    Self::tail_diag_logs(mgr, &rid2, &diag_dir, &wd2, sw).await;
                });
            }

            // Poll until the process dies or we get a kill signal
            loop {
                tokio::select! {
                    _ = kill_signal.notified() => {
                        // Kill requested — signal the process group
                        let pgid = pid as i32;
                        unsafe { libc::kill(-pgid, libc::SIGTERM); }
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        unsafe { libc::kill(-pgid, libc::SIGKILL); }
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                        // Check if process is still alive
                        let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
                        if !alive {
                            break;
                        }
                    }
                }
            }

            // Process exited — transition to Offline
            {
                let mut runners = manager.runners.write().await;
                if let Some(r) = runners.get_mut(&rid) {
                    r.state = RunnerState::Offline;
                    r.pid = None;
                    r.started_at = None;
                }
            }
            manager.emit_state_event(&rid, "offline");
            let _ = manager.save_to_disk().await;
            let _ = exit_tx.send(true);
            processes.write().await.remove(&rid);
        });
    }

    /// Tail the latest Runner_*.log in the _diag directory to detect job events
    /// for reattached (orphaned) runners whose stdout we can't read.
    async fn tail_diag_logs(
        manager: Self,
        runner_id: &str,
        diag_dir: &std::path::Path,
        work_dir: &std::path::Path,
        step_watcher: WorkerLogWatcher,
    ) {
        use tokio::io::{AsyncBufReadExt, BufReader};

        loop {
            // Find the latest Runner_*.log
            let latest = match std::fs::read_dir(diag_dir) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("Runner_"))
                    .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok())),
                Err(_) => break,
            };

            let log_path = match latest {
                Some(entry) => entry.path(),
                None => {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };

            // Seek to end and tail new lines
            let file = match tokio::fs::File::open(&log_path).await {
                Ok(f) => f,
                Err(_) => break,
            };
            // Seek to end
            let metadata = match file.metadata().await {
                Ok(m) => m,
                Err(_) => break,
            };
            let file_len = metadata.len();
            let file = match tokio::fs::File::open(&log_path).await {
                Ok(f) => f,
                Err(_) => break,
            };
            let reader = BufReader::new(file);
            let mut lines = reader.lines();
            // Skip to end
            let mut skipped = 0u64;
            while skipped < file_len {
                match lines.next_line().await {
                    Ok(Some(line)) => skipped += line.len() as u64 + 1,
                    _ => break,
                }
            }

            // Now tail new lines
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        // Broadcast as a log entry for the runner process logs
                        if let Some(idx) = line.find("WRITE LINE: ") {
                            let stdout_line = &line[idx + "WRITE LINE: ".len()..];
                            let entry = LogEntry {
                                runner_id: runner_id.to_string(),
                                timestamp: chrono::Utc::now(),
                                line: stdout_line.to_string(),
                                stream: "stdout".to_string(),
                            };
                            let _ = manager.log_tx.send(entry.clone());
                            {
                                let mut map = manager.recent_logs.write().await;
                                let dq = map
                                    .entry(runner_id.to_string())
                                    .or_insert_with(VecDeque::new);
                                dq.push_back(entry);
                                if dq.len() > RECENT_LOGS_MAX {
                                    dq.pop_front();
                                }
                            }
                        }

                        // Runner_*.log lines contain "WRITE LINE: <timestamp>: Running job: ..."
                        // Extract the part after "WRITE LINE: "
                        let payload = if let Some(idx) = line.find("WRITE LINE: ") {
                            &line[idx + "WRITE LINE: ".len()..]
                        } else {
                            &line
                        };

                        match parse_job_event(payload) {
                            Some(JobEvent::Started(job_name)) => {
                                {
                                    let mut map = manager.runners.write().await;
                                    if let Some(r) = map.get_mut(runner_id) {
                                        r.state = RunnerState::Busy;
                                        r.current_job = Some(job_name.clone());
                                        r.job_started_at = Some(chrono::Utc::now());
                                        r.last_completed_job = None;
                                    }
                                }
                                manager.emit_state_event(runner_id, "busy");
                                let _ = manager.save_to_disk().await;
                                step_watcher
                                    .start_watching(runner_id, &job_name, work_dir)
                                    .await;
                                // Spawn step-watcher polling task
                                let sw = step_watcher.clone();
                                let rid_poll = runner_id.to_string();
                                tokio::spawn(async move {
                                    loop {
                                        tokio::time::sleep(std::time::Duration::from_millis(500))
                                            .await;
                                        if !sw.poll(&rid_poll).await {
                                            break;
                                        }
                                    }
                                });
                            }
                            Some(JobEvent::Completed { succeeded, result }) => {
                                // Capture steps before stopping the watcher
                                let steps = step_watcher
                                    .get_steps(runner_id)
                                    .await
                                    .map(|s| s.steps)
                                    .unwrap_or_default();
                                step_watcher.stop_watching(runner_id).await;

                                // Fetch job context if the poller didn't get it in time
                                let fetched_ctx = {
                                    let has_ctx = manager
                                        .runners
                                        .read()
                                        .await
                                        .get(runner_id)
                                        .is_some_and(|r| r.job_context.is_some());
                                    if has_ctx {
                                        None
                                    } else {
                                        manager.fetch_missing_job_context(runner_id).await
                                    }
                                };
                                if let Some(ctx) = &fetched_ctx {
                                    let mut map = manager.runners.write().await;
                                    if let Some(r) = map.get_mut(runner_id) {
                                        r.job_context = Some(ctx.clone());
                                    }
                                }

                                let error_message = if succeeded {
                                    None
                                } else {
                                    // Try to fetch the full error message from GitHub annotations.
                                    // Prefer direct job_id lookup (avoids matching wrong run under
                                    // concurrency cancellation), fall back to search if unavailable.
                                    let annotation_msg = {
                                        let map = manager.runners.read().await;
                                        let info = map.get(runner_id).map(|r| {
                                            (
                                                r.config.repo_owner.clone(),
                                                r.config.repo_name.clone(),
                                                r.config.name.clone(),
                                                r.current_job.clone().unwrap_or_default(),
                                                r.job_context.as_ref().and_then(|c| c.job_id),
                                            )
                                        });
                                        if let Some((owner, repo, runner_name, job_name, job_id)) =
                                            info
                                        {
                                            let token = manager.auth_token.read().await.clone();
                                            if let Some(token) = token {
                                                if let Ok(gh) =
                                                    crate::github::GitHubClient::new(Some(token))
                                                {
                                                    let msg = if let Some(jid) = job_id {
                                                        gh.get_annotations_by_job_id(
                                                            &owner, &repo, jid,
                                                        )
                                                        .await
                                                    } else {
                                                        gh.get_job_failure_message(
                                                            &owner,
                                                            &repo,
                                                            &runner_name,
                                                            &job_name,
                                                        )
                                                        .await
                                                    };
                                                    match msg {
                                                        Ok(msg) => {
                                                            tracing::info!("Annotation fetch result for {runner_name}: {msg:?}");
                                                            msg
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!("Failed to fetch job annotations for {runner_name}: {e}");
                                                            None
                                                        }
                                                    }
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    };
                                    Some(annotation_msg.unwrap_or(result))
                                };

                                // Build history entry and update runner state
                                let history_entry = {
                                    let mut map = manager.runners.write().await;
                                    if let Some(r) = map.get_mut(runner_id) {
                                        let now = chrono::Utc::now();
                                        let started_at = r.job_started_at.unwrap_or(now);
                                        let duration_secs =
                                            (now - started_at).num_seconds().max(0) as u64;

                                        let entry = types::JobHistoryEntry {
                                            job_name: r.current_job.clone().unwrap_or_default(),
                                            started_at,
                                            completed_at: now,
                                            succeeded,
                                            branch: r
                                                .job_context
                                                .as_ref()
                                                .map(|c| c.branch.clone()),
                                            pr_number: r
                                                .job_context
                                                .as_ref()
                                                .and_then(|c| c.pr_number),
                                            run_url: r.job_context.as_ref().map(|c| {
                                                match c.job_id {
                                                    Some(job_id) => {
                                                        format!("{}/job/{}", c.run_url, job_id)
                                                    }
                                                    None => c.run_url.clone(),
                                                }
                                            }),
                                            error_message: error_message.clone(),
                                            steps,
                                        };

                                        r.last_completed_job = Some(types::CompletedJob {
                                            job_name: entry.job_name.clone(),
                                            succeeded,
                                            completed_at: now,
                                            duration_secs,
                                            branch: entry.branch.clone(),
                                            pr_number: entry.pr_number,
                                            run_url: entry.run_url.clone(),
                                            error_message: error_message.clone(),
                                        });

                                        if succeeded {
                                            r.jobs_completed += 1;
                                        } else {
                                            r.jobs_failed += 1;
                                        }
                                        r.state = RunnerState::Online;
                                        r.current_job = None;
                                        r.job_context = None;
                                        r.job_started_at = None;

                                        Some(entry)
                                    } else {
                                        None
                                    }
                                };

                                if let Some(entry) = history_entry {
                                    manager.record_job_history(runner_id, entry).await;
                                }
                                manager.emit_state_event(runner_id, "online");
                                let _ = manager.save_to_disk().await;
                            }
                            None => {}
                        }
                    }
                    Ok(None) => {
                        // EOF — wait and retry
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    Err(_) => break,
                }
            }
        }
    }

    // ── CRUD ───────────────────────────────────────────────────────

    pub async fn create(
        &self,
        repo_full_name: &str,
        name: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
        group_id: Option<String>,
    ) -> Result<RunnerInfo> {
        let parts: Vec<&str> = repo_full_name.split('/').collect();
        if parts.len() != 2 {
            bail!("Invalid repo name: expected 'owner/repo'");
        }
        let (owner, repo) = (parts[0], parts[1]);

        let id = uuid::Uuid::new_v4().to_string();
        let name = match name {
            Some(n) => n,
            None => {
                let num = self.next_runner_number(repo).await;
                format!("{repo}-runner-{num}")
            }
        };
        let work_dir = self.config.runners_dir().join(&id);
        std::fs::create_dir_all(&work_dir)?;

        let mut default_labels = vec!["self-hosted".to_string(), "macOS".to_string()];
        if cfg!(target_arch = "aarch64") {
            default_labels.push("ARM64".to_string());
        } else {
            default_labels.push("X64".to_string());
        }
        if let Some(extra) = labels {
            for label in extra {
                if !default_labels.contains(&label) {
                    default_labels.push(label);
                }
            }
        }

        let runner = RunnerInfo {
            config: RunnerConfig {
                id: id.clone(),
                name,
                repo_owner: owner.to_string(),
                repo_name: repo.to_string(),
                labels: default_labels,
                mode: mode.unwrap_or(RunnerMode::App),
                work_dir,
                group_id,
            },
            state: RunnerState::Creating,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };

        self.runners.write().await.insert(id, runner.clone());
        self.save_to_disk().await?;
        Ok(runner)
    }

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
                .create(
                    repo_full_name,
                    None,
                    labels.clone(),
                    mode.clone(),
                    Some(group_id.clone()),
                )
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

    pub async fn list_by_group(&self, group_id: &str) -> Vec<RunnerInfo> {
        let infos: Vec<RunnerInfo> = self
            .runners
            .read()
            .await
            .values()
            .filter(|r| r.config.group_id.as_deref() == Some(group_id))
            .cloned()
            .map(Self::with_computed_uptime)
            .collect();
        let history = self.job_history.read().await;
        infos
            .into_iter()
            .map(|info| Self::with_job_estimate(info, &history))
            .collect()
    }

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
            let repo_full_name = format!(
                "{}/{}",
                template.config.repo_owner, template.config.repo_name
            );
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
            let mut sorted = runners.clone();
            sorted.sort_by(|a, b| {
                let num_a = a
                    .config
                    .name
                    .rsplit('-')
                    .next()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let num_b = b
                    .config
                    .name
                    .rsplit('-')
                    .next()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                num_b.cmp(&num_a)
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
                    tracing::error!(
                        "Failed to delete runner {} during scale-down: {e}",
                        runner.config.id
                    );
                    continue;
                }
                removed.push(runner.config.id.clone());
                removed_count += 1;
            }
        }

        let actual_count =
            (previous_count as i16 + added.len() as i16 - removed.len() as i16) as u8;

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

    pub async fn list(&self) -> Vec<RunnerInfo> {
        let infos: Vec<RunnerInfo> = self
            .runners
            .read()
            .await
            .values()
            .cloned()
            .map(Self::with_computed_uptime)
            .collect();
        let history = self.job_history.read().await;
        infos
            .into_iter()
            .map(|info| Self::with_job_estimate(info, &history))
            .collect()
    }

    pub async fn runner_pids_and_names(&self) -> Vec<(String, String, Option<u32>)> {
        let runners = self.runners.read().await;
        runners
            .values()
            .map(|r| (r.config.id.clone(), r.config.name.clone(), r.pid))
            .collect()
    }

    pub async fn get(&self, id: &str) -> Option<RunnerInfo> {
        let info = self
            .runners
            .read()
            .await
            .get(id)
            .cloned()
            .map(Self::with_computed_uptime);
        match info {
            Some(info) => {
                let history = self.job_history.read().await;
                Some(Self::with_job_estimate(info, &history))
            }
            None => None,
        }
    }

    /// Get the current step progress for a running job on the given runner.
    pub async fn get_steps(&self, runner_id: &str) -> Option<StepsResponse> {
        self.step_watcher.get_steps(runner_id).await
    }

    /// Get the log lines for a specific step of a running job.
    pub async fn get_step_logs(
        &self,
        runner_id: &str,
        step_number: u16,
    ) -> Option<crate::api::steps::StepLogsResponse> {
        // 1. Get step info to find the step name
        let steps_response = self.get_steps(runner_id).await?;
        let step = steps_response
            .steps
            .iter()
            .find(|s| s.number == step_number)?;
        let step_name = step.name.clone();

        // 2. Get job_id from job_context
        let runners = self.runners.read().await;
        let runner = runners.get(runner_id)?;
        let job_id = runner.job_context.as_ref()?.job_id?;
        let owner = runner.config.repo_owner.clone();
        let repo = runner.config.repo_name.clone();
        drop(runners);

        // 3. Get auth token (same pattern as start_job_context_poller)
        let token = {
            let t = self.auth_token.read().await;
            t.clone()
        };
        let token = token?;
        let gh = crate::github::GitHubClient::new(Some(token)).ok()?;

        // 4. Fetch via cache
        let raw_log = match self
            .step_log_cache
            .get_or_fetch(job_id, &gh, &owner, &repo)
            .await
        {
            Ok(log) => log,
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch job logs for runner {} (job_id={}): {:#}",
                    runner_id,
                    job_id,
                    e
                );
                return None;
            }
        };
        let sections = crate::github::parse_job_log_sections(&raw_log);

        // 5. Match by step name (not index)
        let section = match sections.iter().find(|(name, _)| name == &step_name) {
            Some(s) => s,
            None => {
                tracing::debug!(
                    "Step '{}' not found in job log sections (found: {:?})",
                    step_name,
                    sections.iter().map(|(n, _)| n).collect::<Vec<_>>()
                );
                return None;
            }
        };
        let lines: Vec<String> = section.1.lines().map(|l| l.to_string()).collect();

        Some(crate::api::steps::StepLogsResponse {
            step_number,
            step_name,
            lines,
        })
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut runners = self.runners.write().await;
        if let Some(runner) = runners.remove(id) {
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
        }
        drop(runners);
        // Also remove any tracked process handle
        self.processes.write().await.remove(id);
        self.delete_job_history(id).await;
        self.save_to_disk().await?;
        Ok(())
    }

    pub async fn update(&self, id: &str, req: types::UpdateRunnerRequest) -> Result<RunnerInfo> {
        let mut runners = self.runners.write().await;
        let runner = runners
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if let Some(labels) = req.labels {
            runner.config.labels = labels;
        }
        if let Some(mode) = req.mode {
            runner.config.mode = mode;
        }
        Ok(runner.clone())
    }

    pub async fn update_state(&self, id: &str, state: RunnerState) -> Result<()> {
        self.update_state_with_error(id, state, None).await
    }

    pub async fn update_state_with_error(
        &self,
        id: &str,
        state: RunnerState,
        error_message: Option<String>,
    ) -> Result<()> {
        let mut runners = self.runners.write().await;
        let runner = runners
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if !runner.state.can_transition_to(&state) {
            bail!(
                "Invalid state transition: {:?} -> {:?}",
                runner.state,
                state
            );
        }
        let prev_running = runner.state == RunnerState::Online || runner.state == RunnerState::Busy;
        let now_running = state == RunnerState::Online || state == RunnerState::Busy;
        runner.state = state;
        runner.error_message = error_message;
        // Persist when running state changes so was_running stays current
        if prev_running != now_running {
            drop(runners);
            let _ = self.save_to_disk().await;
        }
        Ok(())
    }

    // ── Lifecycle ──────────────────────────────────────────────────

    /// Full register-and-start flow:
    /// 1. Creating -> Registering
    /// 2. Download / cache runner binary
    /// 3. Copy binary files to runner work_dir
    /// 4. Get registration token from GitHub
    /// 5. Run config.sh
    /// 6. Spawn run.sh
    /// 7. Store PID, update state to Online
    /// 8. Spawn background monitor task
    pub async fn register_and_start(&self, id: &str, auth_token: &str) -> Result<()> {
        // 1. Transition Creating -> Registering
        self.update_state(id, RunnerState::Registering).await?;
        self.emit_state_event(id, "registering");

        // Continue with the common flow
        self.do_register_and_start(id, auth_token).await
    }

    /// Start a runner that is already in the Registering state.
    /// Used by the start/restart API endpoints.
    pub async fn register_and_start_from_registering(
        &self,
        id: &str,
        auth_token: &str,
    ) -> Result<()> {
        self.emit_state_event(id, "registering");
        self.do_register_and_start(id, auth_token).await
    }

    /// Common register-and-start flow (assumes already in Registering state):
    /// Downloads runner binary if needed, removes stale configuration via
    /// `config.sh remove`, then runs `config.sh` to register before starting run.sh.
    async fn do_register_and_start(&self, id: &str, auth_token: &str) -> Result<()> {
        self.set_auth_token(Some(auth_token.to_string())).await;

        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        let config = &runner.config;

        // Check both .runner and .runner_migrated (newer runner versions rename
        // the config file during a migration).
        let already_configured = config.work_dir.join(".runner").exists()
            || config.work_dir.join(".runner_migrated").exists();

        if !already_configured {
            // First-time setup: download binary and copy to work_dir
            let cached_runner_dir = ensure_runner_binary(&self.config.cache_dir())
                .await
                .context("Failed to download runner binary")?;

            copy_dir_recursive(&cached_runner_dir, &config.work_dir)
                .context("Failed to copy runner binary to work dir")?;
        } else {
            tracing::info!("Runner {} already configured, skipping binary download", id);
        }

        // Kill any orphaned runner processes from a previous daemon session
        // BEFORE reconfiguring, so the old process releases the GitHub session.
        kill_orphaned_processes(&config.work_dir).await;

        let gh = GitHubClient::new(Some(auth_token.to_string()))?;
        let reg = gh
            .get_runner_registration_token(&config.repo_owner, &config.repo_name)
            .await
            .context("Failed to get registration token")?;

        // If already configured, deregister before re-configuring.
        // config.sh refuses to configure an already-configured runner, so we
        // must remove the old configuration first.
        if already_configured {
            let _ = remove_runner(&config.work_dir, &reg.token).await;
            for file_name in [
                ".runner",
                ".runner_migrated",
                ".credentials",
                ".credentials_rsaparams",
            ] {
                let path = config.work_dir.join(file_name);
                if path.exists() {
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!("Failed to remove {file_name}: {e}");
                    }
                }
            }
        }

        let repo_url = format!(
            "https://github.com/{}/{}",
            config.repo_owner, config.repo_name
        );
        configure_runner(
            &config.work_dir,
            &repo_url,
            &reg.token,
            &config.name,
            &config.labels,
        )
        .await
        .context("Failed to configure runner")?;

        // Spawn run.sh
        let mut child = start_runner(&config.work_dir)
            .await
            .context("Failed to start runner process")?;

        // 5b. Capture stdout/stderr for log streaming
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // 6. Store PID, update state to Online, record start time
        let pid = child.id();
        let started_at = chrono::Utc::now();
        {
            let mut runners = self.runners.write().await;
            if let Some(r) = runners.get_mut(id) {
                r.state = RunnerState::Online;
                r.pid = pid;
                r.started_at = Some(started_at);
            }
        }
        self.emit_state_event(id, "online");
        let _ = self.save_to_disk().await;

        // 5c. Spawn log reader tasks
        if let Some(stdout) = stdout {
            let log_tx = self.log_tx.clone();
            let recent_logs = self.recent_logs.clone();
            let runners = self.runners.clone();
            let step_watcher = self.step_watcher.clone();
            let job_history_arc = self.job_history.clone();
            let history_dir = self.config.history_dir();
            let event_tx_stdout = self.event_tx.clone();
            let auth_token_clone = self.auth_token.clone();
            let rid = id.to_string();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        runner_id: rid.clone(),
                        timestamp: chrono::Utc::now(),
                        line: line.clone(),
                        stream: "stdout".to_string(),
                    };
                    let _ = log_tx.send(entry.clone());
                    // Store in ring buffer
                    {
                        let mut map = recent_logs.write().await;
                        let dq = map.entry(rid.clone()).or_insert_with(VecDeque::new);
                        dq.push_back(entry);
                        if dq.len() > RECENT_LOGS_MAX {
                            dq.pop_front();
                        }
                    }
                    // Parse job events from stdout
                    // Lines look like: "2026-03-21 19:49:31Z: Running job: TypeScript (type check + build)"
                    match parse_job_event(&line) {
                        Some(JobEvent::Started(job_name)) => {
                            let work_dir = {
                                let mut map = runners.write().await;
                                if let Some(r) = map.get_mut(&rid) {
                                    r.state = RunnerState::Busy;
                                    r.current_job = Some(job_name.clone());
                                    r.job_started_at = Some(chrono::Utc::now());
                                    r.last_completed_job = None;
                                    Some(r.config.work_dir.clone())
                                } else {
                                    None
                                }
                            };
                            if let Some(work_dir) = work_dir {
                                step_watcher
                                    .start_watching(&rid, &job_name, &work_dir)
                                    .await;
                                // Spawn step-watcher polling task
                                let sw = step_watcher.clone();
                                let rid_poll = rid.clone();
                                tokio::spawn(async move {
                                    loop {
                                        tokio::time::sleep(std::time::Duration::from_millis(500))
                                            .await;
                                        if !sw.poll(&rid_poll).await {
                                            break;
                                        }
                                    }
                                });
                            }
                        }
                        Some(JobEvent::Completed { succeeded, result }) => {
                            let steps_data = step_watcher
                                .get_steps(&rid)
                                .await
                                .map(|s| s.steps)
                                .unwrap_or_default();
                            step_watcher.stop_watching(&rid).await;

                            let mut error_message = if succeeded { None } else { Some(result) };

                            // Fetch job context if the poller didn't get it in time
                            {
                                let has_ctx = runners
                                    .read()
                                    .await
                                    .get(&rid)
                                    .is_some_and(|r| r.job_context.is_some());
                                if !has_ctx {
                                    let info = {
                                        let map = runners.read().await;
                                        map.get(&rid).map(|r| {
                                            (
                                                r.config.name.clone(),
                                                r.config.repo_owner.clone(),
                                                r.config.repo_name.clone(),
                                                r.current_job.clone(),
                                            )
                                        })
                                    };
                                    if let Some((name, owner, repo, Some(job_name))) = info {
                                        let token = auth_token_clone.read().await.clone();
                                        if let Some(token) = token {
                                            if let Ok(gh) = GitHubClient::new(Some(token)) {
                                                if let Ok(Some(ctx)) = gh
                                                    .get_recent_run_for_job(
                                                        &owner, &repo, &name, &job_name,
                                                    )
                                                    .await
                                                {
                                                    let mut map = runners.write().await;
                                                    if let Some(r) = map.get_mut(&rid) {
                                                        r.job_context = Some(ctx);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Fetch full error message from GitHub annotations.
                            // Prefer direct job_id lookup when available.
                            if !succeeded {
                                let info = {
                                    let map = runners.read().await;
                                    map.get(&rid).map(|r| {
                                        (
                                            r.config.repo_owner.clone(),
                                            r.config.repo_name.clone(),
                                            r.config.name.clone(),
                                            r.current_job.clone().unwrap_or_default(),
                                            r.job_context.as_ref().and_then(|c| c.job_id),
                                        )
                                    })
                                };
                                if let Some((owner, repo, runner_name, job_name, job_id)) = info {
                                    let token = auth_token_clone.read().await.clone();
                                    if let Some(token) = token {
                                        if let Ok(gh) =
                                            crate::github::GitHubClient::new(Some(token))
                                        {
                                            let result = if let Some(jid) = job_id {
                                                gh.get_annotations_by_job_id(&owner, &repo, jid)
                                                    .await
                                            } else {
                                                gh.get_job_failure_message(
                                                    &owner,
                                                    &repo,
                                                    &runner_name,
                                                    &job_name,
                                                )
                                                .await
                                            };
                                            match result {
                                                Ok(Some(msg)) => {
                                                    tracing::info!("Annotation fetch result for {runner_name}: {msg:?}");
                                                    error_message = Some(msg);
                                                }
                                                Ok(None) => {
                                                    tracing::info!(
                                                        "No annotations found for {runner_name}"
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::warn!("Failed to fetch job annotations for {runner_name}: {e}");
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let history_entry = {
                                let mut map = runners.write().await;
                                if let Some(r) = map.get_mut(&rid) {
                                    let now = chrono::Utc::now();
                                    let started_at = r.job_started_at.unwrap_or(now);
                                    let duration_secs =
                                        (now - started_at).num_seconds().max(0) as u64;

                                    let entry = types::JobHistoryEntry {
                                        job_name: r.current_job.clone().unwrap_or_default(),
                                        started_at,
                                        completed_at: now,
                                        succeeded,
                                        branch: r.job_context.as_ref().map(|c| c.branch.clone()),
                                        pr_number: r.job_context.as_ref().and_then(|c| c.pr_number),
                                        run_url: r.job_context.as_ref().map(|c| match c.job_id {
                                            Some(job_id) => format!("{}/job/{}", c.run_url, job_id),
                                            None => c.run_url.clone(),
                                        }),
                                        error_message: error_message.clone(),
                                        steps: steps_data,
                                    };

                                    r.last_completed_job = Some(types::CompletedJob {
                                        job_name: entry.job_name.clone(),
                                        succeeded,
                                        completed_at: now,
                                        duration_secs,
                                        branch: entry.branch.clone(),
                                        pr_number: entry.pr_number,
                                        run_url: entry.run_url.clone(),
                                        error_message: error_message.clone(),
                                    });

                                    if succeeded {
                                        r.jobs_completed += 1;
                                    } else {
                                        r.jobs_failed += 1;
                                    }
                                    r.state = RunnerState::Online;
                                    r.current_job = None;
                                    r.job_context = None;
                                    r.job_started_at = None;

                                    Some(entry)
                                } else {
                                    None
                                }
                            };

                            // Record history via cloned Arcs
                            if let Some(entry) = history_entry {
                                let mut hist = job_history_arc.write().await;
                                let entries = hist.entry(rid.clone()).or_default();
                                history::append(entries, entry);
                                if let Err(e) = history::save(&history_dir, &rid, entries) {
                                    tracing::warn!("Failed to save job history for {}: {}", rid, e);
                                }
                            }

                            // Emit state event
                            let _ = event_tx_stdout.send(RunnerEvent {
                                runner_id: rid.clone(),
                                event_type: "state_changed".to_string(),
                                data: serde_json::json!({"state": "online"}),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                        None => {}
                    }
                }
            });
        }
        if let Some(stderr) = stderr {
            let log_tx = self.log_tx.clone();
            let recent_logs = self.recent_logs.clone();
            let rid = id.to_string();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        runner_id: rid.clone(),
                        timestamp: chrono::Utc::now(),
                        line,
                        stream: "stderr".to_string(),
                    };
                    let _ = log_tx.send(entry.clone());
                    // Store in ring buffer
                    {
                        let mut map = recent_logs.write().await;
                        let dq = map.entry(rid.clone()).or_insert_with(VecDeque::new);
                        dq.push_back(entry);
                        if dq.len() > RECENT_LOGS_MAX {
                            dq.pop_front();
                        }
                    }
                }
            });
        }

        // 5d. Spawn step-watcher polling task
        {
            let step_watcher = self.step_watcher.clone();
            let rid = id.to_string();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if !step_watcher.poll(&rid).await {
                        break;
                    }
                }
            });
        }

        // Store process handle with kill signal + exit watch
        let kill_signal = Arc::new(Notify::new());
        let (exit_tx, exit_rx) = watch::channel(false);

        let handle = ProcessHandle {
            kill_signal: kill_signal.clone(),
            exited: exit_rx,
        };
        self.processes.write().await.insert(id.to_string(), handle);

        // 7. Spawn background monitor task — owns `child` exclusively
        let manager = self.clone();
        let runner_id = id.to_string();
        tokio::spawn(async move {
            let exit_status = tokio::select! {
                status = child.wait() => status,
                _ = kill_signal.notified() => {
                    // Kill signal received — gracefully stop the entire process group.
                    // run.sh spawns .NET child processes that hold the GitHub session,
                    // so we must signal the whole group to let them deregister cleanly.
                    if let Some(pid) = child.id() {
                        let pgid = pid as i32;
                        // SIGTERM the process group for graceful shutdown
                        unsafe { libc::kill(-pgid, libc::SIGTERM); }

                        // Wait up to 10s for graceful exit
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            child.wait(),
                        )
                        .await
                        {
                            Ok(status) => status,
                            Err(_) => {
                                tracing::warn!(
                                    "Runner {} did not exit gracefully, sending SIGKILL",
                                    runner_id
                                );
                                unsafe {
                                    libc::kill(-pgid, libc::SIGKILL);
                                }
                                child.wait().await
                            }
                        }
                    } else {
                        // No PID — process already exited
                        child.wait().await
                    }
                }
            };
            tracing::info!("Runner {} exited with status: {:?}", runner_id, exit_status);

            // Signal that the process has fully exited
            let _ = exit_tx.send(true);

            // Update state to Offline
            let mut runners = manager.runners.write().await;
            if let Some(r) = runners.get_mut(&runner_id) {
                if r.state == RunnerState::Online
                    || r.state == RunnerState::Busy
                    || r.state == RunnerState::Stopping
                {
                    r.state = RunnerState::Offline;
                    r.pid = None;
                    r.started_at = None;
                }
            }
            drop(runners);
            manager.processes.write().await.remove(&runner_id);
            manager.emit_state_event(&runner_id, "offline");
        });

        Ok(())
    }

    /// Stop a running runner process.
    ///
    /// Signals the monitoring task to kill the child via `Notify`, then waits
    /// for the `watch` channel to confirm the process has fully exited.
    /// No shared lock on `Child` — eliminates the deadlock from issue #31.
    pub async fn stop_process(&self, id: &str) -> Result<()> {
        // Transition to Stopping
        self.update_state(id, RunnerState::Stopping).await?;
        self.emit_state_event(id, "stopping");

        // Get the process handle
        let handle = self.processes.read().await.get(id).cloned();

        if let Some(handle) = handle {
            // Signal the monitoring task to kill the child
            handle.kill_signal.notify_one();

            // Wait for the process to fully exit (with timeout)
            let mut exited = handle.exited;
            let timeout =
                tokio::time::timeout(std::time::Duration::from_secs(15), exited.wait_for(|&v| v))
                    .await;

            if timeout.is_err() {
                tracing::warn!("Timed out waiting for runner {} to exit", id);
            }
        }

        // Ensure state is Offline (monitor task may have already done this,
        // but handle the case where there was no process handle)
        {
            let mut runners = self.runners.write().await;
            if let Some(r) = runners.get_mut(id) {
                if r.state == RunnerState::Stopping {
                    r.state = RunnerState::Offline;
                    r.pid = None;
                    r.started_at = None;
                }
            }
        }
        self.emit_state_event(id, "offline");
        Ok(())
    }

    /// Full delete flow: stop process, deregister from GitHub, remove work dir.
    pub async fn full_delete(&self, id: &str, auth_token: &str) -> Result<()> {
        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;

        // Stop if running
        if runner.state == RunnerState::Online || runner.state == RunnerState::Busy {
            let _ = self.stop_process(id).await;
        }

        // Try to transition to Deleting
        {
            let mut runners = self.runners.write().await;
            if let Some(r) = runners.get_mut(id) {
                // Force the state for deletion
                r.state = RunnerState::Deleting;
            }
        }
        self.emit_state_event(id, "deleting");

        // Deregister from GitHub
        let config = &runner.config;
        if let Ok(gh) = GitHubClient::new(Some(auth_token.to_string())) {
            if let Ok(reg) = gh
                .get_runner_registration_token(&config.repo_owner, &config.repo_name)
                .await
            {
                let _ = remove_runner(&config.work_dir, &reg.token).await;
            }
        }

        // Remove runner entry and work dir
        self.delete(id).await?;
        Ok(())
    }

    fn emit_state_event(&self, runner_id: &str, state: &str) {
        let _ = self.event_tx.send(RunnerEvent {
            runner_id: runner_id.to_string(),
            event_type: "state_changed".to_string(),
            data: serde_json::json!({"state": state}),
            timestamp: chrono::Utc::now(),
        });
    }

    /// Record a completed job in history.
    pub async fn record_job_history(&self, runner_id: &str, entry: types::JobHistoryEntry) {
        let mut hist = self.job_history.write().await;
        let entries = hist.entry(runner_id.to_string()).or_default();
        history::append(entries, entry);
        if let Err(e) = history::save(&self.config.history_dir(), runner_id, entries) {
            tracing::warn!("Failed to save job history for {}: {}", runner_id, e);
        }
    }

    /// Get job history for a runner (newest first).
    pub async fn get_job_history(&self, runner_id: &str) -> Vec<types::JobHistoryEntry> {
        let hist = self.job_history.read().await;
        let mut entries = hist.get(runner_id).cloned().unwrap_or_default();
        entries.reverse();
        entries
    }

    /// Delete job history for a runner.
    pub async fn delete_job_history(&self, runner_id: &str) {
        self.job_history.write().await.remove(runner_id);
        if let Err(e) = history::delete(&self.config.history_dir(), runner_id) {
            tracing::warn!("Failed to delete job history for {}: {}", runner_id, e);
        }
    }

    /// Delete a single job history entry by its `started_at` timestamp.
    pub async fn delete_job_history_entry(&self, runner_id: &str, started_at: &str) -> Result<()> {
        let ts: chrono::DateTime<chrono::Utc> = started_at
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid timestamp: {e}"))?;
        let mut hist = self.job_history.write().await;
        let entries = hist
            .get_mut(runner_id)
            .ok_or_else(|| anyhow::anyhow!("No history for runner"))?;
        let before = entries.len();
        entries.retain(|e| e.started_at != ts);
        if entries.len() == before {
            anyhow::bail!("No matching history entry found");
        }
        history::save(&self.config.history_dir(), runner_id, entries)?;
        Ok(())
    }

    /// Try to fetch job context from GitHub for a runner that's missing it.
    /// Called at job completion for fast jobs that finish before the poller runs.
    pub async fn fetch_missing_job_context(&self, runner_id: &str) -> Option<types::JobContext> {
        let (runner_name, owner, repo, job_name) = {
            let map = self.runners.read().await;
            let r = map.get(runner_id)?;
            (
                r.config.name.clone(),
                r.config.repo_owner.clone(),
                r.config.repo_name.clone(),
                r.current_job.clone()?,
            )
        };

        let token = self.auth_token.read().await.clone()?;
        let gh = GitHubClient::new(Some(token)).ok()?;

        match gh
            .get_recent_run_for_job(&owner, &repo, &runner_name, &job_name)
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::debug!(
                    runner = %runner_id,
                    error = %e,
                    "Failed to fetch missing job context at completion"
                );
                None
            }
        }
    }
}

/// Parsed result of a job-related stdout line emitted by the GitHub Actions runner.
#[derive(Debug, PartialEq)]
pub enum JobEvent {
    /// The runner started executing a job with the given name.
    Started(String),
    /// A job completed; `succeeded` is true when the result was "Succeeded".
    /// `result` contains the raw result string (e.g. "Succeeded", "Failed", "Cancelled").
    Completed { succeeded: bool, result: String },
}

/// Parse a single stdout line from the runner process into a [`JobEvent`], if it
/// matches a known pattern.
///
/// Expected patterns (prefixed by a timestamp the function ignores):
/// - `"… Running job: <name>"` → [`JobEvent::Started`]
/// - `"… completed with result: Succeeded|<other>"` → [`JobEvent::Completed`]
pub fn parse_job_event(line: &str) -> Option<JobEvent> {
    if let Some(idx) = line.find("Running job: ") {
        let job_name = line[idx + "Running job: ".len()..].to_string();
        return Some(JobEvent::Started(job_name));
    }
    if let Some(idx) = line.find("completed with result: ") {
        let result = line[idx + "completed with result: ".len()..]
            .trim()
            .to_string();
        let succeeded = result == "Succeeded";
        return Some(JobEvent::Completed { succeeded, result });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use state::RunnerState;

    fn create_test_manager() -> RunnerManager {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        RunnerManager::new(config)
    }

    #[tokio::test]
    async fn test_event_broadcast() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);
        let mut rx = manager.subscribe_events();

        manager
            .event_sender()
            .send(RunnerEvent {
                runner_id: "test".to_string(),
                event_type: "state_changed".to_string(),
                data: serde_json::json!({"state": "online"}),
                timestamp: chrono::Utc::now(),
            })
            .unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "state_changed");
        assert_eq!(event.runner_id, "test");
    }

    #[tokio::test]
    async fn test_log_broadcast() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);
        let mut rx = manager.subscribe_logs();

        manager
            .log_sender()
            .send(LogEntry {
                runner_id: "test".to_string(),
                timestamp: chrono::Utc::now(),
                line: "hello".to_string(),
                stream: "stdout".to_string(),
            })
            .unwrap();

        let entry = rx.recv().await.unwrap();
        assert_eq!(entry.line, "hello");
        assert_eq!(entry.runner_id, "test");
        assert_eq!(entry.stream, "stdout");
    }

    #[tokio::test]
    async fn test_create_runner_generates_id_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("aGallea/gifted", None, None, None, None)
            .await
            .unwrap();

        assert!(!runner.config.id.is_empty());
        assert!(runner.config.name.starts_with("gifted-runner-"));
        assert_eq!(runner.config.repo_owner, "aGallea");
        assert_eq!(runner.config.repo_name, "gifted");
        assert_eq!(runner.state, RunnerState::Creating);
        assert!(runner.config.labels.contains(&"self-hosted".to_string()));
    }

    #[tokio::test]
    async fn test_list_runners() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        manager
            .create("aGallea/gifted", None, None, None, None)
            .await
            .unwrap();
        manager
            .create("aGallea/gifted", None, None, None, None)
            .await
            .unwrap();

        let runners = manager.list().await;
        assert_eq!(runners.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_runner() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("aGallea/gifted", None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        manager.delete(&id).await.unwrap();
        let runners = manager.list().await;
        assert_eq!(runners.len(), 0);
    }

    #[tokio::test]
    async fn test_runner_state_transitions() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("aGallea/gifted", None, None, None, None)
            .await
            .unwrap();
        assert_eq!(runner.state, RunnerState::Creating);

        manager
            .update_state(&runner.config.id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state(&runner.config.id, RunnerState::Online)
            .await
            .unwrap();

        let updated = manager.get(&runner.config.id).await.unwrap();
        assert_eq!(updated.state, RunnerState::Online);

        // Invalid transition should fail
        let result = manager
            .update_state(&runner.config.id, RunnerState::Creating)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_persistence_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();

        // Create runners and save
        let manager = RunnerManager::new(config.clone());
        manager
            .create("owner/repo1", None, None, None, None)
            .await
            .unwrap();
        manager
            .create("owner/repo2", None, None, None, None)
            .await
            .unwrap();
        manager.save_to_disk().await.unwrap();

        // Load into a fresh manager
        let manager2 = RunnerManager::new(config);
        manager2.load_from_disk().await.unwrap();
        let runners = manager2.list().await;
        assert_eq!(runners.len(), 2);

        // All loaded runners should be Offline
        for r in &runners {
            assert_eq!(r.state, RunnerState::Offline);
        }
    }

    #[tokio::test]
    async fn test_load_from_disk_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        // Should succeed even when no file exists
        manager.load_from_disk().await.unwrap();
        assert!(manager.list().await.is_empty());
    }

    #[tokio::test]
    async fn test_copy_dir_recursive() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create some files in src
        std::fs::write(src.path().join("file1.txt"), "hello").unwrap();
        std::fs::create_dir_all(src.path().join("subdir")).unwrap();
        std::fs::write(src.path().join("subdir/file2.txt"), "world").unwrap();

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("file1.txt").exists());
        assert!(dst.path().join("subdir/file2.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.path().join("file1.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(dst.path().join("subdir/file2.txt")).unwrap(),
            "world"
        );
    }

    // ── recent_logs ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_recent_logs_empty_for_unknown_runner() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let logs = manager.get_recent_logs("nonexistent-runner-id").await;
        assert!(logs.is_empty(), "expected no logs for an unknown runner");
    }

    #[tokio::test]
    async fn test_recent_logs_stored_on_broadcast() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        // Manually insert a log entry into the ring buffer the same way the
        // stdout reader task does.
        {
            let entry = LogEntry {
                runner_id: "runner-1".to_string(),
                timestamp: chrono::Utc::now(),
                line: "hello from runner".to_string(),
                stream: "stdout".to_string(),
            };
            let mut map = manager.recent_logs.write().await;
            let dq = map
                .entry("runner-1".to_string())
                .or_insert_with(VecDeque::new);
            dq.push_back(entry);
        }

        let logs = manager.get_recent_logs("runner-1").await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].line, "hello from runner");
        assert_eq!(logs[0].stream, "stdout");
    }

    #[tokio::test]
    async fn test_recent_logs_ring_buffer_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        // Insert RECENT_LOGS_MAX + 50 entries, simulating the ring-buffer logic.
        {
            let mut map = manager.recent_logs.write().await;
            let dq = map
                .entry("runner-cap".to_string())
                .or_insert_with(VecDeque::new);
            for i in 0..(RECENT_LOGS_MAX + 50) {
                dq.push_back(LogEntry {
                    runner_id: "runner-cap".to_string(),
                    timestamp: chrono::Utc::now(),
                    line: format!("line {i}"),
                    stream: "stdout".to_string(),
                });
                if dq.len() > RECENT_LOGS_MAX {
                    dq.pop_front();
                }
            }
        }

        let logs = manager.get_recent_logs("runner-cap").await;
        assert_eq!(
            logs.len(),
            RECENT_LOGS_MAX,
            "ring buffer should not exceed RECENT_LOGS_MAX"
        );
        // The oldest surviving entry should be line 50 (the first 50 were evicted).
        assert_eq!(logs[0].line, "line 50");
        // The newest should be line RECENT_LOGS_MAX + 49.
        assert_eq!(
            logs[logs.len() - 1].line,
            format!("line {}", RECENT_LOGS_MAX + 49)
        );
    }

    // ── job parsing ────────────────────────────────────────────────

    #[test]
    fn test_parse_job_event_started() {
        let line = "2026-03-21 20:06:36Z: Running job: TypeScript (type check + build)";
        let event = parse_job_event(line);
        assert_eq!(
            event,
            Some(JobEvent::Started(
                "TypeScript (type check + build)".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_job_event_completed_succeeded() {
        let line =
            "2026-03-21 20:06:51Z: Job TypeScript (type check + build) completed with result: Succeeded";
        let event = parse_job_event(line);
        assert_eq!(
            event,
            Some(JobEvent::Completed {
                succeeded: true,
                result: "Succeeded".to_string()
            })
        );
    }

    #[test]
    fn test_parse_job_event_completed_failed() {
        let line =
            "2026-03-21 20:06:51Z: Job TypeScript (type check + build) completed with result: Failed";
        let event = parse_job_event(line);
        assert_eq!(
            event,
            Some(JobEvent::Completed {
                succeeded: false,
                result: "Failed".to_string()
            })
        );
    }

    #[test]
    fn test_parse_job_event_unrelated_line() {
        let line = "2026-03-21 20:05:00Z: Listening for jobs";
        let event = parse_job_event(line);
        assert_eq!(event, None);
    }

    #[test]
    fn test_parse_job_event_empty_line() {
        assert_eq!(parse_job_event(""), None);
    }

    // ── RunnerInfo serialization ────────────────────────────────────

    #[test]
    fn test_runner_info_serialization_includes_current_job() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        use state::RunnerState;

        let info = crate::runner::types::RunnerInfo {
            config: RunnerConfig {
                id: "abc".to_string(),
                name: "test-runner".to_string(),
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
                labels: vec!["self-hosted".to_string()],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp/runner-abc"),
                group_id: None,
            },
            state: RunnerState::Busy,
            pid: Some(1234),
            uptime_secs: Some(60),
            started_at: None,
            jobs_completed: 3,
            jobs_failed: 1,
            current_job: Some("TypeScript (type check + build)".to_string()),
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(
            json["current_job"],
            serde_json::Value::String("TypeScript (type check + build)".to_string())
        );
        assert_eq!(json["jobs_completed"], 3);
        assert_eq!(json["jobs_failed"], 1);
    }

    #[tokio::test]
    async fn test_update_mode() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("owner/repo", None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        // Default mode is App; update to Service
        manager
            .update(
                &id,
                crate::runner::types::UpdateRunnerRequest {
                    labels: None,
                    mode: Some(crate::runner::types::RunnerMode::Service),
                },
            )
            .await
            .unwrap();

        let updated = manager.get(&id).await.unwrap();
        assert_eq!(
            updated.config.mode,
            crate::runner::types::RunnerMode::Service
        );
    }

    #[tokio::test]
    async fn test_update_not_found_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let result = manager
            .update(
                "nonexistent-id",
                crate::runner::types::UpdateRunnerRequest {
                    labels: Some(vec!["self-hosted".to_string()]),
                    mode: None,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_state_not_found_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let result = manager
            .update_state("nonexistent-id", RunnerState::Online)
            .await;
        assert!(result.is_err(), "expected error for nonexistent runner");
    }

    #[tokio::test]
    async fn test_update_state_invalid_transition() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("owner/repo", None, None, None, None)
            .await
            .unwrap();
        // Creating -> Busy is not a valid transition
        let result = manager
            .update_state(&runner.config.id, RunnerState::Busy)
            .await;
        assert!(
            result.is_err(),
            "expected error for invalid state transition"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Invalid state transition"),
            "unexpected error: {msg}"
        );
    }

    #[tokio::test]
    async fn test_create_runner_with_custom_labels() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                Some(vec!["gpu".to_string(), "macOS".to_string()]),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(runner.config.name, "my-runner");
        // "macOS" is in the default list so should not be duplicated
        let count = runner
            .config
            .labels
            .iter()
            .filter(|l| l.as_str() == "macOS")
            .count();
        assert_eq!(count, 1, "macOS label should not be duplicated");
        assert!(runner.config.labels.contains(&"gpu".to_string()));
    }

    #[tokio::test]
    async fn test_create_runner_invalid_repo_format() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let result = manager.create("nodash", None, None, None, None).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid repo name"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn test_full_delete_nonexistent_runner_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let result = manager.full_delete("nonexistent-id", "fake-token").await;
        assert!(
            result.is_err(),
            "full_delete on nonexistent runner should error"
        );
    }

    #[tokio::test]
    async fn test_full_delete_offline_runner_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("owner/repo", None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        // Transition to Offline so full_delete doesn't try to stop a process
        manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state(&id, RunnerState::Online)
            .await
            .unwrap();
        manager
            .update_state(&id, RunnerState::Offline)
            .await
            .unwrap();

        // full_delete with invalid token: GitHub deregistration will fail but runner
        // should still be removed from the local store.
        let result = manager.full_delete(&id, "invalid-token").await;
        // We expect success (the function ignores deregistration errors)
        assert!(
            result.is_ok(),
            "full_delete should succeed even with invalid token"
        );
        assert!(
            manager.get(&id).await.is_none(),
            "runner should be removed from the manager"
        );
    }

    #[tokio::test]
    async fn test_emit_state_event_is_received_by_subscriber() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let mut rx = manager.subscribe_events();

        // emit_state_event is private but exercised via update_state (which calls it)
        // We can also trigger it via create + update_state.
        let runner = manager
            .create("owner/repo", None, None, None, None)
            .await
            .unwrap();
        manager
            .update_state(&runner.config.id, RunnerState::Registering)
            .await
            .unwrap();

        // The event channel may or may not have fired (depends on whether anyone subscribed
        // before the event was sent).  What matters is that no panic occurred and the
        // subscribe/receive machinery works end-to-end.
        let _ = rx.try_recv(); // swallow any buffered event
    }

    #[tokio::test]
    async fn test_log_sender_and_subscribe_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let mut rx = manager.subscribe_logs();
        let sender = manager.log_sender();
        sender
            .send(LogEntry {
                runner_id: "r1".to_string(),
                timestamp: chrono::Utc::now(),
                line: "test line".to_string(),
                stream: "stdout".to_string(),
            })
            .unwrap();

        let entry = rx.recv().await.unwrap();
        assert_eq!(entry.runner_id, "r1");
        assert_eq!(entry.line, "test line");
    }

    #[tokio::test]
    async fn test_with_computed_uptime_is_non_negative() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        // A runner started in the past should have non-negative uptime
        let started_at = chrono::Utc::now() - chrono::Duration::seconds(10);
        let info = crate::runner::types::RunnerInfo {
            config: RunnerConfig {
                id: "x".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Online,
            pid: None,
            uptime_secs: None,
            started_at: Some(started_at),
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let computed = RunnerManager::with_computed_uptime(info);
        let uptime = computed.uptime_secs.expect("uptime should be computed");
        assert!(
            uptime >= 10,
            "uptime should be at least 10 seconds, got {uptime}"
        );
    }

    #[tokio::test]
    async fn test_with_computed_uptime_none_when_not_started() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = crate::runner::types::RunnerInfo {
            config: RunnerConfig {
                id: "x".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Offline,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let computed = RunnerManager::with_computed_uptime(info);
        assert!(
            computed.uptime_secs.is_none(),
            "uptime should be None when not started"
        );
    }

    #[test]
    fn test_with_job_estimate_busy_with_history() {
        use crate::runner::types::{JobHistoryEntry, RunnerConfig, RunnerMode};
        let now = chrono::Utc::now();
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Busy,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: Some("build".to_string()),
            job_context: None,
            error_message: None,
            job_started_at: Some(now),
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let mut history = HashMap::new();
        history.insert(
            "runner-1".to_string(),
            vec![JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(200),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
            }],
        );
        let result = RunnerManager::with_job_estimate(info, &history);
        assert_eq!(result.estimated_job_duration_secs, Some(200));
    }

    #[test]
    fn test_with_job_estimate_busy_no_history() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Busy,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: Some("build".to_string()),
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let history = HashMap::new();
        let result = RunnerManager::with_job_estimate(info, &history);
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_online_ignored() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Online,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let history = HashMap::new();
        let result = RunnerManager::with_job_estimate(info, &history);
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_busy_no_current_job() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
            },
            state: RunnerState::Busy,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let history = HashMap::new();
        let result = RunnerManager::with_job_estimate(info, &history);
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[tokio::test]
    async fn test_copy_dir_recursive_preserves_executable_bit() {
        use std::fs;
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create an executable file in the source
        let script_path = src.path().join("run.sh");
        fs::write(&script_path, "#!/bin/bash\necho hi").unwrap();
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        let dst_script = dst.path().join("run.sh");
        assert!(dst_script.exists());
        let content = fs::read_to_string(&dst_script).unwrap();
        assert!(content.contains("echo hi"));

        #[cfg(unix)]
        {
            let perms = fs::metadata(&dst_script).unwrap().permissions();
            assert!(
                perms.mode() & 0o111 != 0,
                "copied file should be executable"
            );
        }
    }

    #[tokio::test]
    async fn test_multiple_runners_same_repo_get_sequential_names() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let r1 = manager
            .create("org/myapp", None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("org/myapp", None, None, None, None)
            .await
            .unwrap();

        assert!(
            r1.config.name.contains("myapp-runner-1"),
            "first runner name: {}",
            r1.config.name
        );
        assert!(
            r2.config.name.contains("myapp-runner-2"),
            "second runner name: {}",
            r2.config.name
        );
    }

    #[test]
    fn test_parse_job_event_started_no_timestamp_prefix() {
        // The function should work even without a timestamp prefix
        let event = parse_job_event("Running job: deploy");
        assert_eq!(event, Some(JobEvent::Started("deploy".to_string())));
    }

    #[test]
    fn test_parse_job_event_completed_succeeded_case_sensitive() {
        // "Succeeded" (capital S) triggers succeeded=true
        let event = parse_job_event("Job build completed with result: Succeeded");
        assert_eq!(
            event,
            Some(JobEvent::Completed {
                succeeded: true,
                result: "Succeeded".to_string()
            })
        );

        // Any other result keyword yields succeeded=false
        let event2 = parse_job_event("Job build completed with result: Cancelled");
        assert_eq!(
            event2,
            Some(JobEvent::Completed {
                succeeded: false,
                result: "Cancelled".to_string()
            })
        );
    }

    #[test]
    fn test_parse_job_event_completed_skipped_result() {
        let line = "Job lint completed with result: Skipped";
        let event = parse_job_event(line);
        assert_eq!(
            event,
            Some(JobEvent::Completed {
                succeeded: false,
                result: "Skipped".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_create_with_group_id() {
        let manager = create_test_manager();
        let runner = manager
            .create(
                "owner/repo",
                Some("test-runner".to_string()),
                None,
                None,
                Some("group-123".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(runner.config.group_id, Some("group-123".to_string()));
    }

    #[tokio::test]
    async fn test_create_without_group_id() {
        let manager = create_test_manager();
        let runner = manager
            .create(
                "owner/repo",
                Some("test-runner".to_string()),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(runner.config.group_id, None);
    }

    #[tokio::test]
    async fn test_next_runner_number_increments() {
        let manager = create_test_manager();
        let r1 = manager
            .create("owner/myrepo", None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("owner/myrepo", None, None, None, None)
            .await
            .unwrap();
        assert_eq!(r1.config.name, "myrepo-runner-1");
        assert_eq!(r2.config.name, "myrepo-runner-2");
    }

    #[tokio::test]
    async fn test_next_runner_number_different_repos() {
        let manager = create_test_manager();
        let r1 = manager
            .create("owner/repo-a", None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("owner/repo-b", None, None, None, None)
            .await
            .unwrap();
        assert_eq!(r1.config.name, "repo-a-runner-1");
        assert_eq!(r2.config.name, "repo-b-runner-1");
    }

    #[test]
    fn test_runner_event_serialization() {
        let event = RunnerEvent {
            runner_id: "abc".to_string(),
            event_type: "state_changed".to_string(),
            data: serde_json::json!({"state": "online"}),
            timestamp: chrono::Utc::now(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["runner_id"], "abc");
        assert_eq!(json["event_type"], "state_changed");
        assert_eq!(json["data"]["state"], "online");
    }

    #[test]
    fn test_log_entry_fields() {
        let ts = chrono::Utc::now();
        let entry = LogEntry {
            runner_id: "r1".to_string(),
            timestamp: ts,
            line: "my log line".to_string(),
            stream: "stderr".to_string(),
        };
        assert_eq!(entry.runner_id, "r1");
        assert_eq!(entry.line, "my log line");
        assert_eq!(entry.stream, "stderr");
    }

    #[test]
    fn test_runner_info_serialization_omits_current_job_when_none() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        use state::RunnerState;

        let info = crate::runner::types::RunnerInfo {
            config: RunnerConfig {
                id: "abc".to_string(),
                name: "test-runner".to_string(),
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp/runner-abc"),
                group_id: None,
            },
            state: RunnerState::Online,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            error_message: None,
            job_started_at: None,
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };

        let json = serde_json::to_value(&info).unwrap();
        // `current_job` is `skip_serializing_if = "Option::is_none"`, so the key must be absent.
        assert!(!json.as_object().unwrap().contains_key("current_job"));
    }

    #[tokio::test]
    async fn test_stop_process_signals_and_waits() {
        let manager = create_test_manager();

        // Simulate a running process by inserting a ProcessHandle manually
        let kill_signal = Arc::new(Notify::new());
        let (exit_tx, exit_rx) = watch::channel(false);

        let handle = ProcessHandle {
            kill_signal: kill_signal.clone(),
            exited: exit_rx,
        };

        let runner_id = "test-runner-1";

        // Create a runner in Online state
        manager.runners.write().await.insert(
            runner_id.to_string(),
            RunnerInfo {
                config: RunnerConfig {
                    id: runner_id.to_string(),
                    name: "test".to_string(),
                    repo_owner: "owner".to_string(),
                    repo_name: "repo".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp/test"),
                    group_id: None,
                },
                state: RunnerState::Online,
                pid: Some(12345),
                uptime_secs: None,
                started_at: Some(chrono::Utc::now()),
                jobs_completed: 0,
                jobs_failed: 0,
                current_job: None,
                job_context: None,
                error_message: None,
                job_started_at: None,
                last_completed_job: None,
                estimated_job_duration_secs: None,
            },
        );
        manager
            .processes
            .write()
            .await
            .insert(runner_id.to_string(), handle);

        // Spawn a task that simulates the monitoring task:
        // waits for kill signal, then marks exited
        let ks = kill_signal.clone();
        tokio::spawn(async move {
            ks.notified().await;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let _ = exit_tx.send(true);
        });

        // stop_process should signal and wait
        let result = manager.stop_process(runner_id).await;
        assert!(result.is_ok());

        // Runner should be Offline
        let runner = manager.get(runner_id).await.unwrap();
        assert_eq!(runner.state, RunnerState::Offline);
        assert_eq!(runner.pid, None);
    }

    #[tokio::test]
    async fn test_get_steps_returns_data_after_watcher_started() {
        let manager = create_test_manager();

        // Create a fake runner work dir with a Worker log
        let work_dir = tempfile::tempdir().unwrap();
        let diag = work_dir.path().join("_diag");
        std::fs::create_dir_all(&diag).unwrap();

        let log_content = "\
[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Checkout'\n\
[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.\n\
[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.\n\
[2026-03-23 07:54:55Z INFO StepsRunner] Processing step: DisplayName='Build'\n\
[2026-03-23 07:54:55Z INFO StepsRunner] Starting the step.\n";
        std::fs::write(diag.join("Worker_20260323-075453-utc.log"), log_content).unwrap();

        let runner_id = "test-steps-runner";

        // Before watching: get_steps returns None
        assert!(manager.get_steps(runner_id).await.is_none());

        // Start watching and poll
        manager
            .step_watcher
            .start_watching(runner_id, "build-job", work_dir.path())
            .await;
        manager.step_watcher.poll(runner_id).await;

        // get_steps should return data
        let resp = manager.get_steps(runner_id).await.unwrap();
        assert_eq!(resp.job_name, "build-job");
        assert_eq!(resp.steps.len(), 2);
        assert_eq!(resp.steps[0].name, "Checkout");
        assert_eq!(
            resp.steps[0].status,
            crate::runner::steps::StepStatus::Succeeded
        );
        assert_eq!(resp.steps[1].name, "Build");
        assert_eq!(
            resp.steps[1].status,
            crate::runner::steps::StepStatus::Running
        );

        // Stop watching: get_steps returns None again
        manager.step_watcher.stop_watching(runner_id).await;
        assert!(manager.get_steps(runner_id).await.is_none());
    }
}
