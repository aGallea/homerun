pub mod binary;
pub mod process;
pub mod state;
pub mod types;

use anyhow::{bail, Result};
use serde::Serialize;
use state::RunnerState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use types::{RunnerConfig, RunnerInfo, RunnerMode};
use crate::config::Config;

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
    log_tx: Arc<broadcast::Sender<LogEntry>>,
}

impl RunnerManager {
    pub fn new(config: Config) -> Self {
        let (log_tx, _) = broadcast::channel(1024);
        Self {
            config: Arc::new(config),
            runners: Arc::new(RwLock::new(HashMap::new())),
            log_tx: Arc::new(log_tx),
        }
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<LogEntry> {
        self.log_tx.subscribe()
    }

    pub fn log_sender(&self) -> &broadcast::Sender<LogEntry> {
        &self.log_tx
    }

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
        let count = self.runners.read().await.values()
            .filter(|r| r.config.repo_name == repo)
            .count();
        let name = name.unwrap_or_else(|| format!("{repo}-runner-{}", count + 1));
        let work_dir = self.config.runners_dir().join(&id);
        std::fs::create_dir_all(&work_dir)?;

        let mut default_labels = vec![
            "self-hosted".to_string(),
            "macOS".to_string(),
        ];
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
            jobs_completed: 0,
            jobs_failed: 0,
        };

        self.runners.write().await.insert(id, runner.clone());
        Ok(runner)
    }

    pub async fn list(&self) -> Vec<RunnerInfo> {
        self.runners.read().await.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<RunnerInfo> {
        self.runners.read().await.get(id).cloned()
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut runners = self.runners.write().await;
        if let Some(runner) = runners.remove(id) {
            let _ = std::fs::remove_dir_all(&runner.config.work_dir);
        }
        Ok(())
    }

    pub async fn update(&self, id: &str, req: types::UpdateRunnerRequest) -> Result<RunnerInfo> {
        let mut runners = self.runners.write().await;
        let runner = runners.get_mut(id).ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
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
        let runner = runners.get_mut(id).ok_or_else(|| anyhow::anyhow!("Runner not found"))?;
        if !runner.state.can_transition_to(&state) {
            bail!("Invalid state transition: {:?} -> {:?}", runner.state, state);
        }
        runner.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use state::RunnerState;

    #[tokio::test]
    async fn test_log_broadcast() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);
        let mut rx = manager.subscribe_logs();

        manager.log_sender().send(LogEntry {
            runner_id: "test".to_string(),
            timestamp: chrono::Utc::now(),
            line: "hello".to_string(),
            stream: "stdout".to_string(),
        }).unwrap();

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

        manager.create("aGallea/gifted", None, None, None).await.unwrap();
        manager.create("aGallea/gifted", None, None, None).await.unwrap();

        let runners = manager.list().await;
        assert_eq!(runners.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_runner() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let manager = RunnerManager::new(config);

        let runner = manager.create("aGallea/gifted", None, None, None).await.unwrap();
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

        let runner = manager.create("aGallea/gifted", None, None, None).await.unwrap();
        assert_eq!(runner.state, RunnerState::Creating);

        manager.update_state(&runner.config.id, RunnerState::Registering).await.unwrap();
        manager.update_state(&runner.config.id, RunnerState::Online).await.unwrap();

        let updated = manager.get(&runner.config.id).await.unwrap();
        assert_eq!(updated.state, RunnerState::Online);

        // Invalid transition should fail
        let result = manager.update_state(&runner.config.id, RunnerState::Creating).await;
        assert!(result.is_err());
    }
}
