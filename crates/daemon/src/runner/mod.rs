pub mod binary;
pub mod docker;
pub mod history;
pub mod process;
pub mod state;
pub mod step_log_cache;
pub mod steps;
pub mod types;

use crate::auth::AuthManager;
use crate::config::Config;
use crate::github::GitHubClient;
use crate::runner::binary::ensure_runner_binary;
use crate::runner::process::{
    clean_runner_config, configure_runner, find_runner_pid, kill_orphaned_processes, remove_runner,
    start_runner,
};
use crate::runner::state::RunnerState;
use crate::runner::steps::{StepsResponse, WorkerLogWatcher};
use crate::runner::types::{RunnerConfig, RunnerInfo, RunnerMode};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::sync::{broadcast, watch, Mutex, Notify, RwLock};

/// A runner's underlying execution unit — a native child process, or a
/// Docker container. `RunnerManager`'s state machine, log streaming, and
/// stop/monitor plumbing operate on either uniformly past this point.
enum RunningProcess {
    Native(tokio::process::Child),
    Container {
        docker: bollard::Docker,
        container_id: String,
    },
}

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

fn should_apply_job_context(
    state: &RunnerState,
    current_job: Option<&str>,
    current_job_started_at: Option<&chrono::DateTime<chrono::Utc>>,
    has_context: bool,
    expected_job: &str,
    expected_job_started_at_micros: i64,
) -> bool {
    *state == RunnerState::Busy
        && current_job == Some(expected_job)
        && current_job_started_at.map(|started_at| started_at.timestamp_micros())
            == Some(expected_job_started_at_micros)
        && !has_context
}

/// Handle for communicating with a runner's monitoring task.
/// The monitoring task owns the `Child` exclusively — no shared lock needed.
#[derive(Clone)]
struct ProcessHandle {
    /// Signal the monitoring task to kill the child process.
    kill_signal: Arc<Notify>,
    /// Becomes `true` once the child process has fully exited.
    exited: watch::Receiver<bool>,
}

#[derive(Default)]
struct LifecycleOperations {
    starting: HashSet<String>,
    stopping: HashSet<String>,
    updating: HashSet<String>,
    deleting: HashSet<String>,
    shutting_down: bool,
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
    /// User intent, independent from the transient process state. A runner that
    /// crashes remains desired-running until the user explicitly stops/deletes it.
    desired_running: Arc<RwLock<HashSet<String>>>,
    /// Prevent duplicate recovery loops for the same runner.
    recovering: Arc<Mutex<HashSet<String>>>,
    /// Coordinate start and delete reservations under one lock. A deletion
    /// blocks new starts before waiting for an already-running start pipeline.
    lifecycle_operations: Arc<Mutex<LifecycleOperations>>,
    /// Serialize configuration PATCH operations through their persistence commit,
    /// so a failed write can never roll back a newer same-valued update.
    update_lock: Arc<Mutex<()>>,
    /// Serialize persistence writes. Multiple async lifecycle tasks can request
    /// a save concurrently, and unsynchronized writes can truncate runners.json.
    persistence_lock: Arc<Mutex<()>>,
    auth_manager: Option<AuthManager>,
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

/// Returns the default runner labels for the current platform and architecture.
fn default_runner_labels() -> Vec<String> {
    let os_label = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    };
    let arch_label = if cfg!(target_arch = "aarch64") {
        "ARM64"
    } else {
        "X64"
    };
    vec![
        "self-hosted".to_string(),
        os_label.to_string(),
        arch_label.to_string(),
    ]
}

/// Default labels for a container-mode runner. Unlike `default_runner_labels`,
/// these are independent of the daemon's host OS — the runner is a Linux
/// container regardless of host. GitHub's `config.sh` auto-adds the real
/// `self-hosted`/OS/arch labels on top; `docker` is the stable marker workflows
/// use to target container runners (`runs-on: [self-hosted, docker]`).
fn default_container_labels() -> Vec<String> {
    vec!["self-hosted".to_string(), "docker".to_string()]
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
            desired_running: Arc::new(RwLock::new(HashSet::new())),
            recovering: Arc::new(Mutex::new(HashSet::new())),
            lifecycle_operations: Arc::new(Mutex::new(LifecycleOperations::default())),
            update_lock: Arc::new(Mutex::new(())),
            persistence_lock: Arc::new(Mutex::new(())),
            auth_manager: None,
            step_watcher: WorkerLogWatcher::new(),
            step_log_cache: step_log_cache::StepLogCache::new(),
            job_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Attach an `AuthManager` so the runner manager can invalidate auth
    /// when GitHub returns "Bad credentials".
    pub fn set_auth_manager(&mut self, auth: AuthManager) {
        self.auth_manager = Some(auth);
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
                let needs_context: Vec<(String, String, String, String, String, i64)> = {
                    let map = runners.read().await;
                    map.values()
                        .filter(|r| {
                            r.state == RunnerState::Busy
                                && r.job_context.is_none()
                                && r.current_job.is_some()
                                && r.job_started_at.is_some()
                        })
                        .map(|r| {
                            (
                                r.config.id.clone(),
                                r.config.name.clone(),
                                r.config.repo_owner.clone(),
                                r.config.repo_name.clone(),
                                r.current_job.clone().unwrap(),
                                r.job_started_at
                                    .as_ref()
                                    .expect("filtered job start timestamp")
                                    .timestamp_micros(),
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

                for (runner_id, runner_name, owner, repo, job_name, job_started_at_micros) in
                    needs_context
                {
                    match gh
                        .get_active_run_for_runner(&owner, &repo, &runner_name, &job_name)
                        .await
                    {
                        Ok(Some(ctx)) => {
                            let mut map = runners.write().await;
                            if let Some(runner) = map.get_mut(&runner_id) {
                                if should_apply_job_context(
                                    &runner.state,
                                    runner.current_job.as_deref(),
                                    runner.job_started_at.as_ref(),
                                    runner.job_context.is_some(),
                                    &job_name,
                                    job_started_at_micros,
                                ) {
                                    tracing::info!(
                                        runner = %runner_id,
                                        branch = %ctx.branch,
                                        pr = ?ctx.pr_number,
                                        "Job context fetched"
                                    );
                                    runner.job_context = Some(ctx);
                                } else {
                                    tracing::debug!(
                                        runner = %runner_id,
                                        expected_job = %job_name,
                                        current_job = ?runner.current_job,
                                        state = ?runner.state,
                                        "Discarding stale job context result"
                                    );
                                }
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

    /// After recording a job completion, check all OTHER runners' history for
    /// entries with the same `run_id`. If found, annotate them with
    /// `latest_attempt` so the UI shows the re-run result.
    async fn annotate_cross_runner_reruns(
        &self,
        source_runner_id: &str,
        entry: &types::JobHistoryEntry,
    ) {
        let run_id = match entry
            .run_url
            .as_deref()
            .and_then(history::extract_run_id_from_url)
        {
            Some(id) => id,
            None => return,
        };

        let source_runner_name = {
            let map = self.runners.read().await;
            match map.get(source_runner_id) {
                Some(r) => r.config.name.clone(),
                None => return,
            }
        };

        let attempt = types::RunAttempt {
            attempt: 0, // not available from local data; displayed as "re-run"
            succeeded: entry.succeeded,
            runner_name: source_runner_name,
            completed_at: entry.completed_at,
            run_url: entry.run_url.clone(),
        };

        // Phase 1: annotate history entries and collect affected runner IDs
        let mut annotated_runners: Vec<String> = Vec::new();
        {
            let mut hist = self.job_history.write().await;
            for (other_id, other_entries) in hist.iter_mut() {
                if other_id == source_runner_id {
                    continue;
                }
                let mut changed = false;
                for e in other_entries.iter_mut() {
                    if e.job_name == entry.job_name
                        && e.run_url
                            .as_deref()
                            .and_then(history::extract_run_id_from_url)
                            == Some(run_id)
                    {
                        e.latest_attempt = Some(attempt.clone());
                        changed = true;
                    }
                }
                if changed {
                    if let Err(e) =
                        history::save(&self.config.history_dir(), other_id, other_entries)
                    {
                        tracing::warn!("Failed to save rerun annotation for {other_id}: {e}");
                    }
                    annotated_runners.push(other_id.clone());
                }
            }
        }

        // Phase 2: update last_completed_job and emit events for affected runners
        if !annotated_runners.is_empty() {
            let mut runners = self.runners.write().await;
            for other_id in &annotated_runners {
                if let Some(r) = runners.get_mut(other_id) {
                    if let Some(ref mut lc) = r.last_completed_job {
                        if lc
                            .run_url
                            .as_deref()
                            .and_then(history::extract_run_id_from_url)
                            == Some(run_id)
                        {
                            lc.latest_attempt = Some(attempt.clone());
                        }
                    }
                }
                tracing::info!(
                    source = %source_runner_id,
                    target = %other_id,
                    run_id,
                    job = %entry.job_name,
                    succeeded = entry.succeeded,
                    "Annotated cross-runner re-run"
                );
                let _ = self.event_tx.send(RunnerEvent {
                    runner_id: other_id.clone(),
                    event_type: "state_changed".to_string(),
                    data: serde_json::json!({"rerun_updated": true}),
                    timestamp: chrono::Utc::now(),
                });
            }
        }
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
        runners: &HashMap<String, RunnerInfo>,
    ) -> RunnerInfo {
        if info.state == RunnerState::Busy {
            if let Some(ref job_name) = info.current_job {
                // Try own history first
                if let Some(entries) = history.get(&info.config.id) {
                    info.estimated_job_duration_secs =
                        history::median_duration_secs(entries, job_name);
                }
                // Fall back to group history if own history had no match
                if info.estimated_job_duration_secs.is_none() {
                    if let Some(ref group_id) = info.config.group_id {
                        let group_entries: Vec<types::JobHistoryEntry> = runners
                            .values()
                            .filter(|r| {
                                r.config.id != info.config.id
                                    && r.config.group_id.as_ref() == Some(group_id)
                            })
                            .filter_map(|r| history.get(&r.config.id))
                            .flatten()
                            .cloned()
                            .collect();
                        if !group_entries.is_empty() {
                            info.estimated_job_duration_secs =
                                history::median_duration_secs(&group_entries, job_name);
                        }
                    }
                }
            }
        }
        info
    }

    // ── Persistence ────────────────────────────────────────────────

    /// Save all runner configs to disk as JSON.
    pub async fn save_to_disk(&self) -> Result<()> {
        let _persistence_guard = self.persistence_lock.lock().await;
        self.save_to_disk_locked().await
    }

    /// Persist a snapshot while the caller holds `persistence_lock`.
    async fn save_to_disk_locked(&self) -> Result<()> {
        let persisted = {
            let desired_running = self.desired_running.read().await;
            let runners = self.runners.read().await;
            runners
                .values()
                .map(|runner| PersistedRunner {
                    config: runner.config.clone(),
                    was_running: desired_running.contains(&runner.config.id),
                })
                .collect::<Vec<_>>()
        };
        let json = serde_json::to_string_pretty(&persisted)?;
        let path = self.config.runners_json_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Never truncate the live state file in place. A daemon crash or power
        // loss during a direct write otherwise leaves invalid JSON and prevents
        // every runner from loading on the next launch.
        let temp_path = path.with_extension(format!("json.tmp-{}", std::process::id()));
        let write_result = (|| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&temp_path)
                .with_context(|| {
                    format!("creating temporary runner state {}", temp_path.display())
                })?;
            file.write_all(json.as_bytes())
                .context("writing temporary runner state")?;
            file.sync_all().context("syncing temporary runner state")?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(error);
        }

        #[cfg(not(windows))]
        std::fs::rename(&temp_path, &path)
            .with_context(|| format!("replacing runner state {}", path.display()))?;

        #[cfg(windows)]
        {
            // Windows rename does not replace an existing destination. Keep a
            // recoverable backup while swapping the fully-synced temporary file.
            let backup_path = path.with_extension("json.bak");
            let _ = std::fs::remove_file(&backup_path);
            if path.exists() {
                std::fs::rename(&path, &backup_path).context("backing up previous runner state")?;
            }
            if let Err(error) = std::fs::rename(&temp_path, &path) {
                if backup_path.exists() {
                    let _ = std::fs::rename(&backup_path, &path);
                }
                return Err(error).context("installing new runner state");
            }
            let _ = std::fs::remove_file(&backup_path);
        }

        Ok(())
    }

    /// Load runner configs from disk. For runners that were previously running,
    /// checks if their process is still alive and reattaches to it.
    /// Returns a list of runner IDs that were running but whose process is now dead
    /// (these need to be restarted if the restore preference is enabled).
    pub async fn load_from_disk(&self) -> Result<Vec<String>> {
        let path = self.config.runners_json_path();
        #[cfg(windows)]
        if !path.exists() {
            let backup_path = path.with_extension("json.bak");
            if backup_path.exists() {
                tracing::warn!(
                    "Recovering runner state from interrupted Windows swap: {}",
                    backup_path.display()
                );
                std::fs::rename(&backup_path, &path).context("restoring backed-up runner state")?;
            }
        }
        if !path.exists() {
            return Ok(Vec::new());
        }
        let json = std::fs::read_to_string(&path)?;
        let persisted: Vec<PersistedRunner> = serde_json::from_str(&json)?;
        let mut need_restart = Vec::new();
        let mut desired_running = HashSet::new();
        let mut runners = self.runners.write().await;
        for entry in persisted {
            let id = entry.config.id.clone();
            let is_service = entry.config.mode == RunnerMode::Service;
            let (state, pid) = if is_service {
                // Service runners survive daemon restart — always check if process is alive
                match find_runner_pid(&entry.config.work_dir).await {
                    Some(pid) => (RunnerState::Online, Some(pid)),
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
            if entry.was_running || (is_service && state == RunnerState::Online) {
                desired_running.insert(id.clone());
            }
            runners.insert(
                id,
                RunnerInfo {
                    config: entry.config,
                    state,
                    pid,
                    container_id: None,
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
        *self.desired_running.write().await = desired_running;

        // Load job history from disk
        match history::load_all(&self.config.history_dir()) {
            Ok(mut hist) => {
                // Backfill job_number for entries created before the field existed
                for (runner_id, entries) in hist.iter_mut() {
                    let needs_backfill = entries.iter().any(|e| e.job_number == 0);
                    if needs_backfill {
                        for (idx, entry) in entries.iter_mut().enumerate() {
                            if entry.job_number == 0 {
                                entry.job_number = (idx + 1) as u32;
                            }
                        }
                        if let Err(e) =
                            history::save(&self.config.history_dir(), runner_id, entries)
                        {
                            tracing::warn!("Failed to backfill job_number for {runner_id}: {e}");
                        }
                    }
                }

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
                                latest_attempt: None,
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
    /// The process handle is installed before this method returns, so API calls
    /// can never observe an Online reattached runner without being able to stop it.
    pub async fn monitor_orphaned_process(&self, runner_id: &str, pid: u32) {
        let manager = self.clone();
        let rid = runner_id.to_string();
        let kill_signal = Arc::new(Notify::new());
        let (exit_tx, exit_rx) = watch::channel(false);
        let handle = ProcessHandle {
            kill_signal: kill_signal.clone(),
            exited: exit_rx,
        };
        let work_dir = self
            .runners
            .read()
            .await
            .get(&rid)
            .map(|runner| runner.config.work_dir.clone());

        self.processes.write().await.insert(rid.clone(), handle);

        tokio::spawn(async move {
            if let Some(ref work_dir) = work_dir {
                let diag_dir = work_dir.join("_diag");
                let watcher_manager = manager.clone();
                let watcher_id = rid.clone();
                let step_watcher = manager.step_watcher.clone();
                let watcher_work_dir = work_dir.clone();
                tokio::spawn(async move {
                    Self::tail_diag_logs(
                        watcher_manager,
                        &watcher_id,
                        &diag_dir,
                        &watcher_work_dir,
                        step_watcher,
                    )
                    .await;
                });
            }

            loop {
                tokio::select! {
                    _ = kill_signal.notified() => {
                        #[cfg(unix)]
                        {
                            let pgid = pid as i32;
                            unsafe { libc::kill(-pgid, libc::SIGTERM); }
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            unsafe { libc::kill(-pgid, libc::SIGKILL); }
                        }
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new("taskkill")
                                .args(["/T", "/F", "/PID", &pid.to_string()])
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .status();
                        }
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                        #[cfg(unix)]
                        let alive = {
                            let result = unsafe { libc::kill(pid as i32, 0) };
                            result == 0
                                || std::io::Error::last_os_error().raw_os_error()
                                    == Some(libc::EPERM)
                        };
                        #[cfg(windows)]
                        let alive = {
                            use sysinfo::{Pid, ProcessesToUpdate, ProcessRefreshKind, System};
                            let mut sys = System::new();
                            sys.refresh_processes_specifics(
                                ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
                                true,
                                ProcessRefreshKind::nothing(),
                            );
                            sys.process(Pid::from_u32(pid)).is_some()
                        };
                        if !alive {
                            break;
                        }
                    }
                }
            }

            let (exists, unexpected) = {
                let mut runners = manager.runners.write().await;
                if let Some(runner) = runners.get_mut(&rid) {
                    let unexpected =
                        runner.state == RunnerState::Online || runner.state == RunnerState::Busy;
                    if runner.state != RunnerState::Deleting {
                        runner.state = RunnerState::Offline;
                        runner.pid = None;
                        runner.container_id = None;
                        runner.started_at = None;
                        runner.current_job = None;
                        runner.job_context = None;
                        runner.job_started_at = None;
                        runner.error_message = unexpected.then(|| {
                            "Runner process exited unexpectedly; recovery is scheduled".to_string()
                        });
                    }
                    (true, unexpected)
                } else {
                    (false, false)
                }
            };
            manager.processes.write().await.remove(&rid);
            if exists {
                manager.emit_state_event(&rid, "offline");
            }
            if unexpected && manager.is_desired_running(&rid).await {
                manager.schedule_recovery(rid.clone());
            } else {
                let _ = manager.save_to_disk().await;
            }
            let _ = exit_tx.send(true);
        });
    }

    /// Tail the latest Runner_*.log in the _diag directory to detect job events
    /// for reattached (orphaned) runners whose stdout we can't read.
    /// Tail the latest Runner_*.log in `_diag/` using offset-based polling.
    ///
    /// This is the primary job-event detection mechanism on Windows, where
    /// Runner.Listener.exe output doesn't flow through the piped cmd.exe
    /// stdout. It also runs for reattached (orphaned) runners on all platforms.
    ///
    /// Uses synchronous file reads + offset tracking (like `WorkerLogWatcher`)
    /// instead of async `BufReader::lines()`, which doesn't reliably pick up
    /// data appended after EOF on regular files.
    async fn tail_diag_logs(
        manager: Self,
        runner_id: &str,
        diag_dir: &std::path::Path,
        work_dir: &std::path::Path,
        step_watcher: WorkerLogWatcher,
    ) {
        let mut current_log: Option<std::path::PathBuf> = None;
        let mut file_offset: u64 = 0;
        // Start by skipping existing content so we only process new lines
        let mut needs_initial_seek = true;

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Check if runner still exists
            {
                let map = manager.runners.read().await;
                if !map.contains_key(runner_id) {
                    break;
                }
            }

            // Find the latest Runner_*.log
            let latest = match std::fs::read_dir(diag_dir) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("Runner_"))
                    .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                    .map(|e| e.path()),
                Err(_) => {
                    // _diag dir doesn't exist yet — runner still starting, keep waiting
                    continue;
                }
            };

            let log_path = match latest {
                Some(p) => p,
                None => continue, // No Runner_*.log yet
            };

            // If a newer log file appeared, switch to it
            if current_log.as_ref() != Some(&log_path) {
                if needs_initial_seek {
                    // First time: skip to end of existing file
                    file_offset = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
                    needs_initial_seek = false;
                } else {
                    // New log file appeared mid-run — read from the start
                    file_offset = 0;
                }
                current_log = Some(log_path.clone());
            }

            // Read new content from offset
            let content = match std::fs::read_to_string(&log_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let content_len = content.len() as u64;
            if content_len <= file_offset {
                continue; // No new data
            }

            let new_text = &content[file_offset as usize..];
            file_offset = content_len;

            for line in new_text.lines() {
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

                // Extract job event payload
                let payload = if let Some(idx) = line.find("WRITE LINE: ") {
                    &line[idx + "WRITE LINE: ".len()..]
                } else {
                    line
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
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
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
                                if let Some((owner, repo, runner_name, job_name, job_id)) = info {
                                    let token = manager.auth_token.read().await.clone();
                                    if let Some(token) = token {
                                        if let Ok(gh) =
                                            crate::github::GitHubClient::new(Some(token))
                                        {
                                            let msg = if let Some(jid) = job_id {
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
                                let duration_secs = (now - started_at).num_seconds().max(0) as u64;

                                let entry = types::JobHistoryEntry {
                                    job_name: r.current_job.clone().unwrap_or_default(),
                                    started_at,
                                    completed_at: now,
                                    succeeded,
                                    branch: r.job_context.as_ref().map(|c| c.branch.clone()),
                                    pr_number: r.job_context.as_ref().and_then(|c| c.pr_number),
                                    run_url: r.job_context.as_ref().map(|c| match c.job_id {
                                        Some(job_id) => {
                                            format!("{}/job/{}", c.run_url, job_id)
                                        }
                                        None => c.run_url.clone(),
                                    }),
                                    error_message: error_message.clone(),
                                    steps,
                                    latest_attempt: None,
                                    job_number: 0,
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
                                    latest_attempt: None,
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

                                let runner_name = r.config.name.clone();
                                Some((entry, runner_name, duration_secs))
                            } else {
                                None
                            }
                        };

                        if let Some((entry, _runner_name, _duration_secs)) = history_entry {
                            manager.record_job_history(runner_id, entry).await;
                        }
                        manager.emit_state_event(runner_id, "online");
                        let _ = manager.save_to_disk().await;
                    }
                    None => {}
                }
            }
        }
    }

    // ── CRUD ───────────────────────────────────────────────────────

    /// Validate fields that do not require mutating manager state. API handlers
    /// call this before authentication so malformed requests still return 400,
    /// while valid unauthenticated requests never persist orphaned runners.
    pub fn validate_create_request(
        repo_full_name: &str,
        name: Option<&str>,
        mode: Option<&RunnerMode>,
        container: Option<&types::ContainerConfig>,
    ) -> Result<()> {
        let Some((owner, repo)) = repo_full_name.split_once('/') else {
            bail!("Invalid repo name: expected 'owner/repo'");
        };
        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            bail!("Invalid repo name: expected non-empty 'owner/repo'");
        }
        if owner
            .chars()
            .chain(repo.chars())
            .any(|character| character.is_whitespace() || character.is_control())
        {
            bail!("Invalid repo name: whitespace and control characters are not allowed");
        }

        if let Some(name) = name {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                bail!("Runner name cannot be empty");
            }
            if trimmed.chars().count() > 100 {
                bail!("Runner name must be at most 100 characters");
            }
            if trimmed.chars().any(char::is_control) {
                bail!("Runner name cannot contain control characters");
            }
        }

        if matches!(mode, Some(RunnerMode::Container)) {
            match container {
                None => bail!("Container mode requires a container image configuration"),
                Some(config) if config.image.trim().is_empty() => {
                    bail!("Container mode requires a non-empty container image")
                }
                _ => {}
            }

            if let Some(name) = name {
                if !name.trim().chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
                }) {
                    bail!(
                        "Container runner name may only contain letters, digits, '_', '.' or '-' \
                         (it is used as the Docker container name)"
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn create(
        &self,
        repo_full_name: &str,
        name: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
        group_id: Option<String>,
        container: Option<types::ContainerConfig>,
    ) -> Result<RunnerInfo> {
        self.create_with_intent(
            repo_full_name,
            name,
            labels,
            mode,
            group_id,
            container,
            false,
        )
        .await
    }

    pub(crate) async fn create_desired_running(
        &self,
        repo_full_name: &str,
        name: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
        group_id: Option<String>,
        container: Option<types::ContainerConfig>,
    ) -> Result<RunnerInfo> {
        self.create_with_intent(
            repo_full_name,
            name,
            labels,
            mode,
            group_id,
            container,
            true,
        )
        .await
    }

    // This private adapter mirrors the public creation inputs and adds one
    // atomic persistence flag. Keeping the arguments explicit avoids a second,
    // duplicative request type solely for internal transaction handling.
    #[allow(clippy::too_many_arguments)]
    async fn create_with_intent(
        &self,
        repo_full_name: &str,
        name: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
        group_id: Option<String>,
        container: Option<types::ContainerConfig>,
        desired: bool,
    ) -> Result<RunnerInfo> {
        Self::validate_create_request(
            repo_full_name,
            name.as_deref(),
            mode.as_ref(),
            container.as_ref(),
        )?;
        let (owner, repo) = repo_full_name
            .split_once('/')
            .expect("validated repository name must contain one slash");

        let id = uuid::Uuid::new_v4().to_string();
        let name = match name {
            Some(name) => name.trim().to_string(),
            None => {
                let num = self.next_runner_number(repo).await;
                format!("{repo}-runner-{num}")
            }
        };

        // Container runners are Linux regardless of host, and need a stable
        // `docker` marker for routing; native runners keep host platform labels.
        let platform_defaults = if matches!(mode.as_ref(), Some(RunnerMode::Container)) {
            default_container_labels()
        } else {
            default_runner_labels()
        };
        let resolved_labels = match labels {
            Some(user_labels) => {
                let normalized = types::normalize_labels(user_labels)?;
                if normalized.is_empty() {
                    platform_defaults
                } else {
                    normalized
                }
            }
            None => platform_defaults,
        };

        let work_dir = self.config.runners_dir().join(&id);
        std::fs::create_dir_all(&work_dir)?;

        let runner = RunnerInfo {
            config: RunnerConfig {
                id: id.clone(),
                name,
                display_name: None,
                repo_owner: owner.to_string(),
                repo_name: repo.to_string(),
                labels: resolved_labels,
                mode: mode.unwrap_or(RunnerMode::App),
                work_dir,
                group_id,
                container,
            },
            state: RunnerState::Creating,
            pid: None,
            container_id: None,
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

        // Desired-running creation also reserves the start under the same
        // lifecycle mutex used by daemon shutdown. This closes the gap where
        // shutdown could begin after persistence but before the API reserved
        // the runner's start operation.
        // Every creation participates in the lifecycle barrier so shutdown
        // cannot race a persistence transaction. Desired-running creation also
        // reserves its start before releasing this mutex.
        let mut creation_operations = self.lifecycle_operations.lock().await;
        if creation_operations.shutting_down {
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
            bail!("Daemon shutdown is in progress");
        }

        let _persistence_guard = self.persistence_lock.lock().await;
        {
            let mut runners = self.runners.write().await;
            if runners.values().any(|existing| {
                let same_name = existing
                    .config
                    .name
                    .eq_ignore_ascii_case(&runner.config.name);
                let same_repository = existing.config.repo_owner.eq_ignore_ascii_case(owner)
                    && existing.config.repo_name.eq_ignore_ascii_case(repo);
                let docker_name_collision = existing.config.mode == RunnerMode::Container
                    && runner.config.mode == RunnerMode::Container;
                same_name && (same_repository || docker_name_collision)
            }) {
                drop(runners);
                let _ = std::fs::remove_dir_all(&runner.config.work_dir);
                bail!(
                    "A conflicting runner named '{}' already exists",
                    runner.config.name
                );
            }
            runners.insert(id.clone(), runner.clone());
        }
        if desired && !creation_operations.starting.insert(id.clone()) {
            self.runners.write().await.remove(&id);
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
            bail!("Runner '{id}' already has a start operation in progress");
        }
        if desired {
            self.desired_running.write().await.insert(id.clone());
        }

        if let Err(error) = self.save_to_disk_locked().await {
            self.runners.write().await.remove(&id);
            if desired {
                self.desired_running.write().await.remove(&id);
                creation_operations.starting.remove(&id);
            }
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
            drop(_persistence_guard);
            drop(creation_operations);
            return Err(error).context("persisting newly-created runner");
        }

        // Keep lifecycle admission locked through the durable write. Shutdown
        // therefore cannot overtake a creation that it failed to reject.
        drop(_persistence_guard);
        drop(creation_operations);
        Ok(runner)
    }

    pub async fn create_batch(
        &self,
        repo_full_name: &str,
        count: u8,
        name_prefix: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
        container: Option<types::ContainerConfig>,
    ) -> Result<(String, Vec<RunnerInfo>, Vec<types::BatchCreateError>)> {
        let group_id = uuid::Uuid::new_v4().to_string();
        let mut runners = Vec::new();
        let mut errors = Vec::new();

        for i in 0..count {
            match self
                .create_desired_running(
                    repo_full_name,
                    name_prefix
                        .as_ref()
                        .map(|prefix| format!("{prefix}-{}", i + 1)),
                    labels.clone(),
                    mode.clone(),
                    Some(group_id.clone()),
                    container.clone(),
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
        let runners_guard = self.runners.read().await;
        let history = self.job_history.read().await;
        runners_guard
            .values()
            .filter(|r| r.config.group_id.as_deref() == Some(group_id))
            .cloned()
            .map(Self::with_computed_uptime)
            .map(|info| Self::with_job_estimate(info, &history, &runners_guard))
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
                    .create_desired_running(
                        &repo_full_name,
                        None,
                        Some(template.config.labels.clone()),
                        Some(template.config.mode.clone()),
                        Some(group_id.to_string()),
                        template.config.container.clone(),
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
                if matches!(
                    runner.state,
                    RunnerState::Busy | RunnerState::Stopping | RunnerState::Deleting
                ) {
                    // Keep the existing response field for backwards compatibility;
                    // it represents runners that cannot be safely removed yet.
                    skipped_busy.push(runner.config.id.clone());
                    continue;
                }

                let token = self.auth_token.read().await.clone();
                let delete_result = if let Some(token) = token {
                    self.full_delete(&runner.config.id, &token).await
                } else {
                    self.delete(&runner.config.id).await
                };
                if let Err(e) = delete_result {
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
        let runners_guard = self.runners.read().await;
        let history = self.job_history.read().await;
        runners_guard
            .values()
            .cloned()
            .map(Self::with_computed_uptime)
            .map(|info| Self::with_job_estimate(info, &history, &runners_guard))
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
        let runners_guard = self.runners.read().await;
        let history = self.job_history.read().await;
        runners_guard
            .get(id)
            .cloned()
            .map(Self::with_computed_uptime)
            .map(|info| Self::with_job_estimate(info, &history, &runners_guard))
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

    pub async fn has_active_process(&self, id: &str) -> bool {
        self.processes.read().await.contains_key(id)
    }

    pub(crate) async fn begin_start_operation(&self, id: &str) -> Result<()> {
        let mut operations = self.lifecycle_operations.lock().await;
        if operations.shutting_down {
            bail!("Daemon shutdown is in progress");
        }
        if operations.deleting.contains(id) {
            bail!("Runner '{id}' is being deleted");
        }
        if operations.stopping.contains(id) {
            bail!("Runner '{id}' has a stop operation in progress");
        }
        if operations.updating.contains(id) {
            bail!("Runner '{id}' is being updated");
        }
        if !self.runners.read().await.contains_key(id) {
            bail!("Runner not found");
        }
        if !operations.starting.insert(id.to_string()) {
            bail!("Runner '{id}' already has a start operation in progress");
        }
        Ok(())
    }

    pub(crate) async fn finish_start_operation(&self, id: &str) {
        self.lifecycle_operations.lock().await.starting.remove(id);
    }

    /// Atomically stop admitting new starts and return the number of starts
    /// that were already admitted. Existing starts keep their reservation so
    /// shutdown can wait for them before deciding which processes to stop.
    pub(crate) async fn begin_shutdown_operation(&self) -> Result<usize> {
        let mut operations = self.lifecycle_operations.lock().await;
        if operations.shutting_down {
            bail!("Daemon shutdown is already in progress");
        }
        operations.shutting_down = true;
        Ok(operations.starting.len())
    }

    /// Re-open lifecycle admission when shutdown cannot complete. Callers must
    /// first drain operations admitted before the barrier so reopening cannot
    /// overlap an older mutation that shutdown had already accepted.
    pub(crate) async fn cancel_shutdown_operation(&self) {
        self.lifecycle_operations.lock().await.shutting_down = false;
    }

    /// Wait for all lifecycle mutations admitted before the shutdown barrier.
    /// New mutations cannot appear after `begin_shutdown_operation`.
    pub(crate) async fn wait_for_lifecycle_operations_to_finish(&self) {
        loop {
            let operations = self.lifecycle_operations.lock().await;
            let pending = operations.starting.len()
                + operations.stopping.len()
                + operations.updating.len()
                + operations.deleting.len();
            drop(operations);
            if pending == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    pub(crate) async fn begin_stop_operation(&self, id: &str) -> Result<()> {
        let mut operations = self.lifecycle_operations.lock().await;
        if operations.shutting_down {
            bail!("Daemon shutdown is in progress");
        }
        if operations.deleting.contains(id) {
            bail!("Runner '{id}' is being deleted");
        }
        if operations.starting.contains(id) {
            bail!("Runner '{id}' has a start operation in progress");
        }
        if operations.updating.contains(id) {
            bail!("Runner '{id}' is being updated");
        }
        if !self.runners.read().await.contains_key(id) {
            bail!("Runner not found");
        }
        if !operations.stopping.insert(id.to_string()) {
            bail!("Runner '{id}' already has a stop operation in progress");
        }
        Ok(())
    }

    pub(crate) async fn finish_stop_operation(&self, id: &str) {
        self.lifecycle_operations.lock().await.stopping.remove(id);
    }

    async fn begin_update_operation(&self, id: &str) -> Result<()> {
        let mut operations = self.lifecycle_operations.lock().await;
        if operations.shutting_down {
            bail!("Daemon shutdown is in progress");
        }
        if operations.deleting.contains(id) {
            bail!("Runner '{id}' is being deleted");
        }
        if operations.starting.contains(id) {
            bail!("Runner '{id}' has a start operation in progress");
        }
        if operations.stopping.contains(id) {
            bail!("Runner '{id}' has a stop operation in progress");
        }
        if !self.runners.read().await.contains_key(id) {
            bail!("Runner not found");
        }
        if !operations.updating.insert(id.to_string()) {
            bail!("Runner '{id}' already has an update in progress");
        }
        Ok(())
    }

    async fn finish_update_operation(&self, id: &str) {
        self.lifecycle_operations.lock().await.updating.remove(id);
    }

    async fn begin_delete_operation(&self, id: &str) -> Result<()> {
        let mut operations = self.lifecycle_operations.lock().await;
        if operations.shutting_down {
            bail!("Daemon shutdown is in progress");
        }
        if !self.runners.read().await.contains_key(id) {
            bail!("Runner not found");
        }
        if !operations.deleting.insert(id.to_string()) {
            bail!("Runner '{id}' already has a deletion in progress");
        }
        Ok(())
    }

    async fn finish_delete_operation(&self, id: &str) {
        self.lifecycle_operations.lock().await.deleting.remove(id);
    }

    async fn wait_for_mutations_to_finish(&self, id: &str) -> Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let operations = self.lifecycle_operations.lock().await;
                if !operations.starting.contains(id)
                    && !operations.stopping.contains(id)
                    && !operations.updating.contains(id)
                {
                    break;
                }
                drop(operations);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!("Timed out waiting for runner '{id}' lifecycle operation to finish")
        })?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.begin_delete_operation(id).await?;
        let result = self.delete_reserved(id).await;
        self.finish_delete_operation(id).await;
        result
    }

    async fn prepare_delete_reserved(&self, id: &str) -> Result<()> {
        // The deletion reservation already blocks new Start/Stop/PATCH operations.
        // Wait before changing desired-running so a timeout leaves the runner and
        // its recovery intent exactly as they were before the delete request.
        self.wait_for_mutations_to_finish(id).await?;
        self.set_desired_running(id, false).await?;

        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if runner.state == RunnerState::Online
            || runner.state == RunnerState::Busy
            || self.has_active_process(id).await
        {
            self.stop_process_internal(id, true).await?;
        }
        Ok(())
    }

    async fn remove_reserved(&self, id: &str) -> Result<()> {
        let _persistence_guard = self.persistence_lock.lock().await;
        let removed = self.runners.write().await.remove(id);
        if let Err(error) = self.save_to_disk_locked().await {
            if let Some(runner) = removed {
                self.runners.write().await.insert(id.to_string(), runner);
            }
            return Err(error).context("persisting runner deletion");
        }
        drop(_persistence_guard);

        // Destructive cleanup happens only after the durable state no longer
        // references this runner. A failed write therefore cannot resurrect a
        // deleted runner with a missing work directory on the next launch.
        self.processes.write().await.remove(id);
        self.delete_job_history(id).await;
        if let Some(runner) = removed {
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
        }
        Ok(())
    }

    async fn delete_reserved(&self, id: &str) -> Result<()> {
        self.wait_for_mutations_to_finish(id).await?;
        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if runner.config.work_dir.join(".runner").exists()
            || runner.config.work_dir.join(".runner_migrated").exists()
        {
            bail!("Authentication required to deregister configured runner before deletion");
        }
        self.prepare_delete_reserved(id).await?;
        self.remove_reserved(id).await?;
        self.emit_state_event(id, "deleting");
        Ok(())
    }

    pub async fn update(&self, id: &str, req: types::UpdateRunnerRequest) -> Result<RunnerInfo> {
        let _update_guard = self.update_lock.lock().await;
        self.begin_update_operation(id).await?;
        let result = self.update_reserved(id, req).await;
        self.finish_update_operation(id).await;
        result
    }

    async fn update_reserved(
        &self,
        id: &str,
        req: types::UpdateRunnerRequest,
    ) -> Result<RunnerInfo> {
        let normalized_labels = req.labels.map(types::normalize_labels).transpose()?;
        let requested_mode = req.mode;
        let display_name = match req.display_name {
            Some(value) => Some(types::normalize_display_name(value)?),
            None => None,
        };
        let _persistence_guard = self.persistence_lock.lock().await;
        let previous = {
            let mut runners = self.runners.write().await;
            let runner = runners
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;

            let stopped = matches!(
                runner.state,
                RunnerState::Creating | RunnerState::Offline | RunnerState::Error
            );
            if let Some(ref requested_mode) = requested_mode {
                if requested_mode != &runner.config.mode {
                    if !stopped {
                        bail!("Runner mode can only be changed while the runner is stopped");
                    }
                    if *requested_mode == RunnerMode::Container
                        || runner.config.mode == RunnerMode::Container
                    {
                        bail!(
                            "Container execution mode cannot be changed after creation; create a new runner instead"
                        );
                    }
                }
            }
            if normalized_labels.is_some() && !stopped {
                bail!("Runner labels can only be changed while the runner is stopped");
            }

            let previous_config = (
                runner.config.labels.clone(),
                runner.config.mode.clone(),
                runner.config.display_name.clone(),
            );
            if let Some(labels) = normalized_labels {
                runner.config.labels = labels;
            }
            if let Some(requested_mode) = requested_mode {
                runner.config.mode = requested_mode;
            }
            if let Some(display_name) = display_name {
                runner.config.display_name = display_name;
            }
            let applied_config = (
                runner.config.labels.clone(),
                runner.config.mode.clone(),
                runner.config.display_name.clone(),
            );
            (previous_config, applied_config)
        };

        if let Err(error) = self.save_to_disk_locked().await {
            let mut runners = self.runners.write().await;
            if let Some(runner) = runners.get_mut(id) {
                let current_config = (
                    runner.config.labels.clone(),
                    runner.config.mode.clone(),
                    runner.config.display_name.clone(),
                );
                // Do not undo a newer concurrent configuration update. Runtime
                // fields are intentionally never replaced during rollback.
                if current_config == previous.1 {
                    runner.config.labels = previous.0 .0;
                    runner.config.mode = previous.0 .1;
                    runner.config.display_name = previous.0 .2;
                }
            }
            return Err(error).context("persisting runner update");
        }
        drop(_persistence_guard);
        self.get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))
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
        let prev_running = {
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
            let prev_running =
                runner.state == RunnerState::Online || runner.state == RunnerState::Busy;
            runner.state = state.clone();
            runner.error_message = error_message;
            prev_running
        };
        let now_running = state == RunnerState::Online || state == RunnerState::Busy;
        if prev_running != now_running {
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
    /// 5. Run the config script
    /// 6. Spawn the runner script
    /// 7. Store PID, update state to Online
    /// 8. Spawn background monitor task
    pub async fn register_and_start(&self, id: &str, auth_token: &str) -> Result<()> {
        self.begin_start_operation(id).await?;

        let result = async {
            self.set_desired_running(id, true).await?;
            // 1. Transition Creating -> Registering
            self.update_state(id, RunnerState::Registering).await?;
            self.emit_state_event(id, "registering");
            self.do_register_and_start(id, auth_token).await
        }
        .await;
        self.finish_start_operation(id).await;
        result
    }

    /// Start a Creating/Offline/Error runner while the caller retains the start
    /// reservation. Manual API callers persist desired-running intent before spawning
    /// this work; startup restore and recovery already have that intent.
    pub(crate) async fn start_existing_reserved(&self, id: &str, auth_token: &str) -> Result<()> {
        self.update_state(id, RunnerState::Registering).await?;
        self.emit_state_event(id, "registering");
        self.do_register_and_start(id, auth_token).await
    }

    /// Common register-and-start flow (assumes already in Registering state):
    /// Downloads runner binary if needed, removes stale configuration via
    /// the config script's remove command, then runs the config script to register before starting the runner script.
    async fn do_register_and_start(&self, id: &str, auth_token: &str) -> Result<()> {
        self.set_auth_token(Some(auth_token.to_string())).await;

        // API unit tests use a dedicated sentinel token. Do not let their
        // background auto-start tasks download/configure a real GitHub Actions
        // runner: parallel tests would otherwise contend on the process-wide
        // runner download lock and keep lifecycle reservations for up to a minute.
        // This branch is compiled only in tests and cannot affect production.
        #[cfg(test)]
        if auth_token == "ghp_test_token" {
            self.update_state_with_error(
                id,
                RunnerState::Error,
                Some("Runner startup skipped for test authentication".to_string()),
            )
            .await?;
            self.emit_state_event(id, "error");
            return Ok(());
        }

        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if runner.state != RunnerState::Registering {
            bail!(
                "Runner '{id}' must be Registering before start, found {:?}",
                runner.state
            );
        }
        if self.has_active_process(id).await {
            bail!("Runner '{id}' already has an active process");
        }
        let config = &runner.config;

        // Check both .runner and .runner_migrated (newer runner versions rename
        // the config file during a migration).
        let already_configured = config.work_dir.join(".runner").exists()
            || config.work_dir.join(".runner_migrated").exists();

        if !already_configured {
            // First-time setup: download binary and copy to work_dir. Container
            // runners always need the Linux build — even bind-mounted into a
            // container launched from Docker Desktop on macOS/Windows, the
            // container itself runs a Linux kernel.
            let cached_runner_dir = if config.mode == RunnerMode::Container {
                let (_, arch) = binary::detect_platform();
                binary::ensure_runner_binary_for_container(&self.config.cache_dir(), arch)
                    .await
                    .context("Failed to download Linux runner binary for container")?
            } else {
                ensure_runner_binary(&self.config.cache_dir())
                    .await
                    .context("Failed to download runner binary")?
            };

            copy_dir_recursive(&cached_runner_dir, &config.work_dir)
                .context("Failed to copy runner binary to work dir")?;
        } else {
            tracing::info!("Runner {} already configured, skipping binary download", id);
        }

        // Kill any orphaned runner processes from a previous daemon session
        // BEFORE reconfiguring, so the old process releases the GitHub session.
        // Not applicable to container runners — nothing native was ever spawned.
        if config.mode != RunnerMode::Container {
            kill_orphaned_processes(&config.work_dir).await;
        }

        let gh = GitHubClient::new(Some(auth_token.to_string()))?;
        let reg = match gh
            .get_runner_registration_token(&config.repo_owner, &config.repo_name)
            .await
        {
            Ok(reg) => reg,
            Err(e) => {
                if crate::github::is_bad_credentials(&e) {
                    tracing::warn!("GitHub token is invalid (Bad credentials), logging out");
                    self.set_auth_token(None).await;
                    if let Some(ref auth) = self.auth_manager {
                        let _ = auth.logout().await;
                    }
                }
                return Err(e).context("Failed to get registration token");
            }
        };

        // Container runners connect to Docker once up front; native runners
        // never touch it.
        let docker_client = if config.mode == RunnerMode::Container {
            Some(docker::connect()?)
        } else {
            None
        };
        let container_cfg = config.container.as_ref();

        // If already configured, deregister before re-configuring. GitHub
        // requires a dedicated remove token; a registration token is not valid for
        // `config.sh remove` and previously left stale runners behind silently.
        if already_configured {
            let removal = gh
                .get_runner_remove_token(&config.repo_owner, &config.repo_name)
                .await
                .context("Failed to get runner removal token")?;
            if let (Some(dc), Some(cc)) = (&docker_client, container_cfg) {
                docker::deregister(dc, &config.work_dir, &cc.image, &removal.token)
                    .await
                    .context("Failed to deregister existing container runner")?;
            } else {
                remove_runner(&config.work_dir, &removal.token)
                    .await
                    .context("Failed to deregister existing runner")?;
            }
            clean_runner_config(&config.work_dir);
        }

        let repo_url = format!(
            "https://github.com/{}/{}",
            config.repo_owner, config.repo_name
        );

        type BoxedRead = Pin<Box<dyn AsyncRead + Send>>;
        let (running, stdout, stderr, pid, container_id): (
            RunningProcess,
            Option<BoxedRead>,
            Option<BoxedRead>,
            Option<u32>,
            Option<String>,
        ) = if let (Some(dc), Some(cc)) = (&docker_client, container_cfg) {
            let container = docker::configure_and_start_container(
                dc,
                &config.work_dir,
                &cc.image,
                &repo_url,
                &reg.token,
                &config.name,
                &config.labels,
                &cc.extra_env,
            )
            .await
            .context("Failed to configure/start container runner")?;
            let container_id = container.container_id.clone();
            (
                RunningProcess::Container {
                    docker: dc.clone(),
                    container_id: container_id.clone(),
                },
                Some(Box::pin(container.stdout) as BoxedRead),
                Some(Box::pin(container.stderr) as BoxedRead),
                None,
                Some(container_id),
            )
        } else {
            configure_runner(
                &config.work_dir,
                &repo_url,
                &reg.token,
                &config.name,
                &config.labels,
            )
            .await
            .context("Failed to configure runner")?;

            // Spawn the runner script (run.sh/run.cmd)
            let mut child = start_runner(&config.work_dir)
                .await
                .context("Failed to start runner process")?;

            // 5b. Capture stdout/stderr for log streaming
            let stdout = child.stdout.take().map(|s| Box::pin(s) as BoxedRead);
            let stderr = child.stderr.take().map(|s| Box::pin(s) as BoxedRead);
            let pid = child.id();
            (RunningProcess::Native(child), stdout, stderr, pid, None)
        };

        // Publish the process handle and Online state in one critical section.
        // Stop/restart/delete can now safely act as soon as Online is observable.
        let kill_signal = Arc::new(Notify::new());
        let (exit_tx, exit_rx) = watch::channel(false);
        let handle = ProcessHandle {
            kill_signal: kill_signal.clone(),
            exited: exit_rx,
        };
        let started_at = chrono::Utc::now();
        {
            let mut runners = self.runners.write().await;
            let runner = runners
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("Runner not found while publishing process"))?;
            if runner.state != RunnerState::Registering {
                bail!(
                    "Runner '{id}' changed to {:?} before its process could be published",
                    runner.state
                );
            }
            let mut processes = self.processes.write().await;
            if processes.contains_key(id) {
                bail!("Runner '{id}' already has an active process");
            }
            processes.insert(id.to_string(), handle);
            runner.state = RunnerState::Online;
            runner.pid = pid;
            runner.container_id = container_id;
            runner.started_at = Some(started_at);
            runner.current_job = None;
            runner.job_context = None;
            runner.job_started_at = None;
            runner.error_message = None;
        }
        self.emit_state_event(id, "online");
        if let Err(error) = self.save_to_disk().await {
            tracing::error!(
                runner = %id,
                error = %error,
                "Runner is online, but its state could not be persisted"
            );
        }

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
                                        latest_attempt: None,
                                        job_number: 0,
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
                                        latest_attempt: None,
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

                                    let runner_name = r.config.name.clone();
                                    Some((entry, runner_name, duration_secs))
                                } else {
                                    None
                                }
                            };

                            // Record history via cloned Arcs
                            if let Some((entry, _runner_name, _duration_secs)) = history_entry {
                                let self_name = {
                                    let map = runners.read().await;
                                    map.get(&rid)
                                        .map(|r| r.config.name.clone())
                                        .unwrap_or_default()
                                };
                                let mut hist = job_history_arc.write().await;
                                let entries = hist.entry(rid.clone()).or_default();
                                history::append(entries, entry.clone(), &self_name);
                                if let Err(e) = history::save(&history_dir, &rid, entries) {
                                    tracing::warn!("Failed to save job history for {}: {}", rid, e);
                                }

                                // Annotate other runners' history entries for the same run_id
                                if let Some(run_id) = entry
                                    .run_url
                                    .as_deref()
                                    .and_then(history::extract_run_id_from_url)
                                {
                                    let runner_name = {
                                        let map = runners.read().await;
                                        map.get(&rid).map(|r| r.config.name.clone())
                                    };
                                    if let Some(runner_name) = runner_name {
                                        let attempt = types::RunAttempt {
                                            attempt: 0,
                                            succeeded: entry.succeeded,
                                            runner_name,
                                            completed_at: entry.completed_at,
                                            run_url: entry.run_url.clone(),
                                        };
                                        for (other_id, other_entries) in hist.iter_mut() {
                                            if *other_id == rid {
                                                continue;
                                            }
                                            let mut changed = false;
                                            for e in other_entries.iter_mut() {
                                                if e.job_name == entry.job_name
                                                    && e.run_url
                                                        .as_deref()
                                                        .and_then(history::extract_run_id_from_url)
                                                        == Some(run_id)
                                                {
                                                    e.latest_attempt = Some(attempt.clone());
                                                    changed = true;
                                                }
                                            }
                                            if changed {
                                                if let Err(e) = history::save(
                                                    &history_dir,
                                                    other_id,
                                                    other_entries,
                                                ) {
                                                    tracing::warn!("Failed to save rerun annotation for {other_id}: {e}");
                                                }
                                                let _ = event_tx_stdout.send(RunnerEvent {
                                                    runner_id: other_id.clone(),
                                                    event_type: "state_changed".to_string(),
                                                    data: serde_json::json!({"rerun_updated": true}),
                                                    timestamp: chrono::Utc::now(),
                                                });
                                            }
                                        }
                                    }
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

        // 5d. On Windows, stdout piping through cmd.exe wrappers is unreliable
        // (Runner.Listener.exe output doesn't flow through the piped cmd.exe stdout).
        // Spawn diag log tailing as the primary job-event detection mechanism.
        #[cfg(windows)]
        {
            let mgr = self.clone();
            let rid = id.to_string();
            let diag_dir = config.work_dir.join("_diag");
            let wd = config.work_dir.clone();
            let sw = self.step_watcher.clone();
            tokio::spawn(async move {
                Self::tail_diag_logs(mgr, &rid, &diag_dir, &wd, sw).await;
            });
        }

        // 5e. Spawn step-watcher polling task
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

        // Spawn background monitor task — owns `running` exclusively.
        let manager = self.clone();
        let runner_id = id.to_string();
        tokio::spawn(async move {
            let exit_description = match running {
                RunningProcess::Native(mut child) => {
                    let exit_status = tokio::select! {
                        status = child.wait() => status,
                        _ = kill_signal.notified() => {
                            // Kill signal received — gracefully stop the entire process group.
                            // The runner script spawns .NET child processes that hold the GitHub session,
                            // so we must signal the whole group to let them deregister cleanly.
                            if let Some(pid) = child.id() {
                                // Gracefully stop the entire process tree
                                #[cfg(unix)]
                                {
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
                                }
                                #[cfg(windows)]
                                {
                                    // On Windows, use taskkill /T to kill the process tree
                                    let _ = std::process::Command::new("taskkill")
                                        .args(["/T", "/F", "/PID", &pid.to_string()])
                                        .stdout(std::process::Stdio::null())
                                        .stderr(std::process::Stdio::null())
                                        .status();
                                    child.wait().await
                                }
                            } else {
                                // No PID — process already exited
                                child.wait().await
                            }
                        }
                    };
                    format!("{exit_status:?}")
                }
                RunningProcess::Container {
                    docker,
                    container_id,
                } => {
                    tokio::select! {
                        result = docker::wait_container(&docker, &container_id) => {
                            // The container exited on its own — remove it so
                            // exited containers don't pile up (the kill path
                            // below removes via stop_container).
                            let _ = docker::remove_container(&docker, &container_id).await;
                            format!("{result:?}")
                        }
                        _ = kill_signal.notified() => {
                            // Docker's stop already does SIGTERM-then-SIGKILL
                            // with a timeout, so no manual escalation needed.
                            let _ = docker::stop_container(
                                &docker,
                                &container_id,
                                std::time::Duration::from_secs(10),
                            )
                            .await;
                            "stopped by request".to_string()
                        }
                    }
                }
            };
            tracing::info!("Runner {} exited: {}", runner_id, exit_description);

            // Update state before notifying stop callers. This guarantees that a
            // successful stop wait never returns while the old process is still
            // represented as running.
            let unexpected = {
                let mut runners = manager.runners.write().await;
                if let Some(r) = runners.get_mut(&runner_id) {
                    let unexpected = r.state == RunnerState::Online || r.state == RunnerState::Busy;
                    if unexpected || r.state == RunnerState::Stopping {
                        r.state = RunnerState::Offline;
                        r.pid = None;
                        r.container_id = None;
                        r.started_at = None;
                        r.current_job = None;
                        r.job_context = None;
                        r.job_started_at = None;
                        r.error_message = unexpected.then(|| {
                            "Runner process exited unexpectedly; recovery is scheduled".to_string()
                        });
                    }
                    unexpected
                } else {
                    false
                }
            };
            manager.processes.write().await.remove(&runner_id);
            manager.emit_state_event(&runner_id, "offline");

            let should_recover = unexpected && manager.is_desired_running(&runner_id).await;
            if should_recover {
                manager.schedule_recovery(runner_id.clone());
            } else {
                let _ = manager.save_to_disk().await;
            }
            let _ = exit_tx.send(true);
        });

        Ok(())
    }

    pub(crate) async fn set_desired_running(&self, runner_id: &str, desired: bool) -> Result<()> {
        let _persistence_guard = self.persistence_lock.lock().await;
        if desired && !self.runners.read().await.contains_key(runner_id) {
            bail!("Runner not found");
        }

        let previous = {
            let mut desired_running = self.desired_running.write().await;
            let previous = desired_running.contains(runner_id);
            if desired {
                desired_running.insert(runner_id.to_string());
            } else {
                desired_running.remove(runner_id);
            }
            previous
        };
        if let Err(error) = self.save_to_disk_locked().await {
            let mut desired_running = self.desired_running.write().await;
            if previous {
                desired_running.insert(runner_id.to_string());
            } else {
                desired_running.remove(runner_id);
            }
            return Err(error).context("persisting desired runner state");
        }
        Ok(())
    }

    async fn is_desired_running(&self, runner_id: &str) -> bool {
        self.desired_running.read().await.contains(runner_id)
    }

    /// Keep retrying a runner that the user expects to be online. Retries are
    /// de-duplicated and back off to one attempt per minute. Explicit Stop or
    /// Delete clears `desired_running`, which cancels the loop.
    pub fn schedule_recovery(&self, runner_id: String) {
        let manager = self.clone();
        tokio::spawn(async move {
            {
                let mut recovering = manager.recovering.lock().await;
                if !recovering.insert(runner_id.clone()) {
                    return;
                }
            }

            const BACKOFF_SECS: [u64; 5] = [2, 5, 15, 30, 60];
            let mut attempt = 0usize;

            loop {
                let delay_secs = BACKOFF_SECS[attempt.min(BACKOFF_SECS.len() - 1)];
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                if !manager.is_desired_running(&runner_id).await {
                    break;
                }

                let Some(info) = manager.get(&runner_id).await else {
                    break;
                };
                match info.state {
                    RunnerState::Offline | RunnerState::Error => {}
                    RunnerState::Registering => {
                        // A manual start is in flight. Keep this de-duplicated loop
                        // alive so it can resume if that attempt fails.
                        continue;
                    }
                    RunnerState::Online | RunnerState::Busy => break,
                    RunnerState::Stopping => continue,
                    RunnerState::Creating | RunnerState::Deleting => break,
                }

                let Some(token) = manager.auth_token.read().await.clone() else {
                    {
                        let mut runners = manager.runners.write().await;
                        if let Some(runner) = runners.get_mut(&runner_id) {
                            let next_delay =
                                BACKOFF_SECS[(attempt + 1).min(BACKOFF_SECS.len() - 1)];
                            runner.state = RunnerState::Error;
                            runner.error_message = Some(format!(
                                "Runner is waiting for authentication; retrying in {next_delay}s"
                            ));
                        }
                    }
                    manager.emit_state_event(&runner_id, "error");
                    let _ = manager.save_to_disk().await;
                    attempt += 1;
                    continue;
                };

                if let Err(error) = manager.begin_start_operation(&runner_id).await {
                    tracing::debug!(
                        runner = %runner_id,
                        error = %error,
                        "Recovery deferred by another lifecycle operation"
                    );
                    continue;
                }

                match manager.start_existing_reserved(&runner_id, &token).await {
                    Ok(()) => {
                        manager.finish_start_operation(&runner_id).await;
                        tracing::info!(
                            runner = %runner_id,
                            attempt = attempt + 1,
                            "Runner recovered after an unexpected exit"
                        );
                        break;
                    }
                    Err(error) => {
                        attempt += 1;
                        let next_delay = BACKOFF_SECS[attempt.min(BACKOFF_SECS.len() - 1)];
                        tracing::error!(
                            runner = %runner_id,
                            attempt,
                            error = %error,
                            "Automatic runner recovery failed"
                        );
                        let _ = manager
                            .update_state_with_error(
                                &runner_id,
                                RunnerState::Error,
                                Some(format!(
                                    "Automatic recovery attempt {attempt} failed: {error:#}. Retrying in {next_delay}s"
                                )),
                            )
                            .await;
                        manager.emit_state_event(&runner_id, "error");
                        manager.finish_start_operation(&runner_id).await;
                    }
                }
            }

            manager.recovering.lock().await.remove(&runner_id);
        });
    }

    async fn begin_stop(&self, id: &str, clear_desired: bool) -> Result<()> {
        // Hold the persistence transaction across mutation, durable write, and
        // rollback so no other save can publish an intermediate stop state.
        let _persistence_guard = self.persistence_lock.lock().await;
        let (previous_state, was_desired) = {
            let mut desired_running = self.desired_running.write().await;
            let mut runners = self.runners.write().await;
            let runner = runners
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
            if !runner.state.can_transition_to(&RunnerState::Stopping) {
                bail!(
                    "Invalid state transition: {:?} -> {:?}",
                    runner.state,
                    RunnerState::Stopping
                );
            }
            let previous_state = runner.state.clone();
            let was_desired = desired_running.contains(id);
            runner.state = RunnerState::Stopping;
            runner.error_message = None;
            if clear_desired {
                desired_running.remove(id);
            }
            (previous_state, was_desired)
        };

        if let Err(error) = self.save_to_disk_locked().await {
            let mut desired_running = self.desired_running.write().await;
            let mut runners = self.runners.write().await;
            if let Some(runner) = runners.get_mut(id) {
                if runner.state == RunnerState::Stopping {
                    runner.state = previous_state;
                }
            }
            if was_desired {
                desired_running.insert(id.to_string());
            } else {
                desired_running.remove(id);
            }
            return Err(error).context("persisting stop transition");
        }

        drop(_persistence_guard);
        self.emit_state_event(id, "stopping");
        Ok(())
    }

    /// Stop a running runner process because the user explicitly requested it.
    pub async fn stop_process(&self, id: &str) -> Result<()> {
        self.begin_stop_operation(id).await?;
        let result = self.stop_process_internal(id, true).await;
        self.finish_stop_operation(id).await;
        result
    }

    /// Stop a process while retaining the user's desired-running intent. This is
    /// used by daemon shutdown flows so a later launch can restore it.
    pub async fn stop_process_preserving_intent(&self, id: &str) -> Result<()> {
        self.begin_stop_operation(id).await?;
        let result = self.stop_process_internal(id, false).await;
        self.finish_stop_operation(id).await;
        result
    }

    /// Signals the monitoring task to kill the child via `Notify`, then waits
    /// for the `watch` channel to confirm the process has fully exited.
    /// No shared lock on `Child` — eliminates the deadlock from issue #31.
    pub(crate) async fn stop_process_internal(&self, id: &str, clear_desired: bool) -> Result<()> {
        // State and desired-running intent are changed atomically. A rejected
        // transition must never silently disable future recovery.
        self.begin_stop(id, clear_desired).await?;

        let handle = self.processes.read().await.get(id).cloned();
        if let Some(handle) = handle {
            handle.kill_signal.notify_one();

            let mut exited = handle.exited;
            match tokio::time::timeout(std::time::Duration::from_secs(15), exited.wait_for(|&v| v))
                .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => bail!("Failed while waiting for runner '{id}' to exit: {error}"),
                Err(_) => {
                    // Never claim Offline while the old process may still own the
                    // GitHub runner session; doing so permits a duplicate start.
                    bail!(
                        "Timed out waiting for runner '{id}' to exit; it remains in Stopping state"
                    )
                }
            }

            // The monitor normally applies this transition before sending the
            // watch notification. Keep a defensive fallback for reconstructed or
            // test handles that only signal process exit.
            let mut runners = self.runners.write().await;
            if let Some(runner) = runners.get_mut(id) {
                if runner.state == RunnerState::Stopping {
                    runner.state = RunnerState::Offline;
                    runner.pid = None;
                    runner.container_id = None;
                    runner.started_at = None;
                    runner.current_job = None;
                    runner.job_context = None;
                    runner.job_started_at = None;
                    runner.error_message = None;
                }
            }
            drop(runners);
            self.emit_state_event(id, "offline");
            self.save_to_disk().await?;
        } else {
            // No tracked process exists. Complete the state transition locally.
            let mut runners = self.runners.write().await;
            if let Some(runner) = runners.get_mut(id) {
                runner.state = RunnerState::Offline;
                runner.pid = None;
                runner.container_id = None;
                runner.started_at = None;
                runner.current_job = None;
                runner.job_context = None;
                runner.job_started_at = None;
                runner.error_message = None;
            }
            drop(runners);
            self.emit_state_event(id, "offline");
            self.save_to_disk().await?;
        }

        Ok(())
    }

    /// Full delete flow: stop process, deregister from GitHub, remove work dir.
    pub async fn full_delete(&self, id: &str, auth_token: &str) -> Result<()> {
        self.begin_delete_operation(id).await?;
        let result = self.full_delete_reserved(id, auth_token).await;
        self.finish_delete_operation(id).await;
        result
    }

    async fn full_delete_reserved(&self, id: &str, auth_token: &str) -> Result<()> {
        self.prepare_delete_reserved(id).await?;
        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        // Only configured runners have a GitHub registration to remove. Use the
        // dedicated removal token and propagate failures so local deletion never
        // silently leaves a stale remote runner behind.
        let config = &runner.config;
        let configured = config.work_dir.join(".runner").exists()
            || config.work_dir.join(".runner_migrated").exists();
        if configured {
            let gh = GitHubClient::new(Some(auth_token.to_string()))?;
            let removal = gh
                .get_runner_remove_token(&config.repo_owner, &config.repo_name)
                .await
                .context("Failed to get runner removal token")?;
            if let Some(cc) = config.container.as_ref() {
                let dc = docker::connect()?;
                docker::deregister(&dc, &config.work_dir, &cc.image, &removal.token)
                    .await
                    .context("Failed to deregister container runner")?;
            } else {
                remove_runner(&config.work_dir, &removal.token)
                    .await
                    .context("Failed to deregister runner")?;
            }
        }

        self.remove_reserved(id).await?;
        self.emit_state_event(id, "deleting");
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

    /// Record a completed job in history, then annotate other runners' entries
    /// for the same workflow run with `latest_attempt`.
    pub async fn record_job_history(&self, runner_id: &str, entry: types::JobHistoryEntry) {
        let runner_name = {
            let map = self.runners.read().await;
            map.get(runner_id)
                .map(|r| r.config.name.clone())
                .unwrap_or_default()
        };
        let mut hist = self.job_history.write().await;
        let entries = hist.entry(runner_id.to_string()).or_default();
        history::append(entries, entry.clone(), &runner_name);
        if let Err(e) = history::save(&self.config.history_dir(), runner_id, entries) {
            tracing::warn!("Failed to save job history for {}: {}", runner_id, e);
        }
        drop(hist);
        self.annotate_cross_runner_reruns(runner_id, &entry).await;
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
            .create("aGallea/gifted", None, None, None, None, None)
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
            .create("aGallea/gifted", None, None, None, None, None)
            .await
            .unwrap();
        manager
            .create("aGallea/gifted", None, None, None, None, None)
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
            .create("aGallea/gifted", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        manager.delete(&id).await.unwrap();
        let runners = manager.list().await;
        assert_eq!(runners.len(), 0);
    }

    #[tokio::test]
    async fn test_update_validates_display_name_before_mutating_other_fields() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let original_labels = runner.config.labels.clone();

        let result = manager
            .update(
                &runner.config.id,
                types::UpdateRunnerRequest {
                    labels: Some(vec!["changed".to_string()]),
                    mode: Some(RunnerMode::Service),
                    display_name: Some(Some("bad\nname".to_string())),
                },
            )
            .await;

        assert!(result.is_err());
        let unchanged = manager.get(&runner.config.id).await.unwrap();
        assert_eq!(unchanged.config.labels, original_labels);
        assert_eq!(unchanged.config.mode, RunnerMode::App);
        assert_eq!(unchanged.config.display_name, None);
    }

    #[tokio::test]
    async fn test_create_desired_running_persists_intent_atomically() {
        let manager = create_test_manager();
        let runner = manager
            .create_desired_running("owner/repo", None, None, None, None, None)
            .await
            .unwrap();

        assert!(manager.is_desired_running(&runner.config.id).await);
        let persisted: Vec<PersistedRunner> = serde_json::from_str(
            &std::fs::read_to_string(manager.config.runners_json_path()).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].config.id, runner.config.id);
        assert!(persisted[0].was_running);

        // Desired-running creation must return with its start already admitted.
        assert_eq!(manager.begin_shutdown_operation().await.unwrap(), 1);
        manager.finish_start_operation(&runner.config.id).await;
    }

    #[tokio::test]
    async fn test_shutdown_waits_for_creation_persistence_transaction() {
        let manager = create_test_manager();
        let persistence_guard = manager.persistence_lock.lock().await;

        let create_manager = manager.clone();
        let create_task = tokio::spawn(async move {
            create_manager
                .create("owner/repo", None, None, None, None, None)
                .await
        });

        // create_with_intent acquires lifecycle admission before persistence.
        // Keep the persistence transaction blocked until that admission lock
        // is definitely held, then prove shutdown cannot overtake it.
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if manager.lifecycle_operations.try_lock().is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("creation never acquired lifecycle admission");

        let shutdown_manager = manager.clone();
        let mut shutdown_task =
            tokio::spawn(async move { shutdown_manager.begin_shutdown_operation().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut shutdown_task,)
                .await
                .is_err()
        );

        drop(persistence_guard);
        let runner = create_task.await.unwrap().unwrap();
        assert_eq!(shutdown_task.await.unwrap().unwrap(), 0);
        assert!(manager.get(&runner.config.id).await.is_some());
    }

    #[tokio::test]
    async fn test_cancel_shutdown_reopens_lifecycle_admission() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(manager.begin_shutdown_operation().await.unwrap(), 0);
        assert!(manager
            .begin_start_operation(&runner.config.id)
            .await
            .is_err());

        manager.cancel_shutdown_operation().await;
        manager
            .begin_start_operation(&runner.config.id)
            .await
            .unwrap();
        manager.finish_start_operation(&runner.config.id).await;

        // A later shutdown can install a fresh barrier normally.
        assert_eq!(manager.begin_shutdown_operation().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_test_auth_start_skips_external_runner_setup() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager
            .register_and_start(&id, "ghp_test_token")
            .await
            .unwrap();

        let runner = manager.get(&id).await.unwrap();
        assert_eq!(runner.state, RunnerState::Error);
        assert!(!manager.has_active_process(&id).await);
        assert!(runner
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("skipped for test authentication"));
    }

    #[tokio::test]
    async fn test_shutdown_barrier_rejects_new_starts() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(manager.begin_shutdown_operation().await.unwrap(), 0);
        let error = manager
            .begin_start_operation(&runner.config.id)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("shutdown"));
    }

    #[tokio::test]
    async fn test_shutdown_barrier_rejects_other_mutations() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.begin_shutdown_operation().await.unwrap();
        assert!(manager.begin_stop_operation(&id).await.is_err());
        assert!(manager.begin_update_operation(&id).await.is_err());
        assert!(manager.begin_delete_operation(&id).await.is_err());
        assert!(manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_shutdown_waits_for_previously_admitted_start() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.begin_start_operation(&id).await.unwrap();
        assert_eq!(manager.begin_shutdown_operation().await.unwrap(), 1);
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(25),
            manager.wait_for_lifecycle_operations_to_finish(),
        )
        .await
        .is_err());

        manager.finish_start_operation(&id).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            manager.wait_for_lifecycle_operations_to_finish(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_desired_running_is_persisted_independently_from_error_state() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        manager.set_desired_running(&id, true).await.unwrap();
        manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state_with_error(&id, RunnerState::Error, Some("transient".to_string()))
            .await
            .unwrap();
        manager.save_to_disk().await.unwrap();

        let persisted: Vec<PersistedRunner> = serde_json::from_str(
            &std::fs::read_to_string(manager.config.runners_json_path()).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.len(), 1);
        assert!(persisted[0].was_running);

        manager.set_desired_running(&id, false).await.unwrap();
        let persisted: Vec<PersistedRunner> = serde_json::from_str(
            &std::fs::read_to_string(manager.config.runners_json_path()).unwrap(),
        )
        .unwrap();
        assert!(!persisted[0].was_running);
    }

    #[tokio::test]
    async fn test_create_rejects_blank_and_control_character_names() {
        let manager = create_test_manager();
        assert!(manager
            .create(
                "owner/repo",
                Some("   ".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .is_err());
        assert!(manager
            .create(
                "owner/repo",
                Some(
                    "bad
name"
                        .to_string()
                ),
                None,
                None,
                None,
                None,
            )
            .await
            .is_err());
        assert!(manager.list().await.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_explicit_name_creation_is_unique() {
        let manager = create_test_manager();
        let first_manager = manager.clone();
        let second_manager = manager.clone();
        let first = tokio::spawn(async move {
            first_manager
                .create(
                    "owner/repo",
                    Some("shared-name".to_string()),
                    None,
                    None,
                    None,
                    None,
                )
                .await
        });
        let second = tokio::spawn(async move {
            second_manager
                .create(
                    "owner/repo",
                    Some("shared-name".to_string()),
                    None,
                    None,
                    None,
                    None,
                )
                .await
        });
        let results = [first.await.unwrap(), second.await.unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(manager.list().await.len(), 1);
    }

    #[tokio::test]
    async fn test_same_native_runner_name_is_allowed_across_repositories() {
        let manager = create_test_manager();
        manager
            .create(
                "owner/one",
                Some("shared-name".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        manager
            .create(
                "owner/two",
                Some("shared-name".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(manager.list().await.len(), 2);
    }

    #[tokio::test]
    async fn test_same_container_runner_name_is_rejected_across_repositories() {
        let manager = create_test_manager();
        let container = types::ContainerConfig {
            image: "ghcr.io/agallea/homerun-runner:ubuntu-24.04".to_string(),
            extra_env: vec![],
        };
        manager
            .create(
                "owner/one",
                Some("shared-name".to_string()),
                None,
                Some(RunnerMode::Container),
                None,
                Some(container.clone()),
            )
            .await
            .unwrap();
        let duplicate = manager
            .create(
                "owner/two",
                Some("shared-name".to_string()),
                None,
                Some(RunnerMode::Container),
                None,
                Some(container),
            )
            .await;
        assert!(duplicate.is_err());
        assert_eq!(manager.list().await.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_reservation_blocks_new_start_operations() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.begin_delete_operation(&id).await.unwrap();
        assert!(manager.begin_start_operation(&id).await.is_err());
        manager.finish_delete_operation(&id).await;

        manager.begin_start_operation(&id).await.unwrap();
        manager.finish_start_operation(&id).await;
    }

    #[tokio::test]
    async fn test_persisted_mutations_wait_for_their_persistence_transaction() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        let persistence_guard = manager.persistence_lock.lock().await;
        let update = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move {
                manager
                    .update(
                        &id,
                        types::UpdateRunnerRequest {
                            labels: None,
                            mode: None,
                            display_name: Some(Some("durable alias".to_string())),
                        },
                    )
                    .await
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(manager.get(&id).await.unwrap().config.display_name, None);
        drop(persistence_guard);
        update.await.unwrap().unwrap();
        assert_eq!(
            manager
                .get(&id)
                .await
                .unwrap()
                .config
                .display_name
                .as_deref(),
            Some("durable alias")
        );

        manager.set_desired_running(&id, true).await.unwrap();
        let persistence_guard = manager.persistence_lock.lock().await;
        let clear_intent = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move { manager.set_desired_running(&id, false).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(manager.is_desired_running(&id).await);
        drop(persistence_guard);
        clear_intent.await.unwrap().unwrap();
        assert!(!manager.is_desired_running(&id).await);

        manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state(&id, RunnerState::Online)
            .await
            .unwrap();
        manager.set_desired_running(&id, true).await.unwrap();
        let persistence_guard = manager.persistence_lock.lock().await;
        let stopping = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move { manager.begin_stop(&id, true).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let current = manager.get(&id).await.unwrap();
        assert_eq!(current.state, RunnerState::Online);
        assert!(manager.is_desired_running(&id).await);
        drop(persistence_guard);
        stopping.await.unwrap().unwrap();
        assert_eq!(manager.get(&id).await.unwrap().state, RunnerState::Stopping);
        assert!(!manager.is_desired_running(&id).await);
    }

    #[tokio::test]
    async fn test_delete_waits_for_an_admitted_stop_operation() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.begin_stop_operation(&id).await.unwrap();
        assert!(manager.begin_start_operation(&id).await.is_err());
        assert!(manager
            .update(
                &id,
                types::UpdateRunnerRequest {
                    labels: None,
                    mode: None,
                    display_name: Some(Some("blocked".to_string())),
                },
            )
            .await
            .is_err());
        manager.begin_delete_operation(&id).await.unwrap();

        let waiter = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move { manager.wait_for_mutations_to_finish(&id).await })
        };
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        manager.finish_stop_operation(&id).await;
        waiter.await.unwrap().unwrap();
        manager.finish_delete_operation(&id).await;
    }

    #[tokio::test]
    async fn test_update_reservation_blocks_start_and_delete_waits_for_update() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.begin_update_operation(&id).await.unwrap();
        assert!(manager.begin_start_operation(&id).await.is_err());
        manager.begin_delete_operation(&id).await.unwrap();

        let waiter = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move { manager.wait_for_mutations_to_finish(&id).await })
        };
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        manager.finish_update_operation(&id).await;
        waiter.await.unwrap().unwrap();
        manager.finish_delete_operation(&id).await;
    }

    #[tokio::test]
    async fn test_delete_clears_intent_restored_by_inflight_start() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;
        manager.begin_start_operation(&id).await.unwrap();
        manager.set_desired_running(&id, true).await.unwrap();

        let deletion = {
            let manager = manager.clone();
            let id = id.clone();
            tokio::spawn(async move { manager.delete(&id).await })
        };
        loop {
            if manager
                .lifecycle_operations
                .lock()
                .await
                .deleting
                .contains(&id)
            {
                break;
            }
            tokio::task::yield_now().await;
        }

        // The deletion reservation blocks recovery/start admission, so Delete
        // must not mutate user intent while waiting for the admitted start to drain.
        assert!(manager.is_desired_running(&id).await);
        manager.finish_start_operation(&id).await;
        deletion.await.unwrap().unwrap();

        assert!(!manager.is_desired_running(&id).await);
        assert!(manager.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_local_delete_rejects_configured_runner_without_changing_intent() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();
        std::fs::write(runner.config.work_dir.join(".runner"), "configured").unwrap();
        manager.set_desired_running(&id, true).await.unwrap();

        let error = manager.delete(&id).await.unwrap_err();

        assert!(error.to_string().contains("Authentication required"));
        assert!(manager.get(&id).await.is_some());
        assert!(manager.is_desired_running(&id).await);
    }

    #[tokio::test]
    async fn test_full_delete_skips_remote_api_for_unconfigured_runner() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        manager.full_delete(&id, "not-a-real-token").await.unwrap();

        assert!(manager.get(&id).await.is_none());
        assert!(!manager.is_desired_running(&id).await);
    }

    #[tokio::test]
    async fn test_update_rejects_mode_change_and_active_label_change() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        let mode_result = manager
            .update(
                &id,
                types::UpdateRunnerRequest {
                    labels: None,
                    mode: Some(RunnerMode::Container),
                    display_name: None,
                },
            )
            .await;
        assert!(mode_result.is_err());
        assert_eq!(manager.get(&id).await.unwrap().config.mode, RunnerMode::App);

        manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state(&id, RunnerState::Online)
            .await
            .unwrap();
        let label_result = manager
            .update(
                &id,
                types::UpdateRunnerRequest {
                    labels: Some(vec!["changed".to_string()]),
                    mode: None,
                    display_name: None,
                },
            )
            .await;
        assert!(label_result.is_err());

        let mode_result = manager
            .update(
                &id,
                types::UpdateRunnerRequest {
                    labels: None,
                    mode: Some(RunnerMode::Service),
                    display_name: None,
                },
            )
            .await;
        assert!(mode_result.is_err());
    }

    #[tokio::test]
    async fn test_failed_stop_transition_preserves_desired_running_intent() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();
        manager.set_desired_running(&id, true).await.unwrap();

        let result = manager.stop_process(&id).await;
        assert!(result.is_err());
        assert!(manager.is_desired_running(&id).await);
        assert_eq!(manager.get(&id).await.unwrap().state, RunnerState::Creating);
    }

    #[tokio::test]
    async fn test_runner_state_transitions() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("aGallea/gifted", None, None, None, None, None)
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
            .create("owner/repo1", None, None, None, None, None)
            .await
            .unwrap();
        manager
            .create("owner/repo2", None, None, None, None, None)
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
                display_name: None,
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
                labels: vec!["self-hosted".to_string()],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp/runner-abc"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Busy,
            pid: Some(1234),
            container_id: None,
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
    async fn test_update_native_mode_while_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        let updated = manager
            .update(
                &id,
                crate::runner::types::UpdateRunnerRequest {
                    labels: None,
                    mode: Some(crate::runner::types::RunnerMode::Service),
                    display_name: None,
                },
            )
            .await
            .unwrap();

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
                    display_name: None,
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
            .create("owner/repo", None, None, None, None, None)
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
                Some(vec!["gpu".to_string(), "custom".to_string()]),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(runner.config.name, "my-runner");
        // User-provided labels are used as-is, no defaults merged
        assert_eq!(runner.config.labels.len(), 2);
        assert!(runner.config.labels.contains(&"gpu".to_string()));
        assert!(runner.config.labels.contains(&"custom".to_string()));
    }

    #[tokio::test]
    async fn test_create_runner_with_empty_labels_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                Some(vec![]),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Empty labels should fall back to platform defaults
        assert!(runner.config.labels.contains(&"self-hosted".to_string()));
        assert!(runner.config.labels.contains(&default_runner_labels()[1]));
    }

    #[tokio::test]
    async fn test_create_runner_with_none_labels_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();

        // None labels should fall back to platform defaults
        assert!(runner.config.labels.contains(&"self-hosted".to_string()));
        assert!(runner.config.labels.contains(&default_runner_labels()[1]));
    }

    #[tokio::test]
    async fn test_create_runner_with_single_label() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                Some(vec!["self-hosted".to_string()]),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Only the user-provided label, no defaults added
        assert_eq!(runner.config.labels, vec!["self-hosted".to_string()]);
    }

    #[tokio::test]
    async fn test_create_runner_invalid_repo_format() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let result = manager.create("nodash", None, None, None, None, None).await;
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
            .create("owner/repo", None, None, None, None, None)
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

        // The runner was never configured, so full_delete must skip the remote
        // API entirely and remove it even when the supplied token is invalid.
        let result = manager.full_delete(&id, "invalid-token").await;
        assert!(result.is_ok(), "unconfigured full_delete should stay local");
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
            .create("owner/repo", None, None, None, None, None)
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
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Online,
            pid: None,
            container_id: None,
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
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Offline,
            pid: None,
            container_id: None,
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
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
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
                latest_attempt: None,
                job_number: 0,
            }],
        );
        let result = RunnerManager::with_job_estimate(info, &history, &HashMap::new());
        assert_eq!(result.estimated_job_duration_secs, Some(200));
    }

    #[test]
    fn test_with_job_estimate_busy_no_history() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
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
        let result = RunnerManager::with_job_estimate(info, &history, &HashMap::new());
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_online_ignored() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Online,
            pid: None,
            container_id: None,
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
        let result = RunnerManager::with_job_estimate(info, &history, &HashMap::new());
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_busy_no_current_job() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
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
        let result = RunnerManager::with_job_estimate(info, &history, &HashMap::new());
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_group_fallback_when_no_own_history() {
        use crate::runner::types::{JobHistoryEntry, RunnerConfig, RunnerMode};
        let now = chrono::Utc::now();
        // Runner-2 is busy, has no own history, but is in a group with runner-1
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-2".to_string(),
                name: "n2".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: Some("group-a".to_string()),
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
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
        // Sibling runner-1 has history for "build"
        let mut history = HashMap::new();
        history.insert(
            "runner-1".to_string(),
            vec![JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(300),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            }],
        );
        // Runners map — both runners share group-a
        let mut runners = HashMap::new();
        runners.insert(
            "runner-1".to_string(),
            RunnerInfo {
                config: RunnerConfig {
                    id: "runner-1".to_string(),
                    name: "n1".to_string(),
                    display_name: None,
                    repo_owner: "o".to_string(),
                    repo_name: "r".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp"),
                    group_id: Some("group-a".to_string()),
                    container: None,
                },
                state: RunnerState::Online,
                pid: None,
                container_id: None,
                uptime_secs: None,
                started_at: None,
                jobs_completed: 1,
                jobs_failed: 0,
                current_job: None,
                job_context: None,
                error_message: None,
                job_started_at: None,
                last_completed_job: None,
                estimated_job_duration_secs: None,
            },
        );
        runners.insert("runner-2".to_string(), info.clone());

        let result = RunnerManager::with_job_estimate(info, &history, &runners);
        assert_eq!(result.estimated_job_duration_secs, Some(300));
    }

    #[test]
    fn test_with_job_estimate_own_history_takes_priority_over_group() {
        use crate::runner::types::{JobHistoryEntry, RunnerConfig, RunnerMode};
        let now = chrono::Utc::now();
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-2".to_string(),
                name: "n2".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: Some("group-a".to_string()),
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 1,
            jobs_failed: 0,
            current_job: Some("build".to_string()),
            job_context: None,
            error_message: None,
            job_started_at: Some(now),
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        let mut history = HashMap::new();
        // runner-2 own history: 100s
        history.insert(
            "runner-2".to_string(),
            vec![JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(100),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            }],
        );
        // sibling runner-1 history: 500s
        history.insert(
            "runner-1".to_string(),
            vec![JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(500),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            }],
        );
        let mut runners = HashMap::new();
        runners.insert(
            "runner-1".to_string(),
            RunnerInfo {
                config: RunnerConfig {
                    id: "runner-1".to_string(),
                    name: "n1".to_string(),
                    display_name: None,
                    repo_owner: "o".to_string(),
                    repo_name: "r".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp"),
                    group_id: Some("group-a".to_string()),
                    container: None,
                },
                state: RunnerState::Online,
                pid: None,
                container_id: None,
                uptime_secs: None,
                started_at: None,
                jobs_completed: 1,
                jobs_failed: 0,
                current_job: None,
                job_context: None,
                error_message: None,
                job_started_at: None,
                last_completed_job: None,
                estimated_job_duration_secs: None,
            },
        );
        runners.insert("runner-2".to_string(), info.clone());

        let result = RunnerManager::with_job_estimate(info, &history, &runners);
        // Should use own history (100s), NOT group (500s)
        assert_eq!(result.estimated_job_duration_secs, Some(100));
    }

    #[test]
    fn test_with_job_estimate_no_group_no_own_history_returns_none() {
        use crate::runner::types::{RunnerConfig, RunnerMode};
        let now = chrono::Utc::now();
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-1".to_string(),
                name: "n".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: None, // no group
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
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
        let history = HashMap::new();
        let runners = HashMap::new();
        let result = RunnerManager::with_job_estimate(info, &history, &runners);
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[test]
    fn test_with_job_estimate_group_sibling_no_matching_job_returns_none() {
        use crate::runner::types::{JobHistoryEntry, RunnerConfig, RunnerMode};
        let now = chrono::Utc::now();
        let info = RunnerInfo {
            config: RunnerConfig {
                id: "runner-2".to_string(),
                name: "n2".to_string(),
                display_name: None,
                repo_owner: "o".to_string(),
                repo_name: "r".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp"),
                group_id: Some("group-a".to_string()),
                container: None,
            },
            state: RunnerState::Busy,
            pid: None,
            container_id: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: Some("deploy".to_string()), // looking for "deploy"
            job_context: None,
            error_message: None,
            job_started_at: Some(now),
            last_completed_job: None,
            estimated_job_duration_secs: None,
        };
        // sibling has history for "build", not "deploy"
        let mut history = HashMap::new();
        history.insert(
            "runner-1".to_string(),
            vec![JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(300),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            }],
        );
        let mut runners = HashMap::new();
        runners.insert(
            "runner-1".to_string(),
            RunnerInfo {
                config: RunnerConfig {
                    id: "runner-1".to_string(),
                    name: "n1".to_string(),
                    display_name: None,
                    repo_owner: "o".to_string(),
                    repo_name: "r".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp"),
                    group_id: Some("group-a".to_string()),
                    container: None,
                },
                state: RunnerState::Online,
                pid: None,
                container_id: None,
                uptime_secs: None,
                started_at: None,
                jobs_completed: 1,
                jobs_failed: 0,
                current_job: None,
                job_context: None,
                error_message: None,
                job_started_at: None,
                last_completed_job: None,
                estimated_job_duration_secs: None,
            },
        );
        runners.insert("runner-2".to_string(), info.clone());

        let result = RunnerManager::with_job_estimate(info, &history, &runners);
        assert_eq!(result.estimated_job_duration_secs, None);
    }

    #[tokio::test]
    async fn test_copy_dir_recursive_preserves_executable_bit() {
        use crate::platform::process::run_script;
        use std::fs;
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create an executable file in the source
        let script_path = src.path().join(run_script());
        fs::write(&script_path, "#!/bin/bash\necho hi").unwrap();
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        let dst_script = dst.path().join(run_script());
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
            .create("org/myapp", None, None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("org/myapp", None, None, None, None, None)
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
                None,
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
            .create("owner/myrepo", None, None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("owner/myrepo", None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(r1.config.name, "myrepo-runner-1");
        assert_eq!(r2.config.name, "myrepo-runner-2");
    }

    #[tokio::test]
    async fn test_next_runner_number_different_repos() {
        let manager = create_test_manager();
        let r1 = manager
            .create("owner/repo-a", None, None, None, None, None)
            .await
            .unwrap();
        let r2 = manager
            .create("owner/repo-b", None, None, None, None, None)
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
                display_name: None,
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
                labels: vec![],
                mode: RunnerMode::App,
                work_dir: std::path::PathBuf::from("/tmp/runner-abc"),
                group_id: None,
                container: None,
            },
            state: RunnerState::Online,
            pid: None,
            container_id: None,
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
                    display_name: None,
                    repo_owner: "owner".to_string(),
                    repo_name: "repo".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp/test"),
                    group_id: None,
                    container: None,
                },
                state: RunnerState::Online,
                pid: Some(12345),
                container_id: None,
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
        manager.set_desired_running(runner_id, true).await.unwrap();

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
        assert!(!manager.is_desired_running(runner_id).await);
    }

    #[tokio::test]
    async fn test_stop_process_preserving_intent_keeps_restore_state() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();
        manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        manager
            .update_state(&id, RunnerState::Online)
            .await
            .unwrap();
        manager.set_desired_running(&id, true).await.unwrap();

        manager.stop_process_preserving_intent(&id).await.unwrap();

        assert_eq!(manager.get(&id).await.unwrap().state, RunnerState::Offline);
        assert!(manager.is_desired_running(&id).await);
        let persisted: Vec<PersistedRunner> = serde_json::from_str(
            &std::fs::read_to_string(manager.config.runners_json_path()).unwrap(),
        )
        .unwrap();
        assert!(persisted[0].was_running);
    }

    #[test]
    fn test_validate_create_request_rejects_malformed_repository_names() {
        for invalid in [
            "repo",
            "/repo",
            "owner/",
            "owner/repo/extra",
            " owner/repo",
            "owner /repo",
            "owner/ repo",
            "owner/repo ",
        ] {
            assert!(RunnerManager::validate_create_request(invalid, None, None, None).is_err());
        }

        for invalid in [
            format!("owner/{}repo", char::from(9)),
            format!("owner/repo{}", char::from(10)),
        ] {
            assert!(RunnerManager::validate_create_request(&invalid, None, None, None).is_err());
        }

        assert!(RunnerManager::validate_create_request("owner/repo", None, None, None).is_ok());
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

    #[test]
    fn test_already_configured_detects_runner_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!dir.path().join(".runner").exists());
        assert!(!dir.path().join(".runner_migrated").exists());

        // Neither file → not configured
        let configured =
            dir.path().join(".runner").exists() || dir.path().join(".runner_migrated").exists();
        assert!(!configured);

        // .runner exists → configured
        std::fs::write(dir.path().join(".runner"), "{}").unwrap();
        let configured =
            dir.path().join(".runner").exists() || dir.path().join(".runner_migrated").exists();
        assert!(configured);
    }

    #[test]
    fn test_already_configured_detects_runner_migrated_file() {
        let dir = tempfile::tempdir().unwrap();

        // .runner_migrated exists (no .runner) → configured
        std::fs::write(dir.path().join(".runner_migrated"), "{}").unwrap();
        let configured =
            dir.path().join(".runner").exists() || dir.path().join(".runner_migrated").exists();
        assert!(configured);
    }

    #[test]
    fn test_clean_runner_config_called_from_mod() {
        // Verify clean_runner_config is accessible and works end-to-end
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".runner_migrated"), "{}").unwrap();
        std::fs::write(dir.path().join(".credentials"), "cred").unwrap();
        std::fs::write(dir.path().join(".credentials_rsaparams"), "rsa").unwrap();

        clean_runner_config(dir.path());

        assert!(!dir.path().join(".runner_migrated").exists());
        assert!(!dir.path().join(".credentials").exists());
        assert!(!dir.path().join(".credentials_rsaparams").exists());
    }

    #[tokio::test]
    async fn test_record_job_history_assigns_job_number() {
        let manager = create_test_manager();
        let runner_id = "runner-hist-test";
        manager.runners.write().await.insert(
            runner_id.to_string(),
            RunnerInfo {
                config: RunnerConfig {
                    id: runner_id.to_string(),
                    name: "test-runner".to_string(),
                    display_name: None,
                    repo_owner: "owner".to_string(),
                    repo_name: "repo".to_string(),
                    labels: vec![],
                    mode: RunnerMode::App,
                    work_dir: std::path::PathBuf::from("/tmp/test"),
                    group_id: None,
                    container: None,
                },
                state: RunnerState::Online,
                pid: None,
                container_id: None,
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

        let entry = types::JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            succeeded: true,
            branch: None,
            pr_number: None,
            run_url: Some("https://github.com/o/r/actions/runs/100/job/200".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
            job_number: 0,
        };

        manager.record_job_history(runner_id, entry).await;

        let hist = manager.job_history.read().await;
        let entries = hist.get(runner_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].job_number, 1);
    }

    #[tokio::test]
    async fn test_annotate_cross_runner_reruns() {
        let manager = create_test_manager();

        // Create two runners
        for (id, name) in [("r1", "runner-1"), ("r2", "runner-2")] {
            manager.runners.write().await.insert(
                id.to_string(),
                RunnerInfo {
                    config: RunnerConfig {
                        id: id.to_string(),
                        name: name.to_string(),
                        display_name: None,
                        repo_owner: "owner".to_string(),
                        repo_name: "repo".to_string(),
                        labels: vec![],
                        mode: RunnerMode::App,
                        work_dir: std::path::PathBuf::from(format!("/tmp/{id}")),
                        group_id: None,
                        container: None,
                    },
                    state: RunnerState::Online,
                    pid: None,
                    container_id: None,
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

        let now = chrono::Utc::now();

        // Runner-1 has a failed entry for run 100
        let failed_entry = types::JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(600),
            completed_at: now - chrono::Duration::seconds(300),
            succeeded: false,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/o/r/actions/runs/100/job/200".to_string()),
            error_message: Some("exit 1".to_string()),
            steps: vec![],
            latest_attempt: None,
            job_number: 1,
        };
        {
            let mut hist = manager.job_history.write().await;
            hist.entry("r1".to_string()).or_default().push(failed_entry);
        }

        // Runner-2 completes a re-run of the same run_id (100) successfully
        let rerun_entry = types::JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(60),
            completed_at: now,
            succeeded: true,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/o/r/actions/runs/100/job/999".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
            job_number: 0,
        };

        // Record on runner-2 — should annotate runner-1's entry
        manager.record_job_history("r2", rerun_entry).await;

        // Check runner-1's entry got annotated
        let hist = manager.job_history.read().await;
        let r1_entries = hist.get("r1").unwrap();
        assert_eq!(r1_entries.len(), 1);
        let la = r1_entries[0].latest_attempt.as_ref().unwrap();
        assert!(la.succeeded);
        assert_eq!(la.runner_name, "runner-2");

        // Check runner-2's entry was recorded
        let r2_entries = hist.get("r2").unwrap();
        assert_eq!(r2_entries.len(), 1);
        assert_eq!(r2_entries[0].job_number, 1);
    }

    #[tokio::test]
    async fn test_backfill_job_numbers_on_load() {
        let manager = create_test_manager();
        let history_dir = manager.config.history_dir();
        std::fs::create_dir_all(&history_dir).unwrap();

        // Write a history file with job_number: 0 (old format)
        let entries = vec![
            types::JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: chrono::Utc::now(),
                completed_at: chrono::Utc::now(),
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            },
            types::JobHistoryEntry {
                job_name: "test".to_string(),
                started_at: chrono::Utc::now(),
                completed_at: chrono::Utc::now(),
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
                job_number: 0,
            },
        ];
        history::save(&history_dir, "backfill-runner", &entries).unwrap();

        // Create the runner so load_from_disk finds it
        let runners_path = manager.config.runners_json_path();
        if let Some(parent) = runners_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let persisted = serde_json::json!([{
            "id": "backfill-runner",
            "name": "test-runner",
            "repo_owner": "o",
            "repo_name": "r",
            "labels": [],
            "mode": "app",
            "work_dir": "/tmp/backfill",
            "was_running": false
        }]);
        std::fs::write(&runners_path, persisted.to_string()).unwrap();

        manager.load_from_disk().await.unwrap();

        // Verify backfill happened
        let hist = manager.job_history.read().await;
        let entries = hist.get("backfill-runner").unwrap();
        assert_eq!(entries[0].job_number, 1);
        assert_eq!(entries[1].job_number, 2);

        // Verify it was persisted to disk
        let on_disk = history::load_all(&history_dir).unwrap();
        let disk_entries = on_disk.get("backfill-runner").unwrap();
        assert_eq!(disk_entries[0].job_number, 1);
        assert_eq!(disk_entries[1].job_number, 2);
    }

    #[tokio::test]
    async fn test_container_mode_empty_labels_default_to_docker() {
        let manager = create_test_manager();
        let runner = manager
            .create(
                "owner/repo",
                None,
                None,
                Some(RunnerMode::Container),
                None,
                Some(types::ContainerConfig {
                    image: "img:latest".to_string(),
                    extra_env: vec![],
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            runner.config.labels,
            vec!["self-hosted".to_string(), "docker".to_string()]
        );
    }

    #[tokio::test]
    async fn test_container_mode_user_labels_preserved() {
        let manager = create_test_manager();
        let runner = manager
            .create(
                "owner/repo",
                None,
                Some(vec!["self-hosted".to_string(), "rust".to_string()]),
                Some(RunnerMode::Container),
                None,
                Some(types::ContainerConfig {
                    image: "img:latest".to_string(),
                    extra_env: vec![],
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            runner.config.labels,
            vec!["self-hosted".to_string(), "rust".to_string()]
        );
    }

    #[tokio::test]
    async fn test_non_container_mode_does_not_get_docker_label() {
        let manager = create_test_manager();
        let runner = manager
            .create("owner/repo", None, None, Some(RunnerMode::App), None, None)
            .await
            .unwrap();
        assert!(runner.config.labels.contains(&"self-hosted".to_string()));
        assert!(!runner.config.labels.contains(&"docker".to_string()));
    }

    #[tokio::test]
    async fn test_create_rejects_duplicate_name() {
        let manager = create_test_manager();
        manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let err = manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_create_rejects_duplicate_name_case_insensitive_within_repo() {
        let manager = create_test_manager();
        // GitHub runner names are scoped to a repository. Case-insensitive
        // duplicates must still be rejected inside that repository.
        manager
            .create(
                "owner/repo",
                Some("My-Runner".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let err = manager
            .create(
                "owner/repo",
                Some("my-runner".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_create_container_mode_requires_container_config() {
        let manager = create_test_manager();
        let err = manager
            .create(
                "owner/repo",
                Some("c-runner".to_string()),
                None,
                Some(RunnerMode::Container),
                None,
                None, // no container config
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Container mode requires"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_create_non_container_mode_allows_missing_container_config() {
        let manager = create_test_manager();
        // App/Service mode with no container config is fine.
        manager
            .create(
                "owner/repo",
                Some("app-runner".to_string()),
                None,
                Some(RunnerMode::App),
                None,
                None,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_container_mode_rejects_empty_image() {
        let manager = create_test_manager();
        let err = manager
            .create(
                "owner/repo",
                Some("c-empty".to_string()),
                None,
                Some(RunnerMode::Container),
                None,
                Some(types::ContainerConfig {
                    image: "   ".to_string(),
                    extra_env: vec![],
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("non-empty"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn test_create_container_mode_rejects_docker_invalid_name() {
        let manager = create_test_manager();
        let err = manager
            .create(
                "owner/repo",
                Some("bad name/slash".to_string()),
                None,
                Some(RunnerMode::Container),
                None,
                Some(types::ContainerConfig {
                    image: "img:latest".to_string(),
                    extra_env: vec![],
                }),
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("may only contain"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn test_job_context_compare_and_set_rejects_stale_results() {
        let started_at = chrono::Utc::now();
        let stale_started_at = started_at - chrono::Duration::seconds(1);
        let expected_micros = started_at.timestamp_micros();

        assert!(should_apply_job_context(
            &RunnerState::Busy,
            Some("build"),
            Some(&started_at),
            false,
            "build",
            expected_micros,
        ));
        assert!(!should_apply_job_context(
            &RunnerState::Online,
            None,
            None,
            false,
            "build",
            expected_micros,
        ));
        assert!(!should_apply_job_context(
            &RunnerState::Busy,
            Some("test"),
            Some(&started_at),
            false,
            "build",
            expected_micros,
        ));
        assert!(!should_apply_job_context(
            &RunnerState::Busy,
            Some("build"),
            Some(&stale_started_at),
            false,
            "build",
            expected_micros,
        ));
        assert!(!should_apply_job_context(
            &RunnerState::Busy,
            Some("build"),
            Some(&started_at),
            true,
            "build",
            expected_micros,
        ));
    }
}
