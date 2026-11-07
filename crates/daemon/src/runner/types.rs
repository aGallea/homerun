use crate::runner::state::RunnerState;
use crate::runner::steps::StepInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_DISPLAY_NAME_CHARS: usize = 100;
const MAX_LABEL_CHARS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunnerMode {
    App,
    Service,
    Container,
}

/// Configuration for a `RunnerMode::Container` runner. The runner binary
/// itself is never baked into the image — it's bind-mounted from the same
/// cache `ensure_runner_binary` already maintains for native runners — so
/// `image` only needs to provide an OS + toolchain (a first-party base image
/// or any user-supplied registry ref).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerConfig {
    pub image: String,
    #[serde(default)]
    pub extra_env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub id: String,
    /// Technical name registered with GitHub Actions. This must never be changed by display-name edits.
    pub name: String,
    /// Optional local alias shown by HomeRun instead of the technical GitHub runner name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub display_name: Option<String>,
    pub repo_owner: String,
    pub repo_name: String,
    pub labels: Vec<String>,
    pub mode: RunnerMode,
    pub work_dir: std::path::PathBuf,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub container: Option<ContainerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryEntry {
    pub job_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub succeeded: bool,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_message: Option<String>,
    pub steps: Vec<StepInfo>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub latest_attempt: Option<RunAttempt>,
    /// Stable sequential number assigned when the job is recorded. Survives deletions.
    #[serde(default)]
    pub job_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job_name: String,
    pub succeeded: bool,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub duration_secs: u64,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub latest_attempt: Option<RunAttempt>,
}

/// One attempt of a workflow run. Building block for re-run tracking.
/// Phase A: stored as `Option<RunAttempt>` on history entries.
/// Phase B (#92): will expand to `Vec<RunAttempt>` for full timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAttempt {
    pub attempt: u32,
    pub succeeded: bool,
    pub runner_name: String,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: RunnerState,
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub container_id: Option<String>,
    pub uptime_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_job: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_context: Option<JobContext>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub job_started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_completed_job: Option<CompletedJob>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub estimated_job_duration_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
    #[serde(default)]
    pub container: Option<ContainerConfig>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRunnerRequest {
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
    /// Outer `Option` distinguishes an omitted field from an explicit null used to clear the alias.
    #[serde(default, deserialize_with = "deserialize_display_name_update")]
    pub display_name: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBatchRequest {
    pub repo_full_name: String,
    pub count: u8,
    #[serde(default)]
    pub name_prefix: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
    #[serde(default)]
    pub container: Option<ContainerConfig>,
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

fn deserialize_display_name_update<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

pub(crate) fn normalize_labels(labels: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for label in labels {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_LABEL_CHARS {
            anyhow::bail!("Runner labels must be at most {MAX_LABEL_CHARS} characters");
        }
        if trimmed.contains(',') || trimmed.chars().any(char::is_control) {
            anyhow::bail!("Runner labels cannot contain commas or control characters");
        }

        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            normalized.push(trimmed.to_string());
        }
    }

    Ok(normalized)
}

pub(crate) fn normalize_display_name(value: Option<String>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_CHARS {
        anyhow::bail!("Display name must be at most {MAX_DISPLAY_NAME_CHARS} characters");
    }
    if trimmed.chars().any(char::is_control) {
        anyhow::bail!("Display name cannot contain control characters");
    }

    Ok(Some(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_deserialize_without_optional_fields() {
        let json = r#"{"id":"abc-123","name":"test-runner-1","repo_owner":"owner","repo_name":"repo","labels":["self-hosted"],"mode":"app","work_dir":"/tmp/runners/abc-123"}"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.group_id, None);
        assert_eq!(config.display_name, None);
    }

    #[test]
    fn test_runner_config_deserialize_with_optional_fields() {
        let json = r#"{"id":"abc-123","name":"test-runner-1","display_name":"Build machine","repo_owner":"owner","repo_name":"repo","labels":["self-hosted"],"mode":"app","work_dir":"/tmp/runners/abc-123","group_id":"group-uuid-456"}"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.group_id, Some("group-uuid-456".to_string()));
        assert_eq!(config.display_name, Some("Build machine".to_string()));
    }

    #[test]
    fn test_runner_config_serialize_without_optional_fields_omits_them() {
        let config = RunnerConfig {
            id: "abc".to_string(),
            name: "test".to_string(),
            display_name: None,
            repo_owner: "owner".to_string(),
            repo_name: "repo".to_string(),
            labels: vec![],
            mode: RunnerMode::App,
            work_dir: std::path::PathBuf::from("/tmp"),
            group_id: None,
            container: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("group_id"));
        assert!(!json.contains("display_name"));
    }

    #[test]
    fn test_normalize_labels_trims_deduplicates_and_rejects_unsafe_values() {
        let labels = normalize_labels(vec![
            " rust ".to_string(),
            "RUST".to_string(),
            "docker".to_string(),
            "".to_string(),
        ])
        .unwrap();
        assert_eq!(labels, vec!["rust", "docker"]);
        assert!(normalize_labels(vec!["bad,label".to_string()]).is_err());
        assert!(normalize_labels(vec!["bad\nlabel".to_string()]).is_err());
    }

    #[test]
    fn test_update_request_distinguishes_omitted_clear_and_set() {
        let omitted: UpdateRunnerRequest = serde_json::from_str(r#"{}"#).unwrap();
        assert!(omitted.display_name.is_none());

        let clear: UpdateRunnerRequest = serde_json::from_str(r#"{"display_name":null}"#).unwrap();
        assert_eq!(clear.display_name, Some(None));

        let set: UpdateRunnerRequest =
            serde_json::from_str(r#"{"display_name":"Office Mac"}"#).unwrap();
        assert_eq!(set.display_name, Some(Some("Office Mac".to_string())));
    }

    #[test]
    fn test_normalize_display_name() {
        assert_eq!(
            normalize_display_name(Some("  Office Mac  ".to_string())).unwrap(),
            Some("Office Mac".to_string())
        );
        assert_eq!(
            normalize_display_name(Some("   ".to_string())).unwrap(),
            None
        );
        assert_eq!(normalize_display_name(None).unwrap(), None);
        assert!(normalize_display_name(Some("bad\nname".to_string())).is_err());
        assert!(normalize_display_name(Some("x".repeat(101))).is_err());
    }

    #[test]
    fn test_runner_config_deserialize_without_container() {
        let json = r#"{"id":"abc-123","name":"test-runner-1","repo_owner":"owner","repo_name":"repo","labels":["self-hosted"],"mode":"container","work_dir":"/tmp/runners/abc-123"}"#;
        let config: RunnerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.container, None);
    }

    #[test]
    fn test_runner_config_roundtrip_with_container() {
        let config = RunnerConfig {
            id: "abc".to_string(),
            name: "test".to_string(),
            display_name: None,
            repo_owner: "owner".to_string(),
            repo_name: "repo".to_string(),
            labels: vec![],
            mode: RunnerMode::Container,
            work_dir: std::path::PathBuf::from("/tmp"),
            group_id: None,
            container: Some(ContainerConfig {
                image: "ghcr.io/agallea/homerun-runner:ubuntu-24.04".to_string(),
                extra_env: vec![],
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let round_tripped: RunnerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            round_tripped.container.map(|c| c.image),
            Some("ghcr.io/agallea/homerun-runner:ubuntu-24.04".to_string())
        );
    }

    #[test]
    fn test_create_batch_request_deserializes() {
        let json = r#"{"repo_full_name":"owner/repo","count":3}"#;
        let req: CreateBatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.count, 3);
    }
}
