use std::path::PathBuf;

use homerun::client::{
    CreateRunnerRequest, DeviceFlowResponse, JobHistoryEntry, MetricsResponse, RepoInfo,
    RunnerConfig, RunnerInfo, RunnerMetrics, StepInfo, StepsResponse, SystemMetrics,
};
use test_utils::MockDaemon;

fn make_runner(id: &str, state: &str) -> RunnerInfo {
    RunnerInfo {
        config: RunnerConfig {
            id: id.to_string(),
            name: format!("runner-{id}"),
            repo_owner: "octocat".to_string(),
            repo_name: "hello-world".to_string(),
            labels: vec!["self-hosted".to_string()],
            mode: "app".to_string(),
            work_dir: PathBuf::from("/tmp/runners"),
            group_id: None,
        },
        state: state.to_string(),
        pid: None,
        uptime_secs: None,
        jobs_completed: 0,
        jobs_failed: 0,
        current_job: None,
        job_context: None,
        job_started_at: None,
        estimated_job_duration_secs: None,
        last_completed_job: None,
    }
}

fn make_repo(name: &str) -> RepoInfo {
    RepoInfo {
        id: 1,
        full_name: format!("octocat/{name}"),
        name: name.to_string(),
        owner: "octocat".to_string(),
        private: false,
        html_url: format!("https://github.com/octocat/{name}"),
        is_org: false,
    }
}

#[tokio::test]
async fn test_health_check() {
    let mock = MockDaemon::builder().build().await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    client.health().await.expect("health check should succeed");
}

#[tokio::test]
async fn test_auth_status_unauthenticated() {
    let mock = MockDaemon::builder().build().await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let status = client
        .auth_status()
        .await
        .expect("auth_status should succeed");
    assert!(!status.authenticated);
    assert!(status.user.is_none());
}

#[tokio::test]
async fn test_auth_status_authenticated() {
    let mock = MockDaemon::builder()
        .authenticated_as("octocat")
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let status = client
        .auth_status()
        .await
        .expect("auth_status should succeed");
    assert!(status.authenticated);
    let user = status.user.expect("user should be present");
    assert_eq!(user.login, "octocat");
}

#[tokio::test]
async fn test_list_runners() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "online"))
        .with_runner(make_runner("r2", "offline"))
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let runners = client
        .list_runners()
        .await
        .expect("list_runners should succeed");
    assert_eq!(runners.len(), 2);
    assert_eq!(runners[0].config.id, "r1");
    assert_eq!(runners[0].state, "online");
    assert_eq!(runners[1].config.id, "r2");
    assert_eq!(runners[1].state, "offline");
}

#[tokio::test]
async fn test_create_runner() {
    let mock = MockDaemon::builder().build().await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let req = CreateRunnerRequest {
        repo_full_name: "octocat/hello-world".to_string(),
        name: Some("my-runner".to_string()),
        labels: Some(vec!["self-hosted".to_string(), "linux".to_string()]),
        mode: None,
    };
    let runner = client
        .create_runner(&req)
        .await
        .expect("create_runner should succeed");
    assert_eq!(runner.config.repo_owner, "octocat");
    assert_eq!(runner.config.repo_name, "hello-world");
    assert_eq!(runner.config.name, "my-runner");

    // Verify it appears in list
    let runners = client.list_runners().await.expect("list should succeed");
    assert_eq!(runners.len(), 1);
    assert_eq!(runners[0].config.name, "my-runner");
}

#[tokio::test]
async fn test_delete_runner() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("del-1", "offline"))
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    client
        .delete_runner("del-1")
        .await
        .expect("delete should succeed");

    let runners = client.list_runners().await.expect("list should succeed");
    assert!(runners.is_empty());
}

#[tokio::test]
async fn test_start_stop_runner() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("ss-1", "offline"))
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    // Start runner
    client
        .start_runner("ss-1")
        .await
        .expect("start should succeed");
    let runners = client.list_runners().await.unwrap();
    assert_eq!(runners[0].state, "online");

    // Stop runner
    client
        .stop_runner("ss-1")
        .await
        .expect("stop should succeed");
    let runners = client.list_runners().await.unwrap();
    assert_eq!(runners[0].state, "offline");
}

#[tokio::test]
async fn test_restart_runner() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("rs-1", "offline"))
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    client
        .restart_runner("rs-1")
        .await
        .expect("restart should succeed");
    let runners = client.list_runners().await.unwrap();
    assert_eq!(runners[0].state, "online");
}

#[tokio::test]
async fn test_list_repos() {
    let mock = MockDaemon::builder()
        .with_repo(make_repo("repo-a"))
        .with_repo(make_repo("repo-b"))
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let repos = client
        .list_repos()
        .await
        .expect("list_repos should succeed");
    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].full_name, "octocat/repo-a");
    assert_eq!(repos[1].full_name, "octocat/repo-b");
}

#[tokio::test]
async fn test_get_metrics() {
    let metrics = MetricsResponse {
        system: SystemMetrics {
            cpu_percent: 25.0,
            memory_used_bytes: 4_000_000_000,
            memory_total_bytes: 16_000_000_000,
            disk_used_bytes: 100_000_000_000,
            disk_total_bytes: 500_000_000_000,
        },
        runners: vec![RunnerMetrics {
            runner_id: "m-1".to_string(),
            cpu_percent: 10.0,
            memory_bytes: 200_000_000,
        }],
        daemon: None,
    };
    let mock = MockDaemon::builder().with_metrics(metrics).build().await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let m = client
        .get_metrics()
        .await
        .expect("get_metrics should succeed");
    assert!((m.system.cpu_percent - 25.0).abs() < f64::EPSILON);
    assert_eq!(m.runners.len(), 1);
    assert_eq!(m.runners[0].runner_id, "m-1");
}

#[tokio::test]
async fn test_job_history() {
    let history = vec![JobHistoryEntry {
        job_name: "build".to_string(),
        started_at: "2026-03-27T10:00:00Z".to_string(),
        completed_at: "2026-03-27T10:05:00Z".to_string(),
        succeeded: true,
        branch: Some("main".to_string()),
        pr_number: Some(42),
        run_url: Some("https://github.com/octocat/hello-world/actions/runs/1".to_string()),
        duration_secs: 300,
        job_number: 1,
    }];
    let mock = MockDaemon::builder()
        .with_job_history("h-1", history)
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let h = client
        .get_job_history("h-1")
        .await
        .expect("get_job_history should succeed");
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].job_name, "build");
    assert!(h[0].succeeded);
    assert_eq!(h[0].pr_number, Some(42));
}

#[tokio::test]
async fn test_runner_steps() {
    let steps = StepsResponse {
        job_name: "CI".to_string(),
        steps: vec![
            StepInfo {
                number: 1,
                name: "Checkout".to_string(),
                status: "completed".to_string(),
                started_at: Some("2026-03-27T10:00:00Z".to_string()),
                completed_at: Some("2026-03-27T10:00:05Z".to_string()),
            },
            StepInfo {
                number: 2,
                name: "Build".to_string(),
                status: "in_progress".to_string(),
                started_at: Some("2026-03-27T10:00:05Z".to_string()),
                completed_at: None,
            },
        ],
        steps_discovered: 2,
    };
    let mock = MockDaemon::builder()
        .with_steps("st-1", steps)
        .build()
        .await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let s = client
        .get_runner_steps("st-1")
        .await
        .expect("get_runner_steps should succeed");
    assert_eq!(s.job_name, "CI");
    assert_eq!(s.steps.len(), 2);
    assert_eq!(s.steps[0].name, "Checkout");
    assert_eq!(s.steps[0].status, "completed");
    assert_eq!(s.steps[1].name, "Build");
    assert_eq!(s.steps[1].status, "in_progress");
}

#[tokio::test]
async fn test_device_flow() {
    let flow = DeviceFlowResponse {
        device_code: "abc123".to_string(),
        user_code: "ABCD-1234".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
        expires_in: 900,
        interval: 5,
    };
    let mock = MockDaemon::builder().with_device_flow(flow).build().await;
    let client = homerun::client::DaemonClient::new(mock.socket_path().clone());

    let resp = client
        .start_device_flow()
        .await
        .expect("start_device_flow should succeed");
    assert_eq!(resp.user_code, "ABCD-1234");
    assert_eq!(resp.device_code, "abc123");

    // Poll should return None (not yet authorized)
    let poll = client
        .poll_device_flow(&resp.device_code, resp.interval)
        .await
        .expect("poll should not error");
    assert!(
        poll.is_none(),
        "poll should return None when not yet authorized"
    );
}
