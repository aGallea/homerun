pub mod routes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use homerun::client::{
    AuthStatus, DeviceFlowResponse, DiscoveredRepo, GitHubUser, JobHistoryEntry, MetricsResponse,
    RepoInfo, RunnerInfo, StepsResponse,
};

pub struct MockState {
    pub runners: Vec<RunnerInfo>,
    pub repos: Vec<RepoInfo>,
    pub auth: AuthStatus,
    pub metrics: Option<MetricsResponse>,
    pub job_history: HashMap<String, Vec<JobHistoryEntry>>,
    pub steps: HashMap<String, StepsResponse>,
    pub device_flow_response: Option<DeviceFlowResponse>,
    pub device_flow_authorized: bool,
    pub scan_local_results: Vec<DiscoveredRepo>,
    pub scan_remote_results: Vec<DiscoveredRepo>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            runners: Vec::new(),
            repos: Vec::new(),
            auth: AuthStatus {
                authenticated: false,
                user: None,
            },
            metrics: None,
            job_history: HashMap::new(),
            steps: HashMap::new(),
            device_flow_response: None,
            device_flow_authorized: false,
            scan_local_results: Vec::new(),
            scan_remote_results: Vec::new(),
        }
    }
}

pub type SharedState = Arc<RwLock<MockState>>;

pub struct MockDaemon {
    socket_path: PathBuf,
    _tmp_dir: tempfile::TempDir,
    #[cfg(windows)]
    _tcp_addr: Option<std::net::SocketAddr>,
}

impl MockDaemon {
    pub fn builder() -> MockDaemonBuilder {
        MockDaemonBuilder {
            state: MockState::default(),
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// On Windows, returns the TCP address the mock daemon is listening on.
    #[cfg(windows)]
    pub fn tcp_addr(&self) -> Option<std::net::SocketAddr> {
        self._tcp_addr
    }
}

pub struct MockDaemonBuilder {
    state: MockState,
}

impl MockDaemonBuilder {
    pub fn with_runner(mut self, runner: RunnerInfo) -> Self {
        self.state.runners.push(runner);
        self
    }

    pub fn with_repo(mut self, repo: RepoInfo) -> Self {
        self.state.repos.push(repo);
        self
    }

    pub fn authenticated_as(mut self, login: &str) -> Self {
        self.state.auth = AuthStatus {
            authenticated: true,
            user: Some(GitHubUser {
                login: login.to_string(),
                avatar_url: String::new(),
            }),
        };
        self
    }

    pub fn with_metrics(mut self, metrics: MetricsResponse) -> Self {
        self.state.metrics = Some(metrics);
        self
    }

    pub fn with_job_history(mut self, runner_id: &str, history: Vec<JobHistoryEntry>) -> Self {
        self.state
            .job_history
            .insert(runner_id.to_string(), history);
        self
    }

    pub fn with_steps(mut self, runner_id: &str, steps: StepsResponse) -> Self {
        self.state.steps.insert(runner_id.to_string(), steps);
        self
    }

    pub fn with_device_flow(mut self, response: DeviceFlowResponse) -> Self {
        self.state.device_flow_response = Some(response);
        self
    }

    pub fn with_scan_local_results(mut self, repos: Vec<DiscoveredRepo>) -> Self {
        self.state.scan_local_results = repos;
        self
    }

    pub fn with_scan_remote_results(mut self, repos: Vec<DiscoveredRepo>) -> Self {
        self.state.scan_remote_results = repos;
        self
    }

    pub async fn build(self) -> MockDaemon {
        let tmp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let socket_path = tmp_dir.path().join("mock-daemon.sock");

        let shared_state: SharedState = Arc::new(RwLock::new(self.state));
        let app = routes::create_router(shared_state);

        #[cfg(unix)]
        {
            let listener = tokio::net::UnixListener::bind(&socket_path)
                .expect("Failed to bind mock Unix socket");

            tokio::spawn(async move {
                axum::serve(listener, app).await.ok();
            });
        }

        #[cfg(windows)]
        let tcp_addr = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("Failed to bind mock TCP socket");
            let addr = listener.local_addr().unwrap();
            // Write the port to the socket_path file so clients can discover it
            std::fs::write(&socket_path, addr.to_string())
                .expect("Failed to write mock TCP address");
            tokio::spawn(async move {
                axum::serve(listener, app).await.ok();
            });
            addr
        };

        // Give the server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        MockDaemon {
            socket_path,
            _tmp_dir: tmp_dir,
            #[cfg(windows)]
            _tcp_addr: Some(tcp_addr),
        }
    }
}
