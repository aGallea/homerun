use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

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
    pub current_job: Option<String>,
    pub job_context: Option<JobContext>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub job_started_at: Option<String>,
    #[serde(default)]
    pub last_completed_job: Option<CompletedJob>,
    #[serde(default)]
    pub estimated_job_duration_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub runner_id: String,
    pub timestamp: String,
    pub line: String,
    pub stream: String,
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
pub struct StepLogsResponse {
    pub step_number: u16,
    pub step_name: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAttempt {
    pub attempt: u32,
    pub succeeded: bool,
    pub runner_name: String,
    pub completed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job_name: String,
    pub succeeded: bool,
    pub completed_at: String,
    pub duration_secs: u64,
    pub branch: Option<String>,
    pub pr_number: Option<u64>,
    pub run_url: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub latest_attempt: Option<RunAttempt>,
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
    pub error_message: Option<String>,
    pub steps: Vec<StepInfo>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub latest_attempt: Option<RunAttempt>,
    #[serde(default)]
    pub job_number: u32,
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
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
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
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchRequest {
    pub repo_full_name: String,
    pub count: u8,
    pub labels: Option<Vec<String>>,
    pub mode: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub start_runners_on_launch: bool,
    pub notify_status_changes: bool,
    pub notify_job_completions: bool,
    #[serde(default)]
    pub scan_labels: Vec<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub auto_scan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRepo {
    pub full_name: String,
    pub source: String,
    pub workflow_files: Vec<String>,
    pub local_path: Option<String>,
    pub matched_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResults {
    pub last_scan_at: String,
    pub local_results: Vec<DiscoveredRepo>,
    pub remote_results: Vec<DiscoveredRepo>,
    pub merged_results: Vec<DiscoveredRepo>,
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
            let client =
                tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name)?;
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
}

impl DaemonClient {
    #[cfg(unix)]
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    #[cfg(windows)]
    pub fn new_pipe(pipe_name: String) -> Self {
        Self { pipe_name }
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

    /// Return the socket path.
    #[cfg(unix)]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Create a new client with the same connection parameters.
    pub fn clone_connection(&self) -> Self {
        #[cfg(unix)]
        {
            Self::new(self.socket_path.clone())
        }
        #[cfg(windows)]
        {
            Self::new_pipe(self.pipe_name.clone())
        }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<String, String> {
        // hyper requires a valid URI — the host is ignored for Unix sockets / named pipes.
        let uri = format!("http://localhost{path}");
        let mut builder = Request::builder().method(method).uri(&uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder
            .body(body.unwrap_or_default())
            .map_err(|e| e.to_string())?;

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
                .map_err(|e| format!("Failed to connect to daemon — is homerund running? {e}"))?
        };
        #[cfg(windows)]
        let response = {
            let connector = NamedPipeConnector {
                pipe_name: self.pipe_name.clone(),
            };
            let client: Client<NamedPipeConnector, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client
                .request(req)
                .await
                .map_err(|e| format!("Failed to connect to daemon — is homerund running? {e}"))?
        };

        let status = response.status();
        let collected = http_body_util::BodyExt::collect(response.into_body())
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;
        let bytes = collected.to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();

        if !status.is_success() && status.as_u16() != 204 {
            return Err(format!("Daemon returned {status}: {text}"));
        }
        Ok(text)
    }

    // --- API methods ---

    pub async fn health(&self) -> Result<(), String> {
        self.request("GET", "/health", None).await?;
        Ok(())
    }

    pub async fn auth_status(&self) -> Result<AuthStatus, String> {
        let body = self.request("GET", "/auth/status", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn login_with_token(&self, token: &str) -> Result<AuthStatus, String> {
        let payload = serde_json::json!({ "token": token }).to_string();
        let body = self.request("POST", "/auth/login", Some(payload)).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn logout(&self) -> Result<(), String> {
        self.request("DELETE", "/auth", None).await?;
        Ok(())
    }

    pub async fn list_runners(&self) -> Result<Vec<RunnerInfo>, String> {
        let body = self.request("GET", "/runners", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn create_runner(&self, req: &CreateRunnerRequest) -> Result<RunnerInfo, String> {
        let body = self
            .request(
                "POST",
                "/runners",
                Some(serde_json::to_string(req).map_err(|e| e.to_string())?),
            )
            .await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn delete_runner(&self, id: &str) -> Result<(), String> {
        self.request("DELETE", &format!("/runners/{id}"), None)
            .await?;
        Ok(())
    }

    pub async fn start_runner(&self, id: &str) -> Result<(), String> {
        self.request("POST", &format!("/runners/{id}/start"), None)
            .await?;
        Ok(())
    }

    pub async fn stop_runner(&self, id: &str) -> Result<(), String> {
        self.request("POST", &format!("/runners/{id}/stop"), None)
            .await?;
        Ok(())
    }

    pub async fn restart_runner(&self, id: &str) -> Result<(), String> {
        self.request("POST", &format!("/runners/{id}/restart"), None)
            .await?;
        Ok(())
    }

    pub async fn start_device_flow(&self) -> Result<DeviceFlowResponse, String> {
        let body = self.request("POST", "/auth/device", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn poll_device_flow(
        &self,
        device_code: &str,
        interval: u64,
    ) -> Result<AuthStatus, String> {
        let payload = serde_json::json!({
            "device_code": device_code,
            "interval": interval,
        })
        .to_string();
        let body = self
            .request("POST", "/auth/device/poll", Some(payload))
            .await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn list_repos(&self) -> Result<Vec<RepoInfo>, String> {
        let body = self.request("GET", "/repos", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn get_metrics(&self) -> Result<MetricsResponse, String> {
        let body = self.request("GET", "/metrics", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn service_status(&self) -> Result<bool, String> {
        let body = self.request("GET", "/service/status", None).await?;
        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        json["installed"]
            .as_bool()
            .ok_or_else(|| "missing 'installed' field in service status response".to_string())
    }

    pub async fn install_service(&self) -> Result<(), String> {
        self.request("POST", "/service/install", None).await?;
        Ok(())
    }

    pub async fn uninstall_service(&self) -> Result<(), String> {
        self.request("POST", "/service/uninstall", None).await?;
        Ok(())
    }

    pub async fn get_runner_steps(&self, runner_id: &str) -> Result<StepsResponse, String> {
        let body = self.request("GET", &format!("/runners/{runner_id}/steps"), None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn get_step_logs(&self, runner_id: &str, step_number: u16) -> Result<StepLogsResponse, String> {
        let body = self.request("GET", &format!("/runners/{runner_id}/steps/{step_number}/logs"), None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn get_runner_history(&self, runner_id: &str) -> Result<Vec<JobHistoryEntry>, String> {
        let body = self.request("GET", &format!("/runners/{runner_id}/history"), None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn rerun_workflow(&self, runner_id: &str, run_url: &str) -> Result<(), String> {
        let payload = serde_json::json!({ "run_url": run_url }).to_string();
        self.request("POST", &format!("/runners/{runner_id}/rerun"), Some(payload)).await?;
        Ok(())
    }

    pub async fn clear_runner_history(&self, runner_id: &str) -> Result<(), String> {
        self.request("DELETE", &format!("/runners/{runner_id}/history"), None).await?;
        Ok(())
    }

    pub async fn delete_history_entry(&self, runner_id: &str, started_at: &str) -> Result<(), String> {
        let payload = serde_json::json!({ "started_at": started_at }).to_string();
        self.request("DELETE", &format!("/runners/{runner_id}/history/entry"), Some(payload)).await?;
        Ok(())
    }

    pub async fn get_runner_logs(&self, runner_id: &str) -> Result<Vec<LogEntry>, String> {
        let body = self
            .request("GET", &format!("/runners/{runner_id}/logs/recent"), None)
            .await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn create_batch(&self, req: &CreateBatchRequest) -> Result<BatchCreateResponse, String> {
        let body = serde_json::to_string(req).map_err(|e| e.to_string())?;
        let text = self.request("POST", "/runners/batch", Some(body)).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn start_group(&self, group_id: &str) -> Result<GroupActionResponse, String> {
        let text = self.request("POST", &format!("/runners/groups/{group_id}/start"), None).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn stop_group(&self, group_id: &str) -> Result<GroupActionResponse, String> {
        let text = self.request("POST", &format!("/runners/groups/{group_id}/stop"), None).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn restart_group(&self, group_id: &str) -> Result<GroupActionResponse, String> {
        let text = self.request("POST", &format!("/runners/groups/{group_id}/restart"), None).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn delete_group(&self, group_id: &str) -> Result<GroupActionResponse, String> {
        let text = self.request("DELETE", &format!("/runners/groups/{group_id}"), None).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn scale_group(&self, group_id: &str, count: u8) -> Result<ScaleGroupResponse, String> {
        let body = serde_json::json!({ "count": count }).to_string();
        let text = self.request("PATCH", &format!("/runners/groups/{group_id}"), Some(body)).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn get_preferences(&self) -> Result<Preferences, String> {
        let body = self.request("GET", "/preferences", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn update_preferences(&self, prefs: &Preferences) -> Result<Preferences, String> {
        let body = serde_json::to_string(prefs).map_err(|e| e.to_string())?;
        let text = self.request("PUT", "/preferences", Some(body)).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn scan_local(&self, path: &str) -> Result<Vec<DiscoveredRepo>, String> {
        let body = serde_json::json!({ "path": path }).to_string();
        let text = self.request("POST", "/scan/local", Some(body)).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn scan_remote(&self) -> Result<Vec<DiscoveredRepo>, String> {
        let text = self.request("POST", "/scan/remote", None).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    /// Returns the number of active runners being stopped during shutdown.
    pub async fn shutdown(&self) -> Result<usize, String> {
        let body = self.request("POST", "/daemon/shutdown", None).await?;
        let json: serde_json::Value =
            serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
        Ok(json["active_runners"].as_u64().unwrap_or(0) as usize)
    }

    pub async fn get_scan_results(&self) -> Result<Option<ScanResults>, String> {
        let body = self.request("GET", "/scan/results", None).await?;
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn cancel_scan(&self, scan_id: &str) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({ "scan_id": scan_id }).to_string();
        let text = self.request("POST", "/scan/cancel", Some(body)).await?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    pub async fn get_daemon_logs_recent(
        &self,
        level: Option<&str>,
        limit: Option<usize>,
        search: Option<&str>,
    ) -> Result<Vec<DaemonLogEntry>, String> {
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
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }
}
