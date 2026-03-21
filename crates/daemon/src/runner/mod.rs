pub mod binary;
pub mod process;
pub mod state;
pub mod types;

use crate::config::Config;
use crate::github::GitHubClient;
use crate::runner::binary::ensure_runner_binary;
use crate::runner::process::{configure_runner, remove_runner, start_runner};
use crate::runner::state::RunnerState;
use crate::runner::types::{RunnerConfig, RunnerInfo, RunnerMode};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::{broadcast, RwLock};

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

#[derive(Clone)]
pub struct RunnerManager {
    config: Arc<Config>,
    runners: Arc<RwLock<HashMap<String, RunnerInfo>>>,
    processes: Arc<RwLock<HashMap<String, Arc<RwLock<Child>>>>>,
    log_tx: Arc<broadcast::Sender<LogEntry>>,
    event_tx: Arc<broadcast::Sender<RunnerEvent>>,
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
        }
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<LogEntry> {
        self.log_tx.subscribe()
    }

    pub fn log_sender(&self) -> &broadcast::Sender<LogEntry> {
        &self.log_tx
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

    // ── Persistence ────────────────────────────────────────────────

    /// Save all runner configs to disk as JSON.
    pub async fn save_to_disk(&self) -> Result<()> {
        let runners = self.runners.read().await;
        let configs: Vec<&RunnerConfig> = runners.values().map(|r| &r.config).collect();
        let json = serde_json::to_string_pretty(&configs)?;
        let path = self.config.runners_json_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load runner configs from disk, creating entries in Offline state.
    pub async fn load_from_disk(&self) -> Result<()> {
        let path = self.config.runners_json_path();
        if !path.exists() {
            return Ok(());
        }
        let json = std::fs::read_to_string(&path)?;
        let configs: Vec<RunnerConfig> = serde_json::from_str(&json)?;
        let mut runners = self.runners.write().await;
        for config in configs {
            let id = config.id.clone();
            runners.insert(
                id,
                RunnerInfo {
                    config,
                    state: RunnerState::Offline,
                    pid: None,
                    uptime_secs: None,
                    started_at: None,
                    jobs_completed: 0,
                    jobs_failed: 0,
                },
            );
        }
        Ok(())
    }

    // ── CRUD ───────────────────────────────────────────────────────

    pub async fn create(
        &self,
        repo_full_name: &str,
        name: Option<String>,
        labels: Option<Vec<String>>,
        mode: Option<RunnerMode>,
    ) -> Result<RunnerInfo> {
        let parts: Vec<&str> = repo_full_name.split('/').collect();
        if parts.len() != 2 {
            bail!("Invalid repo name: expected 'owner/repo'");
        }
        let (owner, repo) = (parts[0], parts[1]);

        let id = uuid::Uuid::new_v4().to_string();
        let count = self
            .runners
            .read()
            .await
            .values()
            .filter(|r| r.config.repo_name == repo)
            .count();
        let name = name.unwrap_or_else(|| format!("{repo}-runner-{}", count + 1));
        let work_dir = self.config.runners_dir().join(&id);
        std::fs::create_dir_all(&work_dir)?;

        let mut default_labels = vec!["self-hosted".to_string(), "macOS".to_string()];
        if cfg!(target_arch = "aarch64") {
            default_labels.push("ARM64".to_string());
        } else {
            default_labels.push("X64".to_string());
        }
        if let Some(extra) = labels {
            default_labels.extend(extra);
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
            },
            state: RunnerState::Creating,
            pid: None,
            uptime_secs: None,
            started_at: None,
            jobs_completed: 0,
            jobs_failed: 0,
        };

        self.runners.write().await.insert(id, runner.clone());
        self.save_to_disk().await?;
        Ok(runner)
    }

    pub async fn list(&self) -> Vec<RunnerInfo> {
        self.runners
            .read()
            .await
            .values()
            .cloned()
            .map(Self::with_computed_uptime)
            .collect()
    }

    pub async fn get(&self, id: &str) -> Option<RunnerInfo> {
        self.runners
            .read()
            .await
            .get(id)
            .cloned()
            .map(Self::with_computed_uptime)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut runners = self.runners.write().await;
        if let Some(runner) = runners.remove(id) {
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
        }
        drop(runners);
        // Also remove any tracked process handle
        self.processes.write().await.remove(id);
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
        runner.state = state;
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
    /// If the runner is already configured (.runner file exists), skip download + config
    /// and go straight to starting run.sh.
    async fn do_register_and_start(&self, id: &str, auth_token: &str) -> Result<()> {
        let runner = self
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        let config = &runner.config;

        let already_configured = config.work_dir.join(".runner").exists();

        if !already_configured {
            // 1. Download / cache runner binary
            let cached_runner_dir = ensure_runner_binary(&self.config.cache_dir())
                .await
                .context("Failed to download runner binary")?;

            // 2. Copy binary files to runner work_dir
            copy_dir_recursive(&cached_runner_dir, &config.work_dir)
                .context("Failed to copy runner binary to work dir")?;

            // 3. Get registration token
            let gh = GitHubClient::new(Some(auth_token.to_string()))?;
            let reg = gh
                .get_runner_registration_token(&config.repo_owner, &config.repo_name)
                .await
                .context("Failed to get registration token")?;

            // 4. Run config.sh
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
        } else {
            tracing::info!("Runner {} already configured, skipping download + config", id);
        }

        // 5. Spawn run.sh
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

        // 5c. Spawn log reader tasks
        if let Some(stdout) = stdout {
            let log_tx = self.log_tx.clone();
            let rid = id.to_string();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = log_tx.send(LogEntry {
                        runner_id: rid.clone(),
                        timestamp: chrono::Utc::now(),
                        line,
                        stream: "stdout".to_string(),
                    });
                }
            });
        }
        if let Some(stderr) = stderr {
            let log_tx = self.log_tx.clone();
            let rid = id.to_string();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = log_tx.send(LogEntry {
                        runner_id: rid.clone(),
                        timestamp: chrono::Utc::now(),
                        line,
                        stream: "stderr".to_string(),
                    });
                }
            });
        }

        // Store process handle
        let child_arc = Arc::new(RwLock::new(child));
        self.processes
            .write()
            .await
            .insert(id.to_string(), child_arc.clone());

        // 7. Spawn background monitor task
        let manager = self.clone();
        let runner_id = id.to_string();
        tokio::spawn(async move {
            // Wait for child to exit
            let exit_status = {
                let mut child_guard = child_arc.write().await;
                child_guard.wait().await
            };
            tracing::info!("Runner {} exited with status: {:?}", runner_id, exit_status);

            // Update state to Offline
            let mut runners = manager.runners.write().await;
            if let Some(r) = runners.get_mut(&runner_id) {
                // Only transition if still Online or Busy (not if already Stopping/Deleting)
                if r.state == RunnerState::Online || r.state == RunnerState::Busy {
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
    pub async fn stop_process(&self, id: &str) -> Result<()> {
        // Transition to Stopping
        self.update_state(id, RunnerState::Stopping).await?;
        self.emit_state_event(id, "stopping");

        // Kill the child process
        if let Some(child_arc) = self.processes.read().await.get(id).cloned() {
            let mut child = child_arc.write().await;
            let _ = child.kill().await;
            // Wait for the process to fully exit
            let _ = child.wait().await;
        }

        // Update to Offline
        {
            let mut runners = self.runners.write().await;
            if let Some(r) = runners.get_mut(id) {
                r.state = RunnerState::Offline;
                r.pid = None;
            }
        }
        self.processes.write().await.remove(id);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use state::RunnerState;

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
            .create("aGallea/gifted", None, None, None)
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
            .create("aGallea/gifted", None, None, None)
            .await
            .unwrap();
        manager
            .create("aGallea/gifted", None, None, None)
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
            .create("aGallea/gifted", None, None, None)
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
            .create("aGallea/gifted", None, None, None)
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
            .create("owner/repo1", None, None, None)
            .await
            .unwrap();
        manager
            .create("owner/repo2", None, None, None)
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
}
