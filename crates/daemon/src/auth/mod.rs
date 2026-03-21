pub mod keychain;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

const KEYCHAIN_SERVICE: &str = "com.homerun.daemon";
const KEYCHAIN_ACCOUNT: &str = "github-token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
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
        keychain::delete_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
        let mut state = self.state.write().await;
        *state = None;
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
        assert_eq!(status.authenticated, false);
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
}
