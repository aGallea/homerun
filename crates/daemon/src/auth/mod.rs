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
