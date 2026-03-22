use crate::runner::state::RunnerState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunnerMode {
    App,
    Service,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: RunnerState,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_job: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_context: Option<JobContext>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRunnerRequest {
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_deserialize_without_group_id() {
        let json = r#"{"id":"abc-123","name":"test-runner-1","repo_owner":"owner","repo_name":"repo","labels":["self-hosted"],"mode":"app","work_dir":"/tmp/runners/abc-123"}"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.group_id, None);
    }

    #[test]
    fn test_runner_config_deserialize_with_group_id() {
        let json = r#"{"id":"abc-123","name":"test-runner-1","repo_owner":"owner","repo_name":"repo","labels":["self-hosted"],"mode":"app","work_dir":"/tmp/runners/abc-123","group_id":"group-uuid-456"}"#;
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
    fn test_create_batch_request_deserializes() {
        let json = r#"{"repo_full_name":"owner/repo","count":3}"#;
        let req: CreateBatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.count, 3);
    }
}
