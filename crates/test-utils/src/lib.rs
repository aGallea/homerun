pub mod routes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use homerun::client::{
    AuthStatus, DeviceFlowResponse, GitHubUser, JobHistoryEntry, MetricsResponse, RepoInfo,
    RunnerInfo, StepsResponse,
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
        }
    }
}

pub type SharedState = Arc<RwLock<MockState>>;

pub struct MockDaemon {
    socket_path: PathBuf,
    _tmp_dir: tempfile::TempDir,
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

    pub async fn build(self) -> MockDaemon {
        let tmp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let socket_path = tmp_dir.path().join("mock-daemon.sock");

        let shared_state: SharedState = Arc::new(RwLock::new(self.state));
        let app = routes::create_router(shared_state);

        let listener =
            tokio::net::UnixListener::bind(&socket_path).expect("Failed to bind mock Unix socket");

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        // Give the server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        MockDaemon {
            socket_path,
            _tmp_dir: tmp_dir,
        }
    }
}
