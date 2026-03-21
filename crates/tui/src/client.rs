use anyhow::{bail, Context, Result};
use futures::stream::SplitStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio_tungstenite::WebSocketStream;

use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

// --- Response types (mirror daemon's API responses) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub id: String,
    pub name: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub labels: Vec<String>,
    pub mode: String,
    pub work_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: String,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerMetrics {
    pub runner_id: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub system: SystemMetrics,
    pub runners: Vec<RunnerMetrics>,
}

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
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerEvent {
    pub runner_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

// --- Unix socket HTTP connector ---

/// A tower connector that dials a Unix socket instead of TCP.
#[derive(Clone)]
struct UnixConnector {
    socket_path: PathBuf,
}

impl tower::Service<hyper::Uri> for UnixConnector {
    type Response = hyper_util::rt::TokioIo<tokio::net::UnixStream>;
    type Error = std::io::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: hyper::Uri) -> Self::Future {
        let path = self.socket_path.clone();
        Box::pin(async move {
            let stream = tokio::net::UnixStream::connect(path).await?;
            Ok(hyper_util::rt::TokioIo::new(stream))
        })
    }
}

// --- DaemonClient ---

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn default_socket() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        Self::new(home.join(".homerun/daemon.sock"))
    }

    /// Check if the daemon socket exists.
    pub fn socket_exists(&self) -> bool {
        self.socket_path.exists()
    }

    /// Return the socket path (for WebSocket connections).
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn request(&self, method: &str, path: &str, body: Option<String>) -> Result<String> {
        let connector = UnixConnector {
            socket_path: self.socket_path.clone(),
        };
        let client: Client<UnixConnector, String> =
            Client::builder(TokioExecutor::new()).build(connector);

        // hyper requires a valid URI — the host is ignored for Unix sockets.
        let uri = format!("http://localhost{path}");
        let mut builder = Request::builder().method(method).uri(&uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder.body(body.unwrap_or_default())?;

        let response = client
            .request(req)
            .await
            .context("Failed to connect to daemon — is homerund running?")?;

        let status = response.status();
        let collected = http_body_util::BodyExt::collect(response.into_body())
            .await
            .context("Failed to read response body")?;
        let bytes = collected.to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();

        if !status.is_success() && status.as_u16() != 204 {
            bail!("Daemon returned {status}: {text}");
        }
        Ok(text)
    }

    // --- API methods ---

    pub async fn health(&self) -> Result<()> {
        self.request("GET", "/health", None).await?;
        Ok(())
    }

    pub async fn auth_status(&self) -> Result<AuthStatus> {
        let body = self.request("GET", "/auth/status", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn list_runners(&self) -> Result<Vec<RunnerInfo>> {
        let body = self.request("GET", "/runners", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_runner(&self, id: &str) -> Result<RunnerInfo> {
        let body = self.request("GET", &format!("/runners/{id}"), None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn create_runner(&self, req: &CreateRunnerRequest) -> Result<RunnerInfo> {
        let body = self
            .request("POST", "/runners", Some(serde_json::to_string(req)?))
            .await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn delete_runner(&self, id: &str) -> Result<()> {
        self.request("DELETE", &format!("/runners/{id}"), None)
            .await?;
        Ok(())
    }

    pub async fn start_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/start"), None)
            .await?;
        Ok(())
    }

    pub async fn stop_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/stop"), None)
            .await?;
        Ok(())
    }

    pub async fn restart_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/restart"), None)
            .await?;
        Ok(())
    }

    pub async fn list_repos(&self) -> Result<Vec<RepoInfo>> {
        let body = self.request("GET", "/repos", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_metrics(&self) -> Result<MetricsResponse> {
        let body = self.request("GET", "/metrics", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    /// Connect to the daemon's WebSocket endpoint for real-time events.
    /// Returns a stream of incoming WebSocket messages.
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::UnixStream>>> {
        let stream = tokio::net::UnixStream::connect(&self.socket_path).await?;
        let uri = "ws://localhost/events";
        let (ws_stream, _response) =
            tokio_tungstenite::client_async(uri, stream).await?;
        let (_write, read) = ws_stream.split();
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_runners_response() {
        let json = r#"[
            {
                "config": {
                    "id": "abc-123",
                    "name": "gifted-runner-1",
                    "repo_owner": "aGallea",
                    "repo_name": "gifted",
                    "labels": ["self-hosted", "macOS"],
                    "mode": "app",
                    "work_dir": "/tmp/runners/abc-123"
                },
                "state": "online",
                "pid": null,
                "uptime_secs": null,
                "jobs_completed": 0,
                "jobs_failed": 0
            }
        ]"#;
        let runners: Vec<RunnerInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].config.name, "gifted-runner-1");
        assert_eq!(runners[0].state, "online");
    }

    #[tokio::test]
    async fn test_parse_auth_status() {
        let json = r#"{"authenticated": false, "user": null}"#;
        let status: AuthStatus = serde_json::from_str(json).unwrap();
        assert!(!status.authenticated);
        assert!(status.user.is_none());
    }

    #[tokio::test]
    async fn test_parse_metrics_response() {
        let json = r#"{
            "system": {
                "cpu_percent": 12.5,
                "memory_used_bytes": 8000000000,
                "memory_total_bytes": 16000000000,
                "disk_used_bytes": 100000000000,
                "disk_total_bytes": 500000000000
            },
            "runners": []
        }"#;
        let metrics: MetricsResponse = serde_json::from_str(json).unwrap();
        assert!((metrics.system.cpu_percent - 12.5).abs() < f64::EPSILON);
    }
}
