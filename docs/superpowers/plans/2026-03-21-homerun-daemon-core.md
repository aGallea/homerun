# HomeRun Daemon Core — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the HomeRun daemon (`homerund`) — a Rust service that manages GitHub Actions self-hosted runners via an Axum API over a Unix socket.

**Architecture:** A Rust workspace with a `daemon` crate. The daemon exposes a REST/SSE/WebSocket API over a Unix socket. It manages runner processes (download, register, start, stop, delete), authenticates via GitHub PAT (OAuth deferred to Plan 5), streams logs via SSE, and collects metrics via the `sysinfo` crate.

**Deferred to later plans:**

- OAuth flow (`POST /auth/github`) — Plan 5
- `GET /repos/:id/workflows`, `GET /repos/:id/runners` — Plan 2 (TUI) or Plan 5
- `GET /metrics/history` — Plan 5 (notifications & polish)

**Tech Stack:** Rust, Tokio, Axum (Unix socket), octocrab (GitHub API), sysinfo, security-framework (macOS Keychain), serde/toml (config)

**Spec:** `docs/superpowers/specs/2026-03-21-self-runner-design.md`

**Implementation notes:**

- Verify `sysinfo` v0.33 API at implementation time — `System::new_all()` may have been replaced with `System::new()` + specific refresh calls. Check docs.
- Verify `security-framework` v3 error type API — `.code()` method availability may differ.
- For Unix socket serving with Axum 0.8, check latest docs. May need `tokio-listener`, `hyperlocal`, or direct `hyper-util` wiring.

---

## File Structure

```
homerun/
├── Cargo.toml                          # Workspace root
├── Cargo.lock
├── .gitignore
├── crates/
│   └── daemon/
│       ├── Cargo.toml                  # Daemon crate dependencies
│       └── src/
│           ├── main.rs                 # Entry point: parse args, start server
│           ├── lib.rs                  # Re-exports for testing
│           ├── config.rs              # Config loading/saving (~/.homerun/config.toml)
│           ├── server.rs              # Axum router + Unix socket listener
│           ├── auth/
│           │   ├── mod.rs             # Auth module: PAT validation, keychain storage
│           │   └── keychain.rs        # macOS Keychain read/write via security-framework
│           ├── github/
│           │   ├── mod.rs             # GitHub API client wrapper
│           │   └── types.rs           # GitHub API response types
│           ├── runner/
│           │   ├── mod.rs             # Runner manager: orchestrates lifecycle
│           │   ├── state.rs           # Runner state machine (Creating, Online, Busy, etc.)
│           │   ├── process.rs         # Process spawning and monitoring
│           │   ├── binary.rs          # Runner binary download and caching
│           │   └── types.rs           # Runner config, metadata types
│           ├── api/
│           │   ├── mod.rs             # Route registration
│           │   ├── auth.rs            # POST /auth/token, GET /auth/status, DELETE /auth
│           │   ├── repos.rs           # GET /repos
│           │   ├── runners.rs         # CRUD endpoints for runners
│           │   ├── logs.rs            # GET /runners/:id/logs (SSE)
│           │   ├── metrics.rs         # GET /metrics, GET /metrics/history
│           │   └── events.rs          # WS /events (WebSocket)
│           └── metrics.rs             # Metrics collector (sysinfo, ring buffer)
└── tests/
    └── integration/
        ├── mod.rs
        ├── helpers.rs                 # Test helpers: start daemon, create client
        ├── health_test.rs             # Health endpoint test
        ├── auth_test.rs               # Auth flow tests
        └── runner_test.rs             # Runner lifecycle tests
```

---

### Task 1: Project Scaffold

**Files:**

- Create: `Cargo.toml` (workspace root)
- Create: `crates/daemon/Cargo.toml`
- Create: `crates/daemon/src/main.rs`
- Create: `crates/daemon/src/lib.rs`
- Create: `.gitignore`

- [ ] **Step 1: Initialize git repo**

```bash
cd /Users/asaf/workspace/self-runner
git init
```

- [ ] **Step 2: Create .gitignore**

```gitignore
/target
.DS_Store
.superpowers/
*.swp
*.swo
```

- [ ] **Step 3: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/daemon"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/aGallea/homerun"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 4: Create daemon crate Cargo.toml**

```toml
[package]
name = "homerund"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "homerund"
path = "src/main.rs"

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
axum = "0.8"
hyper = { version = "1", features = ["full"] }
hyper-util = { version = "0.1", features = ["tokio"] }
tower = "0.5"
toml = "0.8"
octocrab = "0.44"
sysinfo = "0.33"
security-framework = "3"
dirs = "6"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tokio-stream = "0.1"
futures = "0.3"
reqwest = { version = "0.12", features = ["json"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Create minimal main.rs**

```rust
use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("HomeRun daemon starting...");
    Ok(())
}
```

- [ ] **Step 6: Create lib.rs**

```rust
pub mod config;
pub mod server;
```

- [ ] **Step 7: Create empty module files**

Create empty `config.rs` and `server.rs` with just a comment placeholder so it compiles.

- [ ] **Step 8: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: initial project scaffold with Rust workspace"
```

---

### Task 2: Config Module

**Files:**

- Create: `crates/daemon/src/config.rs`
- Test: inline `#[cfg(test)]` module

- [ ] **Step 1: Write failing test for config defaults**

In `crates/daemon/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.socket_path(), dirs::home_dir().unwrap().join(".homerun/daemon.sock"));
        assert_eq!(config.runners_dir(), dirs::home_dir().unwrap().join(".homerun/runners"));
        assert_eq!(config.cache_dir(), dirs::home_dir().unwrap().join(".homerun/cache"));
        assert_eq!(config.log_dir(), dirs::home_dir().unwrap().join(".homerun/logs"));
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(config, loaded);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund config::tests`
Expected: FAIL — `Config` not defined

- [ ] **Step 3: Implement Config**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    base_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        Self {
            base_dir: home.join(".homerun"),
        }
    }
}

impl Config {
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn socket_path(&self) -> PathBuf {
        self.base_dir.join("daemon.sock")
    }

    pub fn runners_dir(&self) -> PathBuf {
        self.base_dir.join("runners")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn config_path(&self) -> PathBuf {
        self.base_dir.join("config.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)?;
        std::fs::create_dir_all(self.runners_dir())?;
        std::fs::create_dir_all(self.cache_dir())?;
        std::fs::create_dir_all(self.log_dir())?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p homerund config::tests`
Expected: 2 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/config.rs crates/daemon/src/lib.rs
git commit -m "feat: add config module with defaults and serialization"
```

---

### Task 3: Axum Server on Unix Socket

**Files:**

- Create: `crates/daemon/src/server.rs`
- Create: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/main.rs`

- [ ] **Step 1: Write failing test for health endpoint**

In `crates/daemon/src/server.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund server::tests`
Expected: FAIL

- [ ] **Step 3: Implement server with shared AppState**

The `AppState` holds shared state (config, runner manager, auth state). The server creates an Axum router and binds to a Unix socket.

```rust
use anyhow::Result;
use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;
use tokio::net::UnixListener;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        Self::new(Config::with_base_dir(
            tempfile::tempdir().unwrap().into_path().join(".homerun"),
        ))
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn serve(config: Config) -> Result<()> {
    let state = AppState::new(config.clone());
    let app = create_router(state);

    let socket_path = config.socket_path();
    // Remove stale socket file
    let _ = std::fs::remove_file(&socket_path);
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    tracing::info!("Listening on {}", socket_path.display());

    // Axum 0.8 supports UnixListener directly via the tokio feature
    axum::serve(
        tokio_listener::Listener::from(listener),
        app.into_make_service(),
    )
    .await?;
    Ok(())
}
```

**Note:** Add `tokio-listener = "0.4"` to daemon dependencies. This crate provides the `Listener` trait implementation that Axum needs for Unix sockets. Alternatively, use `hyperlocal` or wire `hyper-util` manually — verify the latest Axum 0.8 docs for Unix socket support at implementation time.

````

- [ ] **Step 4: Update main.rs to call serve**

Note: `main.rs` imports from the library crate (`homerund::`) — it does NOT use `mod` declarations. All modules are declared in `lib.rs` so both the binary and integration tests share the same module tree.

```rust
use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = homerund::config::Config::default();
    config.ensure_dirs()?;

    tracing::info!("HomeRun daemon starting...");
    homerund::server::serve(config).await
}
````

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p homerund server::tests`
Expected: PASS

- [ ] **Step 6: Manual smoke test — start daemon, hit health endpoint**

In one terminal:

```bash
cargo run -p homerund
```

In another terminal:

```bash
curl --unix-socket ~/.homerun/daemon.sock http://localhost/health
```

Expected: `{"status":"ok","version":"0.1.0"}`

Kill the daemon with Ctrl+C.

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/
git commit -m "feat: add Axum server on Unix socket with health endpoint"
```

---

### Task 4: Auth Module (PAT)

**Files:**

- Create: `crates/daemon/src/auth/mod.rs`
- Create: `crates/daemon/src/auth/keychain.rs`
- Create: `crates/daemon/src/api/auth.rs`
- Modify: `crates/daemon/src/server.rs` (add auth routes, add auth state to AppState)
- Modify: `crates/daemon/src/api/mod.rs`

- [ ] **Step 1: Write failing test for keychain module**

In `crates/daemon/src/auth/keychain.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Note: these tests interact with the real macOS Keychain.
    // They use a unique test service name to avoid conflicts.

    #[test]
    fn test_store_and_retrieve_token() {
        let service = "com.homerun.test.keychain";
        let account = "github-token";
        let token = "ghp_test_token_12345";

        store_token(service, account, token).unwrap();
        let retrieved = get_token(service, account).unwrap();
        assert_eq!(retrieved, Some(token.to_string()));

        delete_token(service, account).unwrap();
        let deleted = get_token(service, account).unwrap();
        assert_eq!(deleted, None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund auth::keychain::tests`
Expected: FAIL

- [ ] **Step 3: Implement keychain module**

```rust
use anyhow::Result;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

pub fn store_token(service: &str, account: &str, token: &str) -> Result<()> {
    // Delete existing first to avoid "duplicate item" error
    let _ = delete_generic_password(service, account);
    set_generic_password(service, account, token.as_bytes())?;
    Ok(())
}

pub fn get_token(service: &str, account: &str) -> Result<Option<String>> {
    match get_generic_password(service, account) {
        Ok(bytes) => Ok(Some(String::from_utf8(bytes)?)),
        Err(e) if e.code() == -25300 => Ok(None), // errSecItemNotFound
        Err(e) => Err(e.into()),
    }
}

pub fn delete_token(service: &str, account: &str) -> Result<()> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()), // already gone
        Err(e) => Err(e.into()),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p homerund auth::keychain::tests`
Expected: PASS (may prompt for Keychain access on first run)

- [ ] **Step 5: Implement auth module with AuthManager**

In `crates/daemon/src/auth/mod.rs`:

```rust
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

#[derive(Clone)]
pub struct AuthManager {
    state: Arc<RwLock<Option<AuthState>>>,
}

struct AuthState {
    token: String,
    user: GitHubUser,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }

    /// Try to load token from keychain and validate it on startup
    pub async fn try_restore(&self) -> Result<()> {
        if let Some(token) = keychain::get_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)? {
            match self.validate_token(&token).await {
                Ok(user) => {
                    *self.state.write().await = Some(AuthState { token, user });
                }
                Err(_) => {
                    // Token expired/revoked, clean up
                    keychain::delete_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
                }
            }
        }
        Ok(())
    }

    pub async fn login_with_pat(&self, token: &str) -> Result<GitHubUser> {
        let user = self.validate_token(token).await?;
        keychain::store_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, token)?;
        *self.state.write().await = Some(AuthState {
            token: token.to_string(),
            user: user.clone(),
        });
        Ok(user)
    }

    pub async fn logout(&self) -> Result<()> {
        keychain::delete_token(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
        *self.state.write().await = None;
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
        self.state.read().await.as_ref().map(|s| s.token.clone())
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
```

- [ ] **Step 6: Add auth API endpoints**

In `crates/daemon/src/api/auth.rs`:

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use crate::server::AppState;
use crate::auth::AuthStatus;

#[derive(Deserialize)]
pub struct TokenRequest {
    pub token: String,
}

pub async fn login_with_token(
    State(state): State<AppState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<AuthStatus>, (StatusCode, String)> {
    state
        .auth
        .login_with_pat(&body.token)
        .await
        .map(|user| {
            Json(AuthStatus {
                authenticated: true,
                user: Some(user),
            })
        })
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))
}

pub async fn logout(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .auth
        .logout()
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn status(
    State(state): State<AppState>,
) -> Json<AuthStatus> {
    Json(state.auth.status().await)
}
```

- [ ] **Step 7: Wire auth into AppState and router**

Update `server.rs` to add `AuthManager` to `AppState` and register auth routes:

```rust
// In AppState:
pub auth: AuthManager,

// In create_router:
.route("/auth/token", post(api::auth::login_with_token))
.route("/auth", delete(api::auth::logout))
.route("/auth/status", get(api::auth::status))
```

- [ ] **Step 8: Run all tests**

Run: `cargo test -p homerund`
Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/daemon/src/auth/ crates/daemon/src/api/ crates/daemon/src/server.rs crates/daemon/src/lib.rs
git commit -m "feat: add auth module with PAT login and macOS Keychain storage"
```

---

### Task 5: GitHub API Client

**Files:**

- Create: `crates/daemon/src/github/mod.rs`
- Create: `crates/daemon/src/github/types.rs`
- Create: `crates/daemon/src/api/repos.rs`
- Modify: `crates/daemon/src/server.rs` (add repos routes)

- [ ] **Step 1: Define GitHub types**

In `crates/daemon/src/github/types.rs`:

```rust
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
```

- [ ] **Step 2: Write failing test for GitHub client**

In `crates/daemon/src/github/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_client_requires_token() {
        let result = GitHubClient::new(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_github_client_with_token() {
        let client = GitHubClient::new(Some("ghp_test".to_string()));
        assert!(client.is_ok());
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p homerund github::tests`
Expected: FAIL

- [ ] **Step 4: Implement GitHub client**

In `crates/daemon/src/github/mod.rs`:

```rust
pub mod types;

use anyhow::{bail, Result};
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
        let mut page = 1u32;
        let mut repos = Vec::new();
        loop {
            let result = self
                .octocrab
                .current()
                .list_repos_for_authenticated_user()
                .per_page(100)
                .page(page)
                .send()
                .await?;

            if result.items.is_empty() {
                break;
            }

            for repo in &result.items {
                let owner = repo
                    .owner
                    .as_ref()
                    .map(|o| o.login.clone())
                    .unwrap_or_default();
                repos.push(RepoInfo {
                    id: repo.id.into_inner(),
                    full_name: repo.full_name.clone().unwrap_or_default(),
                    name: repo.name.clone(),
                    owner: owner.clone(),
                    private: repo.private.unwrap_or(false),
                    html_url: repo
                        .html_url
                        .as_ref()
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                    is_org: repo
                        .owner
                        .as_ref()
                        .map(|o| o.r#type == "Organization")
                        .unwrap_or(false),
                });
            }

            if result.next.is_none() {
                break;
            }
            page += 1;
        }
        Ok(repos)
    }

    pub async fn get_runner_registration_token(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<RunnerRegistration> {
        let response: serde_json::Value = self
            .octocrab
            .post(
                format!("/repos/{owner}/{repo}/actions/runners/registration-token"),
                None::<&()>,
            )
            .await?;
        Ok(RunnerRegistration {
            token: response["token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing token in response"))?
                .to_string(),
            expires_at: response["expires_at"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        })
    }
}
```

- [ ] **Step 5: Add repos API endpoint**

In `crates/daemon/src/api/repos.rs`:

```rust
use axum::{extract::State, http::StatusCode, Json};
use crate::github::GitHubClient;
use crate::github::types::RepoInfo;
use crate::server::AppState;

pub async fn list_repos(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepoInfo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client = GitHubClient::new(token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let repos = client
        .list_repos()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}
```

- [ ] **Step 6: Wire repos route into router**

```rust
.route("/repos", get(api::repos::list_repos))
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add crates/daemon/src/github/ crates/daemon/src/api/repos.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs crates/daemon/src/lib.rs
git commit -m "feat: add GitHub API client with repo listing and runner registration"
```

---

### Task 6: Runner Binary Downloader

**Files:**

- Create: `crates/daemon/src/runner/binary.rs`
- Create: `crates/daemon/src/runner/mod.rs`

- [ ] **Step 1: Write failing test for binary download URL construction**

In `crates/daemon/src/runner/binary.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_url_macos_arm64() {
        let url = runner_download_url("2.321.0", "osx", "arm64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-osx-arm64-2.321.0.tar.gz"
        );
    }

    #[test]
    fn test_detect_platform() {
        let (os, arch) = detect_platform();
        assert_eq!(os, "osx");
        // arch is either "arm64" or "x64"
        assert!(arch == "arm64" || arch == "x64");
    }

    #[tokio::test]
    async fn test_get_latest_version() {
        // This hits the real GitHub API (no auth needed for public releases)
        let version = get_latest_runner_version().await.unwrap();
        assert!(version.starts_with("2."));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund runner::binary::tests`
Expected: FAIL

- [ ] **Step 3: Implement binary module**

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn runner_download_url(version: &str, os: &str, arch: &str) -> String {
    format!(
        "https://github.com/actions/runner/releases/download/v{version}/actions-runner-{os}-{arch}-{version}.tar.gz"
    )
}

pub fn detect_platform() -> (&'static str, &'static str) {
    let os = "osx";
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    (os, arch)
}

pub async fn get_latest_runner_version() -> Result<String> {
    let octocrab = octocrab::Octocrab::builder().build()?;
    let release = octocrab
        .repos("actions", "runner")
        .releases()
        .get_latest()
        .await?;
    let version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    Ok(version.to_string())
}

/// Downloads and extracts the runner binary to cache_dir, returns the path
pub async fn ensure_runner_binary(cache_dir: &Path) -> Result<PathBuf> {
    let version = get_latest_runner_version().await?;
    let (os, arch) = detect_platform();

    let versioned_dir = cache_dir.join(format!("runner-{version}"));
    let run_sh = versioned_dir.join("run.sh");

    if run_sh.exists() {
        tracing::info!("Runner binary v{version} already cached");
        return Ok(versioned_dir);
    }

    let url = runner_download_url(&version, os, arch);
    tracing::info!("Downloading runner binary v{version} from {url}");

    std::fs::create_dir_all(&versioned_dir)?;

    let response = reqwest::get(&url).await?;
    let bytes = response.bytes().await?;

    let tar_path = cache_dir.join(format!("runner-{version}.tar.gz"));
    std::fs::write(&tar_path, &bytes)?;

    // Extract tar.gz
    let status = tokio::process::Command::new("tar")
        .args(["xzf", tar_path.to_str().unwrap(), "-C", versioned_dir.to_str().unwrap()])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("Failed to extract runner binary");
    }

    // Clean up tar
    let _ = std::fs::remove_file(&tar_path);

    tracing::info!("Runner binary v{version} cached at {}", versioned_dir.display());
    Ok(versioned_dir)
}
```

- [ ] **Step 4: Add reqwest to daemon dependencies (if not already present)**

Ensure `reqwest` is in `crates/daemon/Cargo.toml` dependencies:

```toml
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 5: Run unit tests (skip the download test in CI)**

Run: `cargo test -p homerund runner::binary::tests::test_download_url_macos_arm64`
Run: `cargo test -p homerund runner::binary::tests::test_detect_platform`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/ crates/daemon/Cargo.toml
git commit -m "feat: add runner binary download and caching"
```

---

### Task 7: Runner State Machine & Types

**Files:**

- Create: `crates/daemon/src/runner/state.rs`
- Create: `crates/daemon/src/runner/types.rs`

- [ ] **Step 1: Write failing test for state transitions**

In `crates/daemon/src/runner/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(RunnerState::Creating.can_transition_to(&RunnerState::Registering));
        assert!(RunnerState::Registering.can_transition_to(&RunnerState::Online));
        assert!(RunnerState::Online.can_transition_to(&RunnerState::Busy));
        assert!(RunnerState::Busy.can_transition_to(&RunnerState::Online));
        assert!(RunnerState::Online.can_transition_to(&RunnerState::Offline));
        assert!(RunnerState::Busy.can_transition_to(&RunnerState::Stopping));
        assert!(RunnerState::Stopping.can_transition_to(&RunnerState::Offline));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!RunnerState::Offline.can_transition_to(&RunnerState::Busy));
        assert!(!RunnerState::Creating.can_transition_to(&RunnerState::Busy));
    }

    #[test]
    fn test_any_state_can_error() {
        for state in [
            RunnerState::Creating,
            RunnerState::Registering,
            RunnerState::Online,
            RunnerState::Busy,
        ] {
            assert!(state.can_transition_to(&RunnerState::Error));
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerund runner::state::tests`
Expected: FAIL

- [ ] **Step 3: Implement state machine**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunnerState {
    Creating,
    Registering,
    Online,
    Busy,
    Stopping,
    Offline,
    Error,
    Deleting,
}

impl RunnerState {
    pub fn can_transition_to(&self, next: &RunnerState) -> bool {
        use RunnerState::*;
        matches!(
            (self, next),
            // Happy path
            (Creating, Registering)
                | (Registering, Online)
                | (Online, Busy)
                | (Busy, Online)
                | (Online, Offline)
                // Stopping from busy (graceful)
                | (Busy, Stopping)
                | (Stopping, Offline)
                // Starting from offline
                | (Offline, Registering)
                | (Offline, Online)
                // Deleting from any stable state
                | (Offline, Deleting)
                | (Online, Deleting)
                | (Error, Deleting)
                // Error from any active state
                | (Creating, Error)
                | (Registering, Error)
                | (Online, Error)
                | (Busy, Error)
                | (Stopping, Error)
                // Recovery from error
                | (Error, Registering)
                | (Error, Offline)
        )
    }
}
```

- [ ] **Step 4: Define runner types**

In `crates/daemon/src/runner/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::runner::state::RunnerState;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: RunnerState,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p homerund runner::state::tests`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/runner/
git commit -m "feat: add runner state machine and types"
```

---

### Task 8: Runner Manager

**Files:**

- Create: `crates/daemon/src/runner/process.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add RunnerManager)
- Modify: `crates/daemon/src/server.rs` (add RunnerManager to AppState)

- [ ] **Step 1: Write failing test for RunnerManager create**

In `crates/daemon/src/runner/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund runner::tests`
Expected: FAIL

- [ ] **Step 3: Implement RunnerManager**

In `crates/daemon/src/runner/mod.rs`:

```rust
pub mod binary;
pub mod process;
pub mod state;
pub mod types;

use anyhow::{bail, Result};
use state::RunnerState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use types::{CreateRunnerRequest, RunnerConfig, RunnerInfo, RunnerMode};
use crate::config::Config;

#[derive(Clone)]
pub struct RunnerManager {
    config: Arc<Config>,
    runners: Arc<RwLock<HashMap<String, RunnerInfo>>>,
}

impl RunnerManager {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            runners: Arc::new(RwLock::new(HashMap::new())),
        }
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund runner::tests`
Expected: All pass

- [ ] **Step 5: Add RunnerManager to AppState**

Update `server.rs`:

```rust
pub runner_manager: RunnerManager,
```

And initialize it in `AppState::new`:

```rust
runner_manager: RunnerManager::new(config.clone()),
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/runner/ crates/daemon/src/server.rs
git commit -m "feat: add runner manager with create, list, get, delete"
```

---

### Task 9: Runner API Endpoints

**Files:**

- Create: `crates/daemon/src/api/runners.rs`
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/server.rs` (add runner routes)

- [ ] **Step 1: Write failing test for runners API**

In `crates/daemon/src/api/runners.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use crate::server::{create_router, AppState};

    #[tokio::test]
    async fn test_create_and_list_runners() {
        let state = AppState::new_test();

        // Create a runner
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"aGallea/gifted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // List runners (recreate router — Axum Router is consumed by oneshot)
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(runners.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_runner() {
        let state = AppState::new_test();

        // Create
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"aGallea/gifted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = runner["config"]["id"].as_str().unwrap();

        // Delete
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/runners/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Verify gone
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(runners.len(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund api::runners::tests`
Expected: FAIL

- [ ] **Step 3: Implement runner API endpoints**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::runner::types::{CreateRunnerRequest, RunnerInfo};
use crate::server::AppState;

pub async fn create_runner(
    State(state): State<AppState>,
    Json(body): Json<CreateRunnerRequest>,
) -> Result<(StatusCode, Json<RunnerInfo>), (StatusCode, String)> {
    let runner = state
        .runner_manager
        .create(&body.repo_full_name, body.name, body.labels, body.mode)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn list_runners(
    State(state): State<AppState>,
) -> Json<Vec<RunnerInfo>> {
    Json(state.runner_manager.list().await)
}

pub async fn get_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunnerInfo>, StatusCode> {
    state
        .runner_manager
        .get(&id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn delete_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .runner_manager
        .delete(&id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
```

- [ ] **Step 4: Add start/stop/restart/update endpoints**

```rust
pub async fn start_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.runner_manager.update_state(&id, RunnerState::Online).await
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

pub async fn stop_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.runner_manager.update_state(&id, RunnerState::Offline).await
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

pub async fn restart_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Stop then start — actual process restart is handled by RunnerManager
    state.runner_manager.update_state(&id, RunnerState::Offline).await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    state.runner_manager.update_state(&id, RunnerState::Online).await
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

pub async fn update_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRunnerRequest>,
) -> Result<Json<RunnerInfo>, (StatusCode, String)> {
    state.runner_manager.update(&id, body).await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}
```

Add `UpdateRunnerRequest` to `runner/types.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct UpdateRunnerRequest {
    pub labels: Option<Vec<String>>,
    pub mode: Option<RunnerMode>,
}
```

- [ ] **Step 5: Wire all runner routes into router**

```rust
.route("/runners", get(api::runners::list_runners).post(api::runners::create_runner))
.route("/runners/{id}", get(api::runners::get_runner).patch(api::runners::update_runner).delete(api::runners::delete_runner))
.route("/runners/{id}/start", post(api::runners::start_runner))
.route("/runners/{id}/stop", post(api::runners::stop_runner))
.route("/runners/{id}/restart", post(api::runners::restart_runner))
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/api/runners.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add runner CRUD API endpoints"
```

---

### Task 10: Runner Process Management

**Files:**

- Create: `crates/daemon/src/runner/process.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add start/stop methods to RunnerManager)

This task implements the actual runner lifecycle: downloading the binary, running `config.sh` to register with GitHub, spawning `run.sh`, and handling stop/delete with graceful shutdown.

- [ ] **Step 1: Implement process module**

In `crates/daemon/src/runner/process.rs`:

```rust
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

/// Run config.sh to register the runner with GitHub
pub async fn configure_runner(
    runner_dir: &Path,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
) -> Result<()> {
    let labels_str = labels.join(",");
    let status = Command::new(runner_dir.join("config.sh"))
        .args([
            "--url", url,
            "--token", token,
            "--name", name,
            "--labels", &labels_str,
            "--unattended",
            "--replace",
        ])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("config.sh failed with exit code: {:?}", status.code());
    }
    Ok(())
}

/// Spawn run.sh and return the child process handle
pub async fn start_runner(runner_dir: &Path) -> Result<Child> {
    let child = Command::new(runner_dir.join("run.sh"))
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    Ok(child)
}

/// Remove runner registration from GitHub
pub async fn remove_runner(runner_dir: &Path, token: &str) -> Result<()> {
    let status = Command::new(runner_dir.join("config.sh"))
        .args(["remove", "--token", token])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await?;

    if !status.success() {
        tracing::warn!("config.sh remove failed — runner may need manual cleanup on GitHub");
    }
    Ok(())
}
```

- [ ] **Step 2: Add full lifecycle methods to RunnerManager**

Add `register_and_start` and `stop` methods to `RunnerManager` in `mod.rs`. These methods:

- `register_and_start(id, github_token)`: downloads binary (if needed), copies to runner dir, runs config.sh, spawns run.sh, updates state through Creating → Registering → Online
- `stop(id)`: sends SIGTERM to the process, waits for completion, transitions to Offline

Store `Child` handles in a separate `Arc<RwLock<HashMap<String, Child>>>` field on RunnerManager.

- [ ] **Step 3: Write test for state transitions during lifecycle**

```rust
#[tokio::test]
async fn test_runner_state_transitions() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.path().join(".homerun"));
    config.ensure_dirs().unwrap();
    let manager = RunnerManager::new(config);

    let runner = manager.create("aGallea/gifted", None, None, None).await.unwrap();
    assert_eq!(runner.state, RunnerState::Creating);

    // Simulate state transitions
    manager.update_state(&runner.config.id, RunnerState::Registering).await.unwrap();
    manager.update_state(&runner.config.id, RunnerState::Online).await.unwrap();

    let updated = manager.get(&runner.config.id).await.unwrap();
    assert_eq!(updated.state, RunnerState::Online);

    // Invalid transition should fail
    let result = manager.update_state(&runner.config.id, RunnerState::Creating).await;
    assert!(result.is_err());
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/runner/
git commit -m "feat: add runner process management (configure, start, stop)"
```

---

### Task 11: SSE Log Streaming

**Files:**

- Create: `crates/daemon/src/api/logs.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add log broadcasting)
- Modify: `crates/daemon/src/api/mod.rs`
- Modify: `crates/daemon/src/server.rs` (add log route)

- [ ] **Step 1: Implement log broadcaster in RunnerManager**

Add a `tokio::sync::broadcast` channel to RunnerManager. When a runner process is spawned, a background task reads its stdout/stderr line by line and sends each line to the broadcast channel tagged with the runner ID.

```rust
// In RunnerManager:
log_tx: Arc<broadcast::Sender<LogEntry>>,

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub runner_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub line: String,
    pub stream: String, // "stdout" or "stderr"
}
```

- [ ] **Step 2: Implement SSE endpoint**

In `crates/daemon/src/api/logs.rs`:

```rust
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use crate::server::AppState;

pub async fn stream_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.runner_manager.subscribe_logs();
    let stream = BroadcastStream::new(rx)
        .filter_map(move |entry| {
            match entry {
                Ok(log) if log.runner_id == id => {
                    Some(Ok(Event::default().json_data(&log).unwrap()))
                }
                _ => None,
            }
        });
    Sse::new(stream)
}
```

- [ ] **Step 3: Wire log route**

```rust
.route("/runners/{id}/logs", get(api::logs::stream_logs))
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/api/logs.rs crates/daemon/src/runner/ crates/daemon/src/api/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add SSE log streaming for runners"
```

---

### Task 12: Metrics Collection

**Files:**

- Create: `crates/daemon/src/metrics.rs`
- Create: `crates/daemon/src/api/metrics.rs`
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Write failing test for metrics collector**

In `crates/daemon/src/metrics.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_capacity() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1.0);
        buffer.push(2.0);
        buffer.push(3.0);
        buffer.push(4.0); // overwrites 1.0

        let values: Vec<f64> = buffer.iter().collect();
        assert_eq!(values, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_runner_metrics_snapshot() {
        let collector = MetricsCollector::new();
        // Just verify it doesn't panic — actual system metrics vary
        let system_metrics = collector.system_snapshot();
        assert!(system_metrics.cpu_percent >= 0.0);
        assert!(system_metrics.memory_total_bytes > 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund metrics::tests`
Expected: FAIL

- [ ] **Step 3: Implement metrics collector**

```rust
use serde::Serialize;
use sysinfo::System;
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerMetrics {
    pub runner_id: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

pub struct RingBuffer<T> {
    data: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.data.len() == self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(item);
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.data.iter().cloned()
    }
}

pub struct MetricsCollector {
    system: std::sync::Mutex<System>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            system: std::sync::Mutex::new(System::new_all()),
        }
    }

    pub fn system_snapshot(&self) -> SystemMetrics {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_all();

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (disk_total, disk_used) = disks.list().iter().fold((0u64, 0u64), |(t, u), d| {
            (t + d.total_space(), u + (d.total_space() - d.available_space()))
        });

        SystemMetrics {
            cpu_percent: sys.global_cpu_usage() as f64,
            memory_used_bytes: sys.used_memory(),
            memory_total_bytes: sys.total_memory(),
            disk_used_bytes: disk_used,
            disk_total_bytes: disk_total,
        }
    }

    pub fn runner_metrics(&self, pid: u32) -> Option<RunnerMetrics> {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let pid = sysinfo::Pid::from_u32(pid);
        sys.process(pid).map(|p| RunnerMetrics {
            runner_id: String::new(), // filled by caller
            cpu_percent: p.cpu_usage() as f64,
            memory_bytes: p.memory(),
        })
    }
}
```

- [ ] **Step 4: Add metrics API endpoint**

In `crates/daemon/src/api/metrics.rs`:

```rust
use axum::{extract::State, Json};
use crate::server::AppState;

pub async fn get_metrics(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let system = state.metrics.system_snapshot();
    let runners = state.runner_manager.list().await;

    let runner_metrics: Vec<_> = runners
        .iter()
        .filter_map(|r| {
            r.pid.and_then(|pid| {
                state.metrics.runner_metrics(pid).map(|mut m| {
                    m.runner_id = r.config.id.clone();
                    m
                })
            })
        })
        .collect();

    Json(serde_json::json!({
        "system": system,
        "runners": runner_metrics,
    }))
}
```

- [ ] **Step 5: Wire metrics into AppState and router**

Add `MetricsCollector` to `AppState` and route:

```rust
.route("/metrics", get(api::metrics::get_metrics))
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/metrics.rs crates/daemon/src/api/metrics.rs crates/daemon/src/api/mod.rs crates/daemon/src/server.rs crates/daemon/src/lib.rs
git commit -m "feat: add metrics collection with system and per-runner CPU/RAM/disk"
```

---

### Task 13: WebSocket Events

**Files:**

- Create: `crates/daemon/src/api/events.rs`
- Modify: `crates/daemon/src/runner/mod.rs` (add event broadcasting)
- Modify: `crates/daemon/src/server.rs`

- [ ] **Step 1: Add event types and broadcast channel to RunnerManager**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct RunnerEvent {
    pub runner_id: String,
    pub event_type: String, // "state_changed", "job_started", "job_completed"
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

Add a `broadcast::Sender<RunnerEvent>` to RunnerManager. Emit events on state changes.

- [ ] **Step 2: Implement WebSocket handler**

In `crates/daemon/src/api/events.rs`:

```rust
use axum::{
    extract::{State, ws::{Message, WebSocket, WebSocketUpgrade}},
    response::Response,
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use crate::server::AppState;

pub async fn events_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let rx = state.runner_manager.subscribe_events();
    let mut stream = BroadcastStream::new(rx);

    while let Some(Ok(event)) = stream.next().await {
        let json = serde_json::to_string(&event).unwrap();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
```

- [ ] **Step 3: Wire WebSocket route**

```rust
.route("/events", get(api::events::events_ws))
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p homerund`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/src/api/events.rs crates/daemon/src/runner/ crates/daemon/src/api/mod.rs crates/daemon/src/server.rs
git commit -m "feat: add WebSocket event streaming for real-time runner updates"
```

---

### Task 14: Integration Test — Full Flow

**Files:**

- Create: `crates/daemon/tests/helpers.rs`
- Create: `crates/daemon/tests/health_test.rs`
- Create: `crates/daemon/tests/runner_test.rs`

- [ ] **Step 1: Create test helper that starts daemon on a temp Unix socket**

```rust
// crates/daemon/tests/helpers.rs
use homerund::config::Config;
use homerund::server::{create_router, AppState};
use tempfile::TempDir;

pub struct TestDaemon {
    pub dir: TempDir,
    pub app: axum::Router,
}

impl TestDaemon {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.ensure_dirs().unwrap();
        let state = AppState::new(config);
        let app = create_router(state);
        Self { dir, app }
    }
}
```

- [ ] **Step 2: Write integration test for health**

```rust
#[tokio::test]
async fn test_health() {
    let daemon = TestDaemon::new();
    let response = daemon.app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 3: Write integration test for runner CRUD**

Test the full flow: create runner → list → get → delete → verify gone.

- [ ] **Step 4: Run integration tests**

Run: `cargo test -p homerund --test '*'`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/daemon/tests/
git commit -m "test: add integration tests for health and runner CRUD"
```

---

### Task 15: Final Cleanup & Documentation

**Files:**

- Modify: `crates/daemon/src/lib.rs` (clean up re-exports)
- Modify: `Cargo.toml` (verify all deps are correct)

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Fix any warnings.

- [ ] **Step 2: Run rustfmt**

Run: `cargo fmt --check`
Fix any formatting issues.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: clippy fixes and formatting cleanup"
```
