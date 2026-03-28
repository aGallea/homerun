use homerun::cli;
use homerun::client::{
    DaemonClient, DiscoveredRepo, MetricsResponse, RunnerConfig, RunnerInfo, SystemMetrics,
};
use test_utils::MockDaemon;

fn make_runner(id: &str, name: &str, state: &str) -> RunnerInfo {
    RunnerInfo {
        config: RunnerConfig {
            id: id.to_string(),
            name: name.to_string(),
            repo_owner: "octocat".to_string(),
            repo_name: "hello-world".to_string(),
            labels: vec!["self-hosted".to_string()],
            mode: "app".to_string(),
            work_dir: std::path::PathBuf::from("/tmp"),
            group_id: None,
        },
        state: state.to_string(),
        pid: None,
        uptime_secs: None,
        jobs_completed: 5,
        jobs_failed: 1,
        current_job: None,
        job_context: None,
        job_started_at: None,
        estimated_job_duration_secs: None,
    }
}

#[tokio::test]
async fn test_cmd_list_with_runners() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "my-runner-1", "online"))
        .with_runner(make_runner("r2", "my-runner-2", "busy"))
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    // Should not error
    cli::cmd_list(&client).await.unwrap();
}

#[tokio::test]
async fn test_cmd_list_empty() {
    let mock = MockDaemon::builder().build().await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_list(&client).await.unwrap();
}

#[tokio::test]
async fn test_cmd_list_with_metrics() {
    let metrics = MetricsResponse {
        system: SystemMetrics {
            cpu_percent: 50.0,
            memory_used_bytes: 1024,
            memory_total_bytes: 2048,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
        },
        runners: vec![homerun::client::RunnerMetrics {
            runner_id: "r1".to_string(),
            cpu_percent: 25.0,
            memory_bytes: 1024 * 1024,
        }],
        daemon: None,
    };
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "runner-1", "online"))
        .with_metrics(metrics)
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_list(&client).await.unwrap();
}

#[tokio::test]
async fn test_cmd_status_authenticated() {
    let metrics = MetricsResponse {
        system: SystemMetrics {
            cpu_percent: 42.0,
            memory_used_bytes: 8 * 1024 * 1024 * 1024,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
        },
        runners: vec![],
        daemon: None,
    };
    let mock = MockDaemon::builder()
        .authenticated_as("octocat")
        .with_runner(make_runner("r1", "runner-1", "online"))
        .with_runner(make_runner("r2", "runner-2", "busy"))
        .with_metrics(metrics)
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_status(&client).await.unwrap();
}

#[tokio::test]
async fn test_cmd_status_unauthenticated() {
    let mock = MockDaemon::builder().build().await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_status(&client).await.unwrap();
}

#[tokio::test]
async fn test_cmd_scan_empty_results() {
    let mock = MockDaemon::builder().build().await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_scan(&client, None, false).await.unwrap();
}

#[tokio::test]
async fn test_cmd_scan_local() {
    let repo = DiscoveredRepo {
        full_name: "octocat/hello-world".to_string(),
        source: "local".to_string(),
        workflow_files: vec![".github/workflows/ci.yml".to_string()],
        local_path: Some(std::path::PathBuf::from("/tmp/hello-world")),
        matched_labels: vec![],
    };
    let mock = MockDaemon::builder()
        .with_scan_local_results(vec![repo])
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_scan(&client, Some("/tmp/workspace".to_string()), false)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_cmd_scan_remote() {
    let repo = DiscoveredRepo {
        full_name: "octocat/remote-repo".to_string(),
        source: "remote".to_string(),
        workflow_files: vec![".github/workflows/deploy.yml".to_string()],
        local_path: None,
        matched_labels: vec![],
    };
    let mock = MockDaemon::builder()
        .with_scan_remote_results(vec![repo])
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_scan(&client, None, true).await.unwrap();
}

#[tokio::test]
async fn test_cmd_scan_local_and_remote() {
    let local_repo = DiscoveredRepo {
        full_name: "octocat/both-repo".to_string(),
        source: "local".to_string(),
        workflow_files: vec![".github/workflows/ci.yml".to_string()],
        local_path: Some(std::path::PathBuf::from("/tmp/both-repo")),
        matched_labels: vec![],
    };
    let remote_repo = DiscoveredRepo {
        full_name: "octocat/both-repo".to_string(),
        source: "remote".to_string(),
        workflow_files: vec![".github/workflows/deploy.yml".to_string()],
        local_path: None,
        matched_labels: vec![],
    };
    let mock = MockDaemon::builder()
        .with_scan_local_results(vec![local_repo])
        .with_scan_remote_results(vec![remote_repo])
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());
    cli::cmd_scan(&client, Some("/tmp/workspace".to_string()), true)
        .await
        .unwrap();
}
