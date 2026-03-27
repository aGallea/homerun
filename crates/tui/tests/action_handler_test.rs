use homerun::app::{App, LoginState};
use homerun::client::DaemonClient;
use test_utils::MockDaemon;

fn make_runner(id: &str, state: &str) -> homerun::client::RunnerInfo {
    homerun::client::RunnerInfo {
        config: homerun::client::RunnerConfig {
            id: id.to_string(),
            name: format!("runner-{id}"),
            repo_owner: "test".to_string(),
            repo_name: "repo".to_string(),
            labels: vec!["self-hosted".to_string()],
            mode: "app".to_string(),
            work_dir: std::path::PathBuf::from("/tmp"),
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
    }
}

// Test the tick-like behavior: fetching runners, steps, history, metrics
#[tokio::test]
async fn test_tick_refreshes_runner_list() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "online"))
        .with_runner(make_runner("r2", "busy"))
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());

    let mut app = App::new();
    // Simulate tick: fetch runners
    if let Ok(runners) = client.list_runners().await {
        app.runners = runners;
        app.rebuild_display_items();
    }
    assert_eq!(app.runners.len(), 2);
    assert_eq!(app.display_items.len(), 2);
}

#[tokio::test]
async fn test_tick_fetches_job_history() {
    let entry = homerun::client::JobHistoryEntry {
        job_name: "build".to_string(),
        started_at: "2026-03-27T10:00:00Z".to_string(),
        completed_at: "2026-03-27T10:05:00Z".to_string(),
        succeeded: true,
        branch: Some("main".to_string()),
        pr_number: None,
        run_url: None,
        job_number: 1,
        duration_secs: 300,
    };
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "online"))
        .with_job_history("r1", vec![entry])
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());

    let mut app = App::new();
    if let Ok(runners) = client.list_runners().await {
        app.runners = runners;
        app.rebuild_display_items();
    }
    // Simulate tick: fetch history for selected runner
    if let Some(runner) = app.selected_runner() {
        let rid = runner.config.id.clone();
        if let Ok(history) = client.get_job_history(&rid).await {
            app.selected_runner_history = history;
        }
    }
    assert_eq!(app.selected_runner_history.len(), 1);
}

#[tokio::test]
async fn test_tick_fetches_metrics() {
    let metrics = homerun::client::MetricsResponse {
        system: homerun::client::SystemMetrics {
            cpu_percent: 50.0,
            memory_used_bytes: 1024,
            memory_total_bytes: 2048,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
        },
        runners: vec![],
        daemon: None,
    };
    let mock = MockDaemon::builder().with_metrics(metrics).build().await;
    let client = DaemonClient::new(mock.socket_path().clone());

    let mut app = App::new();
    if let Ok(m) = client.get_metrics().await {
        app.metrics = Some(m);
    }
    assert!(app.metrics.is_some());
    assert!((app.metrics.unwrap().system.cpu_percent - 50.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_login_flow_start_sets_polling() {
    let flow = homerun::client::DeviceFlowResponse {
        device_code: "dc-test".to_string(),
        user_code: "TEST-1234".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
        expires_in: 900,
        interval: 5,
    };
    let mock = MockDaemon::builder().with_device_flow(flow).build().await;
    let client = DaemonClient::new(mock.socket_path().clone());

    let mut app = App::new();
    // Simulate StartLogin action
    match client.start_device_flow().await {
        Ok(flow) => {
            app.login_state = Some(LoginState::Polling {
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                interval: flow.interval,
            });
        }
        Err(e) => {
            app.login_state = Some(LoginState::Error {
                message: e.to_string(),
            });
        }
    }
    assert!(matches!(app.login_state, Some(LoginState::Polling { .. })));
    if let Some(LoginState::Polling { user_code, .. }) = &app.login_state {
        assert_eq!(user_code, "TEST-1234");
    }
}

#[tokio::test]
async fn test_login_poll_pending() {
    let flow = homerun::client::DeviceFlowResponse {
        device_code: "dc-test".to_string(),
        user_code: "TEST-1234".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
        expires_in: 900,
        interval: 5,
    };
    let mock = MockDaemon::builder().with_device_flow(flow).build().await;
    let client = DaemonClient::new(mock.socket_path().clone());

    // Poll — not yet authorized
    let result = client.poll_device_flow("dc-test", 5).await.unwrap();
    assert!(result.is_none(), "should return None when not authorized");
}

#[tokio::test]
async fn test_login_success_dismisses_on_next_tick() {
    let mut app = App::new();
    app.login_state = Some(LoginState::Success {
        username: "octocat".to_string(),
    });

    // Simulate tick auto-dismiss
    if let Some(LoginState::Success { ref username }) = app.login_state {
        app.status_message = Some(format!("Logged in as {username}"));
        app.login_state = None;
    }

    assert!(app.login_state.is_none());
    assert_eq!(app.status_message, Some("Logged in as octocat".to_string()));
}

#[tokio::test]
async fn test_auto_login_when_unauthenticated() {
    let flow = homerun::client::DeviceFlowResponse {
        device_code: "dc-auto".to_string(),
        user_code: "AUTO-CODE".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
        expires_in: 900,
        interval: 5,
    };
    let mock = MockDaemon::builder().with_device_flow(flow).build().await;
    let client = DaemonClient::new(mock.socket_path().clone());

    let mut app = App::new();
    app.daemon_connected = true;
    // auth_status is None by default (unauthenticated)

    // Simulate auto-login logic from main.rs
    let auto_login =
        !app.auth_status.as_ref().is_some_and(|a| a.authenticated) && app.daemon_connected;
    assert!(auto_login);

    if auto_login {
        if let Ok(flow) = client.start_device_flow().await {
            app.login_state = Some(LoginState::Polling {
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                interval: flow.interval,
            });
        }
    }
    assert!(matches!(app.login_state, Some(LoginState::Polling { .. })));
}

#[tokio::test]
async fn test_start_stop_runner_via_client() {
    let mock = MockDaemon::builder()
        .with_runner(make_runner("r1", "offline"))
        .build()
        .await;
    let client = DaemonClient::new(mock.socket_path().clone());

    // Start
    client.start_runner("r1").await.unwrap();
    let runners = client.list_runners().await.unwrap();
    assert_eq!(runners[0].state, "online");

    // Refresh app state like handle_action does
    let mut app = App::new();
    app.runners = runners;
    app.rebuild_display_items();
    assert_eq!(app.runners[0].state, "online");
}
