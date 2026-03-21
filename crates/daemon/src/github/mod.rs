pub mod types;

use anyhow::{bail, Result};
use serde::Deserialize;
use types::{RepoInfo, RunnerRegistration};

pub struct GitHubClient {
    octocrab: octocrab::Octocrab,
}

impl GitHubClient {
    pub fn new(token: Option<String>) -> Result<Self> {
        let Some(token) = token else {
            bail!("Not authenticated — please login first");
        };
        let octocrab = octocrab::Octocrab::builder()
            .personal_token(token)
            .build()?;
        Ok(Self { octocrab })
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
}
