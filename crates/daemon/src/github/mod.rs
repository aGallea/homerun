pub mod types;

use anyhow::{bail, Result};
use serde::Deserialize;
use types::{RepoInfo, RunnerRegistration};

pub struct GitHubClient {
    octocrab: octocrab::Octocrab,
    token: String,
}

impl GitHubClient {
    pub fn new(token: Option<String>) -> Result<Self> {
        let Some(token) = token else {
            bail!("Not authenticated — please login first");
        };
        let octocrab = octocrab::Octocrab::builder()
            .personal_token(token.clone())
            .build()?;
        Ok(Self { octocrab, token })
    }

    pub async fn list_repos(&self) -> Result<Vec<RepoInfo>> {
        let first_page = self
            .octocrab
            .current()
            .list_repos_for_authenticated_user()
            .per_page(100)
            .send()
            .await?;

        let repos = self.octocrab.all_pages(first_page).await?;

        let result = repos
            .into_iter()
            .filter_map(|repo| {
                let owner = repo.owner.as_ref()?;
                let is_org = owner.r#type == "Organization";
                Some(RepoInfo {
                    id: repo.id.0,
                    full_name: repo
                        .full_name
                        .clone()
                        .unwrap_or_else(|| format!("{}/{}", owner.login, repo.name)),
                    name: repo.name.clone(),
                    owner: owner.login.clone(),
                    private: repo.private.unwrap_or(false),
                    html_url: repo
                        .html_url
                        .as_ref()
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                    is_org,
                })
            })
            .collect();

        Ok(result)
    }

    /// Fetch `.github/workflows/` contents for `owner/repo` and return the
    /// relative file paths of workflow files that contain `runs-on: self-hosted`.
    pub async fn list_self_hosted_workflows(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct ContentItem {
            name: String,
            download_url: Option<String>,
            #[serde(rename = "type")]
            item_type: String,
        }

        let route = format!("/repos/{owner}/{repo}/contents/.github/workflows");
        let items: Vec<ContentItem> = match self.octocrab.get(route, None::<&()>).await {
            Ok(v) => v,
            Err(_) => return Ok(Vec::new()), // directory missing or no access
        };

        let mut matching: Vec<String> = Vec::new();

        for item in items {
            if item.item_type != "file" {
                continue;
            }
            let ext = std::path::Path::new(&item.name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if ext != "yml" && ext != "yaml" {
                continue;
            }

            let download_url = match item.download_url {
                Some(u) => u,
                None => continue,
            };

            // _get fetches a URL; body_to_string reads the response body
            let response = match self.octocrab._get(download_url).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let content = match self.octocrab.body_to_string(response).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            if content.contains("runs-on: self-hosted") {
                matching.push(format!(".github/workflows/{}", item.name));
            }
        }

        matching.sort();
        Ok(matching)
    }

    pub async fn get_runner_registration_token(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<RunnerRegistration> {
        #[derive(Deserialize)]
        struct RegistrationTokenResponse {
            token: String,
            expires_at: String,
        }

        let route = format!("/repos/{owner}/{repo}/actions/runners/registration-token");
        let response: RegistrationTokenResponse = self.octocrab.post(route, None::<&()>).await?;

        Ok(RunnerRegistration {
            token: response.token,
            expires_at: response.expires_at,
        })
    }

    /// Find the in-progress workflow run that matches this runner.
    ///
    /// Matches by job name (from runner stdout) against the GitHub API's job
    /// list, since the `runner_name` field on jobs may not be populated yet
    /// while a job is still in progress.
    pub async fn get_active_run_for_runner(
        &self,
        owner: &str,
        repo: &str,
        runner_name: &str,
        job_name: &str,
    ) -> Result<Option<crate::runner::types::JobContext>> {
        #[derive(Deserialize)]
        struct WorkflowRun {
            id: u64,
            head_branch: String,
            html_url: String,
            pull_requests: Vec<PullRequestRef>,
        }

        #[derive(Deserialize)]
        struct PullRequestRef {
            number: u64,
            url: String,
        }

        #[derive(Deserialize)]
        struct RunsResponse {
            workflow_runs: Vec<WorkflowRun>,
        }

        #[derive(Deserialize)]
        struct RunJob {
            id: u64,
            name: String,
            runner_name: Option<String>,
        }

        #[derive(Deserialize)]
        struct JobsResponse {
            jobs: Vec<RunJob>,
        }

        let route = format!("/repos/{owner}/{repo}/actions/runs?status=in_progress");
        let runs: RunsResponse = match self.octocrab.get(&route, None::<&()>).await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        for run in runs.workflow_runs {
            let jobs_route = format!("/repos/{owner}/{repo}/actions/runs/{}/jobs", run.id);
            let jobs: JobsResponse = match self.octocrab.get(&jobs_route, None::<&()>).await {
                Ok(j) => j,
                Err(_) => continue,
            };

            // Match by runner_name if available, otherwise fall back to job name
            let matched_job = jobs.jobs.iter().find(|j| {
                if let Some(rn) = j.runner_name.as_deref() {
                    rn == runner_name
                } else {
                    j.name == job_name
                }
            });

            if let Some(job) = matched_job {
                let (pr_number, pr_url) = if let Some(pr) = run.pull_requests.first() {
                    let html_url = pr
                        .url
                        .replace("api.github.com/repos", "github.com")
                        .replace("/pulls/", "/pull/");
                    (Some(pr.number), Some(html_url))
                } else {
                    (None, None)
                };

                return Ok(Some(crate::runner::types::JobContext {
                    branch: run.head_branch,
                    pr_number,
                    pr_url,
                    run_url: run.html_url,
                    job_id: Some(job.id),
                }));
            }
        }

        Ok(None)
    }

    /// Fetch the raw log content for a specific job from GitHub Actions.
    ///
    /// The GitHub API endpoint returns a 302 redirect to blob storage serving
    /// plain text. Since octocrab expects JSON responses, we use reqwest directly.
    pub async fn get_job_logs(&self, owner: &str, repo: &str, job_id: u64) -> Result<String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/actions/jobs/{job_id}/logs");
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "homerun")
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "Failed to fetch job logs: HTTP {}",
                response.status().as_u16()
            );
        }

        let body = response.text().await?;
        Ok(body)
    }
}

/// Parse raw GitHub Actions job log text into sections by step name.
///
/// The log format uses `##[group]Step Name` / `##[endgroup]` markers to delimit
/// steps, with each line prefixed by a 29-character timestamp
/// (`2026-03-23T07:54:51.0000000Z `).
///
/// Returns a `Vec<(step_name, log_content)>`.
pub fn parse_job_log_sections(raw_log: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in raw_log.lines() {
        // Strip the timestamp prefix (29 chars: 28 for timestamp + 'Z', then a space)
        let content = if line.len() > 29 { &line[29..] } else { line };
        let content = content.trim_start();

        if let Some(name) = content.strip_prefix("##[group]") {
            // Close any open section before starting a new one
            if let Some(name) = current_name.take() {
                sections.push((name, current_lines.join("\n")));
                current_lines.clear();
            }
            current_name = Some(name.to_string());
        } else if content == "##[endgroup]" {
            if let Some(name) = current_name.take() {
                sections.push((name, current_lines.join("\n")));
                current_lines.clear();
            }
        } else if current_name.is_some() {
            current_lines.push(content.to_string());
        }
    }

    // Handle unterminated section (step still running)
    if let Some(name) = current_name.take() {
        sections.push((name, current_lines.join("\n")));
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_client_requires_token() {
        let result = GitHubClient::new(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_github_client_none_error_message() {
        let err = GitHubClient::new(None).err().unwrap();
        assert!(err.to_string().contains("Not authenticated") || err.to_string().contains("login"));
    }

    #[tokio::test]
    async fn test_github_client_with_token() {
        let client = GitHubClient::new(Some("ghp_test".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_github_client_with_some_token_creates_ok() {
        // Any non-empty token string should create a client (validation happens on API call)
        let client = GitHubClient::new(Some("fake-token-value".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_github_client_empty_token_creates_ok() {
        // An empty token string is still a Some(...), so it should build successfully
        // — authentication errors only surface when making actual API calls.
        let result = GitHubClient::new(Some(String::new()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_github_client_none_error_contains_not_authenticated() {
        let err = GitHubClient::new(None).err().unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("Not authenticated"),
            "expected 'Not authenticated' in error, got: {msg}"
        );
    }

    #[test]
    fn test_github_client_none_error_contains_login() {
        let err = GitHubClient::new(None).err().unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("login"),
            "expected 'login' in error, got: {msg}"
        );
    }

    /// Verify that list_repos returns an error (not a panic) when called
    /// with an invalid token — exercises the API call error path.
    #[tokio::test]
    async fn test_list_repos_with_invalid_token_returns_err() {
        let client = GitHubClient::new(Some("invalid_token_xyz".to_string())).unwrap();
        let result = client.list_repos().await;
        assert!(
            result.is_err(),
            "expected API call to fail with invalid token"
        );
    }

    /// Verify that get_runner_registration_token returns an error for an invalid token.
    #[tokio::test]
    async fn test_get_runner_registration_token_invalid_token_returns_err() {
        let client = GitHubClient::new(Some("invalid_token_xyz".to_string())).unwrap();
        let result = client
            .get_runner_registration_token("fake-owner", "fake-repo")
            .await;
        assert!(
            result.is_err(),
            "expected API call to fail with invalid token"
        );
    }

    /// Verify that list_self_hosted_workflows returns an empty vec (not an error)
    /// when the workflows directory is inaccessible (error is swallowed by design).
    #[tokio::test]
    async fn test_list_self_hosted_workflows_inaccessible_repo_returns_empty() {
        let client = GitHubClient::new(Some("invalid_token_xyz".to_string())).unwrap();
        let result = client
            .list_self_hosted_workflows("fake-owner-xyz-123", "fake-repo-xyz-456")
            .await;
        // By design the function swallows errors and returns Ok(Vec::new())
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_runner_registration_type_fields() {
        use types::RunnerRegistration;
        let reg = RunnerRegistration {
            token: "token123".to_string(),
            expires_at: "2030-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(reg.token, "token123");
        assert_eq!(reg.expires_at, "2030-01-01T00:00:00Z");
    }

    #[test]
    fn test_repo_info_fields() {
        use types::RepoInfo;
        let repo = RepoInfo {
            id: 42,
            full_name: "owner/repo".to_string(),
            name: "repo".to_string(),
            owner: "owner".to_string(),
            private: true,
            html_url: "https://github.com/owner/repo".to_string(),
            is_org: false,
        };
        assert_eq!(repo.id, 42);
        assert!(repo.private);
        assert!(!repo.is_org);
    }

    #[test]
    fn test_repo_info_is_org_true() {
        use types::RepoInfo;
        let repo = RepoInfo {
            id: 1,
            full_name: "myorg/tool".to_string(),
            name: "tool".to_string(),
            owner: "myorg".to_string(),
            private: false,
            html_url: "https://github.com/myorg/tool".to_string(),
            is_org: true,
        };
        assert!(repo.is_org);
        assert!(!repo.private);
    }

    #[test]
    fn test_repo_info_serialization_roundtrip() {
        use types::RepoInfo;
        let repo = RepoInfo {
            id: 99,
            full_name: "a/b".to_string(),
            name: "b".to_string(),
            owner: "a".to_string(),
            private: false,
            html_url: "https://github.com/a/b".to_string(),
            is_org: false,
        };
        let json = serde_json::to_string(&repo).unwrap();
        let back: RepoInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, repo.id);
        assert_eq!(back.full_name, repo.full_name);
        assert_eq!(back.is_org, repo.is_org);
    }

    #[test]
    fn test_runner_registration_serialization_roundtrip() {
        use types::RunnerRegistration;
        let reg = RunnerRegistration {
            token: "abc123".to_string(),
            expires_at: "2030-12-31T23:59:59Z".to_string(),
        };
        let json = serde_json::to_string(&reg).unwrap();
        let back: RunnerRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(back.token, "abc123");
        assert_eq!(back.expires_at, "2030-12-31T23:59:59Z");
    }

    #[tokio::test]
    async fn test_github_client_builder_with_valid_token() {
        // Creating a client with a valid-format token should succeed
        let client = GitHubClient::new(Some("ghp_abcdefghij0123456789".to_string()));
        assert!(
            client.is_ok(),
            "valid-format token should build client successfully"
        );
    }

    #[tokio::test]
    async fn test_list_repos_with_empty_token_returns_err() {
        // An empty string token will fail GitHub's API validation
        let client = GitHubClient::new(Some(String::new())).unwrap();
        let result = client.list_repos().await;
        assert!(result.is_err(), "expected error with empty token");
    }

    #[test]
    fn test_parse_job_log_into_steps() {
        let raw_log = "2026-03-23T07:54:51.0000000Z ##[group]Run actions/checkout@v6\n\
            2026-03-23T07:54:52.0000000Z Syncing repository: owner/repo\n\
            2026-03-23T07:54:53.0000000Z ##[endgroup]\n\
            2026-03-23T07:54:53.0000000Z ##[group]Check formatting\n\
            2026-03-23T07:54:54.0000000Z + cargo fmt --check\n\
            2026-03-23T07:54:55.0000000Z ##[endgroup]\n";

        let sections = parse_job_log_sections(raw_log);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "Run actions/checkout@v6");
        assert!(sections[0].1.contains("Syncing repository"));
        assert_eq!(sections[1].0, "Check formatting");
        assert!(sections[1].1.contains("cargo fmt"));
    }

    #[test]
    fn test_parse_job_log_unterminated_section() {
        let raw_log = "2026-03-23T07:54:51.0000000Z ##[group]Running step\n\
            2026-03-23T07:54:52.0000000Z Some output here\n";

        let sections = parse_job_log_sections(raw_log);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Running step");
        assert!(sections[0].1.contains("Some output here"));
    }

    #[test]
    fn test_parse_job_log_empty_input() {
        let sections = parse_job_log_sections("");
        assert!(sections.is_empty());
    }

    #[tokio::test]
    async fn test_list_self_hosted_workflows_empty_token_returns_empty() {
        // Even with an empty token, the function should return Ok(empty vec)
        // because it swallows directory-access errors.
        let client = GitHubClient::new(Some(String::new())).unwrap();
        let result = client
            .list_self_hosted_workflows("any-owner", "any-repo")
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
