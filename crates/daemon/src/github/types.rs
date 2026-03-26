use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub id: u64,
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub private: bool,
    pub html_url: String,
    pub is_org: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerRegistration {
    pub token: String,
    pub expires_at: String,
}

/// Status of a job from the latest attempt of a workflow run.
/// Used by the re-run poller to detect outcome changes.
#[derive(Debug, Clone)]
pub struct LatestJobStatus {
    pub job_id: u64,
    pub succeeded: bool,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub run_attempt: u32,
    pub runner_name: Option<String>,
}
