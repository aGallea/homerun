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
                    full_name: repo.full_name.clone().unwrap_or_else(|| {
                        format!("{}/{}", owner.login, repo.name)
                    }),
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
        let response: RegistrationTokenResponse =
            self.octocrab.post(route, None::<&()>).await?;

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

    #[tokio::test]
    async fn test_github_client_with_token() {
        let client = GitHubClient::new(Some("ghp_test".to_string()));
        assert!(client.is_ok());
    }
}
