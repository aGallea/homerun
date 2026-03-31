use anyhow::{bail, Context, Result};
use futures::stream::SplitStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use tokio_tungstenite::WebSocketStream;

/// Percent-encode a string for use in a URL query parameter.
fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

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
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContext {
    pub branch: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub run_url: String,
    #[serde(default)]
    pub job_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: String,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
    #[serde(default)]
    pub current_job: Option<String>,
    #[serde(default)]
    pub job_context: Option<JobContext>,
    #[serde(default)]
    pub job_started_at: Option<String>,
    #[serde(default)]
    pub estimated_job_duration_secs: Option<u64>,
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
pub struct DaemonLogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonMetrics {
    pub pid: u32,
    pub uptime_seconds: u64,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub child_processes: Vec<ChildProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildProcessInfo {
    pub pid: u32,
    pub runner_id: String,
    pub runner_name: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub system: SystemMetrics,
    pub runners: Vec<RunnerMetrics>,
    #[serde(default)]
    pub daemon: Option<DaemonMetrics>,
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
pub struct DiscoveredRepo {
    pub full_name: String,
    pub source: String,
    pub workflow_files: Vec<String>,
    pub local_path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub matched_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalScanRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerEvent {
    pub runner_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryEntry {
    pub job_name: String,
    pub started_at: String,
    pub completed_at: String,
    pub succeeded: bool,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    #[serde(default)]
    pub duration_secs: u64,
    #[serde(default)]
    pub job_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateResponse {
    pub group_id: String,
    pub runners: Vec<RunnerInfo>,
    pub errors: Vec<BatchCreateError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateError {
    pub index: u8,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupActionResult {
    pub runner_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupActionResponse {
    pub group_id: String,
    pub results: Vec<GroupActionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleGroupResponse {
    pub group_id: String,
    pub previous_count: u8,
    pub target_count: u8,
    pub actual_count: u8,
    pub added: Vec<RunnerInfo>,
    pub removed: Vec<String>,
    pub skipped_busy: Vec<String>,
}

// --- Platform-aware HTTP connectors ---

/// A tower connector that dials a Unix socket instead of TCP.
#[cfg(unix)]
#[derive(Clone)]
struct UnixConnector {
    socket_path: PathBuf,
}

#[cfg(unix)]
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

/// A tower connector that dials a Windows named pipe instead of TCP.
#[cfg(windows)]
#[derive(Clone)]
struct NamedPipeConnector {
    pipe_name: String,
}

#[cfg(windows)]
impl tower::Service<hyper::Uri> for NamedPipeConnector {
    type Response = hyper_util::rt::TokioIo<tokio::net::windows::named_pipe::NamedPipeClient>;
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
        let pipe_name = self.pipe_name.clone();
        Box::pin(async move {
            let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name)?;
            Ok(hyper_util::rt::TokioIo::new(client))
        })
    }
}

// --- DaemonClient ---

pub struct DaemonClient {
    #[cfg(unix)]
    socket_path: PathBuf,
    #[cfg(windows)]
    pipe_name: String,
    /// TCP address for testing on Windows (mock daemon uses TCP).
    #[cfg(windows)]
    tcp_addr: Option<std::net::SocketAddr>,
}

impl DaemonClient {
    #[cfg(unix)]
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// On Windows, `new()` creates a TCP-based client for test compatibility.
    /// The socket_path is read to get the TCP address written by the mock daemon.
    #[cfg(windows)]
    pub fn new(socket_path: PathBuf) -> Self {
        // The mock daemon on Windows writes the TCP address to the socket_path file.
        let tcp_addr = std::fs::read_to_string(&socket_path)
            .ok()
            .and_then(|s| s.trim().parse().ok());
        Self {
            pipe_name: r"\\.\pipe\homerun-daemon".to_string(),
            tcp_addr,
        }
    }

    #[cfg(windows)]
    pub fn new_pipe(pipe_name: String) -> Self {
        Self {
            pipe_name,
            tcp_addr: None,
        }
    }

    pub fn default_socket() -> Self {
        #[cfg(unix)]
        {
            let home = dirs::home_dir().expect("no home directory");
            Self::new(home.join(".homerun/daemon.sock"))
        }
        #[cfg(windows)]
        {
            Self::new_pipe(r"\\.\pipe\homerun-daemon".to_string())
        }
    }

    /// Check if the daemon socket/pipe exists.
    pub fn socket_exists(&self) -> bool {
        #[cfg(unix)]
        {
            self.socket_path.exists()
        }
        #[cfg(windows)]
        {
            tokio::net::windows::named_pipe::ClientOptions::new()
                .open(&self.pipe_name)
                .is_ok()
        }
    }

    /// Return the socket path (for WebSocket connections).
    #[cfg(unix)]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Send an HTTP request to the daemon and return (status_code, body_text).
    async fn send_request(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<(u16, String)> {
        let uri = format!("http://localhost{path}");
        let mut builder = Request::builder().method(method).uri(&uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder.body(body.unwrap_or_default())?;

        #[cfg(unix)]
        let response = {
            let connector = UnixConnector {
                socket_path: self.socket_path.clone(),
            };
            let client: Client<UnixConnector, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client
                .request(req)
                .await
                .context("Failed to connect to daemon — is homerund running?")?
        };
        #[cfg(windows)]
        let response = if let Some(addr) = self.tcp_addr {
            // Test mode: connect via TCP to mock daemon
            let tcp_uri: hyper::Uri = format!("http://{addr}{path}").parse()?;
            let tcp_req = Request::builder()
                .method(method)
                .uri(tcp_uri)
                .header("content-type", "application/json")
                .body(req.into_body())?;
            let connector = hyper_util::client::legacy::connect::HttpConnector::new();
            let client: Client<_, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client
                .request(tcp_req)
                .await
                .context("Failed to connect to daemon — is homerund running?")?
        } else {
            let connector = NamedPipeConnector {
                pipe_name: self.pipe_name.clone(),
            };
            let client: Client<NamedPipeConnector, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client
                .request(req)
                .await
                .context("Failed to connect to daemon — is homerund running?")?
        };

        let status = response.status().as_u16();
        let collected = http_body_util::BodyExt::collect(response.into_body())
            .await
            .context("Failed to read response body")?;
        let bytes = collected.to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();
        Ok((status, text))
    }

    async fn request(&self, method: &str, path: &str, body: Option<String>) -> Result<String> {
        let (status_code, text) = self.send_request(method, path, body).await?;
        let status = hyper::StatusCode::from_u16(status_code).unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);

        if !status.is_success() && status_code != 204 {
            bail!("Daemon returned {status}: {text}");
        }
        Ok(text)
    }

    async fn raw_request(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<(u16, String)> {
        self.send_request(method, path, body).await
    }

    // --- API methods ---

    pub async fn health(&self) -> Result<()> {
        self.request("GET", "/health", None).await?;
        Ok(())
    }

    /// Returns the number of active runners being stopped during shutdown.
    pub async fn shutdown(&self) -> Result<usize> {
        let body = self.request("POST", "/daemon/shutdown", None).await?;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        Ok(json["active_runners"].as_u64().unwrap_or(0) as usize)
    }

    pub async fn auth_status(&self) -> Result<AuthStatus> {
        let body = self.request("GET", "/auth/status", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn start_device_flow(&self) -> Result<DeviceFlowResponse> {
        let body = self.request("POST", "/auth/device", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn poll_device_flow(
        &self,
        device_code: &str,
        interval: u64,
    ) -> Result<Option<AuthStatus>> {
        let payload = serde_json::json!({
            "device_code": device_code,
            "interval": interval,
        });
        let (status, text) = self
            .raw_request("POST", "/auth/device/poll", Some(payload.to_string()))
            .await?;
        if status == 401 {
            Ok(None)
        } else if (200..300).contains(&status) {
            let auth: AuthStatus = serde_json::from_str(&text)?;
            Ok(Some(auth))
        } else {
            bail!("Device flow poll failed ({status}): {text}")
        }
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

    pub async fn scan_local(&self, path: &str) -> Result<Vec<DiscoveredRepo>> {
        let req = LocalScanRequest {
            path: path.to_string(),
        };
        let body = self
            .request("POST", "/scan/local", Some(serde_json::to_string(&req)?))
            .await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn scan_remote(&self) -> Result<Vec<DiscoveredRepo>> {
        let body = self.request("POST", "/scan/remote", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn create_batch(
        &self,
        repo_full_name: &str,
        count: u8,
        labels: Option<Vec<String>>,
        mode: Option<String>,
    ) -> Result<BatchCreateResponse> {
        let body = serde_json::json!({
            "repo_full_name": repo_full_name,
            "count": count,
            "labels": labels,
            "mode": mode,
        });
        let text = self
            .request("POST", "/runners/batch", Some(body.to_string()))
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn start_group(&self, group_id: &str) -> Result<GroupActionResponse> {
        let text = self
            .request("POST", &format!("/runners/groups/{group_id}/start"), None)
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn stop_group(&self, group_id: &str) -> Result<GroupActionResponse> {
        let text = self
            .request("POST", &format!("/runners/groups/{group_id}/stop"), None)
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn restart_group(&self, group_id: &str) -> Result<GroupActionResponse> {
        let text = self
            .request("POST", &format!("/runners/groups/{group_id}/restart"), None)
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn delete_group(&self, group_id: &str) -> Result<GroupActionResponse> {
        let text = self
            .request("DELETE", &format!("/runners/groups/{group_id}"), None)
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn scale_group(&self, group_id: &str, count: u8) -> Result<ScaleGroupResponse> {
        let body = serde_json::json!({ "count": count });
        let text = self
            .request(
                "PATCH",
                &format!("/runners/groups/{group_id}"),
                Some(body.to_string()),
            )
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn get_daemon_logs_recent(
        &self,
        level: Option<&str>,
        limit: Option<usize>,
        search: Option<&str>,
    ) -> Result<Vec<DaemonLogEntry>> {
        let mut params = Vec::new();
        if let Some(l) = level {
            params.push(format!("level={}", l));
        }
        if let Some(n) = limit {
            params.push(format!("limit={}", n));
        }
        if let Some(s) = search {
            params.push(format!("search={}", url_encode(s)));
        }
        let query = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        let body = self
            .request("GET", &format!("/daemon/logs/recent{}", query), None)
            .await?;
        let entries: Vec<DaemonLogEntry> = serde_json::from_str(&body)?;
        Ok(entries)
    }

    pub async fn get_runner_steps(&self, runner_id: &str) -> Result<StepsResponse> {
        let body = self
            .request("GET", &format!("/runners/{runner_id}/steps"), None)
            .await?;
        serde_json::from_str(&body).context("Failed to parse steps response")
    }

    pub async fn get_job_history(&self, runner_id: &str) -> Result<Vec<JobHistoryEntry>> {
        let body = self
            .request("GET", &format!("/runners/{runner_id}/history"), None)
            .await?;
        Ok(serde_json::from_str(&body)?)
    }

    /// Connect to the daemon's WebSocket endpoint for real-time events.
    /// Returns a stream of incoming WebSocket messages.
    #[cfg(unix)]
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::UnixStream>>> {
        let stream = tokio::net::UnixStream::connect(&self.socket_path).await?;
        let uri = "ws://localhost/events";
        let (ws_stream, _response) = tokio_tungstenite::client_async(uri, stream).await?;
        let (_write, read) = ws_stream.split();
        Ok(read)
    }

    /// Connect to the daemon's WebSocket endpoint for real-time events.
    /// Returns a stream of incoming WebSocket messages.
    #[cfg(windows)]
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::windows::named_pipe::NamedPipeClient>>>
    {
        let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&self.pipe_name)?;
        let uri = "ws://localhost/events";
        let (ws_stream, _response) = tokio_tungstenite::client_async(uri, client).await?;
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
