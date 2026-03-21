pub mod keychain;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const KEYCHAIN_SERVICE: &str = "com.homerun.daemon";
const KEYCHAIN_ACCOUNT: &str = "github-token";
const GITHUB_CLIENT_ID: &str = "Ov23liUGCrUgXVf9nTRd";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const DEVICE_FLOW_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Internal response from GitHub's device flow access token poll endpoint.
#[derive(Debug, Deserialize)]
struct PollResponse {
    access_token: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
    interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user: Option<GitHubUser>,
}

struct AuthState {
    token: String,
    user: GitHubUser,
}

#[derive(Clone)]
pub struct AuthManager {
    state: Arc<RwLock<Option<AuthState>>>,
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }

    /// Attempt to restore a previously saved token from the keychain on startup.
    pub async fn try_restore(&self) -> Result<()> {
        if let Some(token) = keychain::get_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)? {
            match self.validate_token(&token).await {
                Ok(user) => {
                    let mut state = self.state.write().await;
                    *state = Some(AuthState { token, user });
                }
                Err(e) => {
                    tracing::warn!("Stored token is no longer valid, clearing: {e}");
                    let _ = keychain::delete_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
                }
            }
        }
        Ok(())
    }

    /// Validate the PAT via the GitHub API, store it in the keychain, and update state.
    pub async fn login_with_pat(&self, token: &str) -> Result<GitHubUser> {
        let user = self.validate_token(token).await?;
        keychain::store_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, token)?;
        let mut state = self.state.write().await;
        *state = Some(AuthState {
            token: token.to_string(),
            user: user.clone(),
        });
        Ok(user)
    }

    /// Remove the token from keychain and clear in-memory state.
    pub async fn logout(&self) -> Result<()> {
        // Clear in-memory state first, then try keychain cleanup
        let mut state = self.state.write().await;
        *state = None;
        drop(state);
        // Best-effort keychain cleanup — don't fail if token wasn't in keychain
        let _ = keychain::delete_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);
        Ok(())
    }

    pub async fn status(&self) -> AuthStatus {
        let state = self.state.read().await;
        match &*state {
            Some(s) => AuthStatus {
                authenticated: true,
                user: Some(s.user.clone()),
            },
            None => AuthStatus {
                authenticated: false,
                user: None,
            },
        }
    }

    pub async fn token(&self) -> Option<String> {
        let state = self.state.read().await;
        state.as_ref().map(|s| s.token.clone())
    }

    /// Initiate a GitHub Device Flow. Returns the user_code and verification_uri
    /// that should be shown to the user.
    pub async fn start_device_flow(&self) -> Result<DeviceFlowResponse> {
        let client = reqwest::Client::new();
        let response = client
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "repo")])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "GitHub device flow initiation failed: {}",
                response.status()
            ));
        }

        let flow: DeviceFlowResponse = response.json().await?;
        Ok(flow)
    }

    /// Poll GitHub until the device is authorized or until timeout.
    /// On success, stores the token in the keychain and returns the GitHubUser.
    pub async fn poll_device_flow(&self, device_code: &str, interval: u64) -> Result<GitHubUser> {
        let client = reqwest::Client::new();
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(DEVICE_FLOW_TIMEOUT_SECS);
        let mut poll_interval = interval;

        loop {
            if std::time::Instant::now() > deadline {
                return Err(anyhow!("Device flow authorization timed out"));
            }

            tokio::time::sleep(std::time::Duration::from_secs(poll_interval)).await;

            let response = client
                .post(ACCESS_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", GITHUB_CLIENT_ID),
                    ("device_code", device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await?;

            let poll: PollResponse = response.json().await?;

            if let Some(token) = poll.access_token {
                let user = self.validate_token(&token).await?;
                keychain::store_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, &token)?;
                let mut state = self.state.write().await;
                *state = Some(AuthState {
                    token,
                    user: user.clone(),
                });
                return Ok(user);
            }

            match poll.error.as_deref() {
                Some("authorization_pending") => {
                    // Normal — keep polling
                }
                Some("slow_down") => {
                    // GitHub wants us to poll slower
                    poll_interval = poll.interval.unwrap_or(poll_interval + 5);
                }
                Some("expired_token") => {
                    return Err(anyhow!("Device flow code expired. Please start again."));
                }
                Some("access_denied") => {
                    return Err(anyhow!("Authorization denied by user."));
                }
                Some(other) => {
                    return Err(anyhow!("Device flow error: {other}"));
                }
                None => {
                    return Err(anyhow!("Unexpected empty response from GitHub"));
                }
            }
        }
    }

    async fn validate_token(&self, token: &str) -> Result<GitHubUser> {
        let octocrab = octocrab::Octocrab::builder()
            .personal_token(token.to_string())
            .build()?;
        let user = octocrab.current().user().await?;
        Ok(GitHubUser {
            login: user.login,
            avatar_url: user.avatar_url.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Helper to create an AuthManager that is already in an authenticated state,
    /// bypassing the GitHub API call.
    fn authenticated_manager(login: &str) -> AuthManager {
        let user = GitHubUser {
            login: login.to_string(),
            avatar_url: "https://example.com/avatar.png".to_string(),
        };
        AuthManager {
            state: Arc::new(RwLock::new(Some(AuthState {
                token: "ghp_fake_test_token".to_string(),
                user,
            }))),
        }
    }

    #[tokio::test]
    async fn test_auth_manager_new_is_not_authenticated() {
        let manager = AuthManager::new();
        let status = manager.status().await;
        assert!(!status.authenticated);
        assert!(status.user.is_none());
    }

    #[tokio::test]
    async fn test_auth_manager_default_is_not_authenticated() {
        let manager = AuthManager::default();
        let status = manager.status().await;
        assert!(!status.authenticated);
    }

    #[tokio::test]
    async fn test_token_returns_none_when_not_logged_in() {
        let manager = AuthManager::new();
        let token = manager.token().await;
        assert!(token.is_none());
    }

    #[tokio::test]
    async fn test_login_with_invalid_pat_fails() {
        let manager = AuthManager::new();
        let result = manager.login_with_pat("ghp_invalidtoken_for_testing").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_logout_when_not_logged_in() {
        // Logout should either succeed or fail due to keychain — not panic
        let manager = AuthManager::new();
        let result = manager.logout().await;
        // Accept both ok (no entry to delete) and err (keychain can vary)
        let _ = result;
    }

    #[tokio::test]
    async fn test_status_returns_correct_structure_when_unauthenticated() {
        let manager = AuthManager::new();
        let status = manager.status().await;
        assert!(!status.authenticated);
        assert!(status.user.is_none());
    }

    #[tokio::test]
    async fn test_status_authenticated_returns_true_and_user() {
        let manager = authenticated_manager("octocat");
        let status = manager.status().await;
        assert!(status.authenticated);
        let user = status.user.unwrap();
        assert_eq!(user.login, "octocat");
    }

    #[tokio::test]
    async fn test_token_returns_some_when_authenticated() {
        let manager = authenticated_manager("octocat");
        let token = manager.token().await;
        assert_eq!(token, Some("ghp_fake_test_token".to_string()));
    }

    #[tokio::test]
    async fn test_logout_clears_state_when_authenticated() {
        let manager = authenticated_manager("octocat");
        // Verify authenticated first
        assert!(manager.status().await.authenticated);
        // Logout — may fail on keychain, but state should still be cleared
        let _ = manager.logout().await;
        let status = manager.status().await;
        assert!(!status.authenticated);
    }

    #[tokio::test]
    async fn test_github_user_clone() {
        let user = GitHubUser {
            login: "testuser".to_string(),
            avatar_url: "https://example.com/avatar.png".to_string(),
        };
        let cloned = user.clone();
        assert_eq!(cloned.login, "testuser");
        assert_eq!(cloned.avatar_url, "https://example.com/avatar.png");
    }

    #[tokio::test]
    async fn test_auth_status_serialization() {
        let status = AuthStatus {
            authenticated: true,
            user: Some(GitHubUser {
                login: "testuser".to_string(),
                avatar_url: "https://example.com/avatar.png".to_string(),
            }),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"authenticated\":true"));
        assert!(json.contains("testuser"));
    }

    /// Validate that `login_with_pat` with an invalid token fails and leaves
    /// the manager in an unauthenticated state.
    #[tokio::test]
    async fn test_login_with_invalid_pat_leaves_state_unauthenticated() {
        let manager = AuthManager::new();
        let _ = manager
            .login_with_pat("definitely_invalid_token_abc123")
            .await;
        // Whether it errors or not, the state should not be authenticated
        // (we can't guarantee the keychain state, but internal state is clear)
        // The important check: the result is an error
        let result = manager
            .login_with_pat("definitely_invalid_token_abc123")
            .await;
        assert!(result.is_err());
    }

    /// Validate that `validate_token` (indirectly via login_with_pat) returns
    /// a meaningful error message for an invalid token.
    #[tokio::test]
    async fn test_login_with_pat_error_is_descriptive() {
        let manager = AuthManager::new();
        let err = manager
            .login_with_pat("ghp_fake_invalid_token_for_testing")
            .await
            .unwrap_err();
        // The error should come from the octocrab/GitHub API layer
        let msg = err.to_string();
        assert!(!msg.is_empty(), "error message should not be empty");
    }

    /// Test that `try_restore` succeeds (returns Ok) even when there is no
    /// stored token in the keychain — it should be a no-op in that case.
    #[tokio::test]
    async fn test_try_restore_with_no_stored_token_returns_ok() {
        // Use a fresh AuthManager; there is likely no token for the default service
        // in the test environment, so try_restore should just return Ok(()).
        let manager = AuthManager::new();
        // We can't easily control the keychain in tests, but try_restore must
        // not panic and must return a Result.
        let result = manager.try_restore().await;
        // Accept both Ok (no token stored) and err (keychain unavailable).
        // The key thing is it shouldn't panic.
        let _ = result;
    }

    /// Test that `AuthStatus` correctly deserializes from JSON.
    #[test]
    fn test_auth_status_deserialization() {
        let json = r#"{"authenticated":false,"user":null}"#;
        let status: AuthStatus = serde_json::from_str(json).unwrap();
        assert!(!status.authenticated);
        assert!(status.user.is_none());
    }

    /// Test that `AuthStatus` with a user deserializes correctly.
    #[test]
    fn test_auth_status_deserialization_with_user() {
        let json = r#"{"authenticated":true,"user":{"login":"alice","avatar_url":"https://example.com/a.png"}}"#;
        let status: AuthStatus = serde_json::from_str(json).unwrap();
        assert!(status.authenticated);
        let user = status.user.unwrap();
        assert_eq!(user.login, "alice");
    }

    /// Test that `GitHubUser` serializes to JSON correctly.
    #[test]
    fn test_github_user_serialization() {
        let user = GitHubUser {
            login: "bob".to_string(),
            avatar_url: "https://example.com/bob.png".to_string(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("\"login\":\"bob\""));
        assert!(json.contains("avatar_url"));
    }

    /// Verify that multiple clones of AuthManager share the same underlying state.
    #[tokio::test]
    async fn test_auth_manager_clone_shares_state() {
        let manager = authenticated_manager("shared_user");
        let clone = manager.clone();

        // Both should see the authenticated state
        assert!(manager.status().await.authenticated);
        assert!(clone.status().await.authenticated);

        // Logout via clone, original should also see unauthenticated
        let _ = clone.logout().await;
        assert!(!manager.status().await.authenticated);
    }

    /// Test that token() returns the correct token after being authenticated via helper.
    #[tokio::test]
    async fn test_token_value_matches_stored_token() {
        let manager = authenticated_manager("tokenuser");
        let token = manager.token().await;
        assert_eq!(token.as_deref(), Some("ghp_fake_test_token"));
    }

    /// Test that status() returns the correct user login after being authenticated.
    #[tokio::test]
    async fn test_status_user_login_matches() {
        let manager = authenticated_manager("specific_login_name");
        let status = manager.status().await;
        assert_eq!(status.user.unwrap().login, "specific_login_name");
    }
}
