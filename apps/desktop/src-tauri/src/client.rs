use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub runner_id: String,
    pub timestamp: String,
    pub line: String,
    pub stream: String,
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

    /// Return the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<String, String> {
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
        let req = builder
            .body(body.unwrap_or_default())
            .map_err(|e| e.to_string())?;

        let response = client
            .request(req)
            .await
            .map_err(|e| format!("Failed to connect to daemon — is homerund running? {e}"))?;

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
        self.request("POST", "/auth/logout", None).await?;
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
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }

    pub async fn install_service(&self) -> Result<(), String> {
        self.request("POST", "/service/install", None).await?;
        Ok(())
    }

    pub async fn uninstall_service(&self) -> Result<(), String> {
        self.request("POST", "/service/uninstall", None).await?;
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
}
