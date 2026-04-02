use std::sync::Arc;

use crate::api::{service as api_service, updates as api_updates};
use anyhow::Result;
use axum::{
    routing::{delete, get, patch, post},
    Json, Router,
};
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::RwLock;

use crate::api;
use crate::auth::AuthManager;
use crate::config::Config;
use crate::logging::DaemonLogState;
use crate::metrics::MetricsCollector;
use crate::notifications::NotificationManager;
use crate::runner::RunnerManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub auth: AuthManager,
    pub runner_manager: RunnerManager,
    pub metrics: Arc<MetricsCollector>,
    pub notifications: Arc<NotificationManager>,
    pub daemon_logs: DaemonLogState,
    pub scan_state: crate::api::scanner::ScanState,
    pub daemon_start_time: std::time::Instant,
    pub daemon_pid: u32,
}

impl AppState {
    pub fn new(config: Config, daemon_logs: DaemonLogState) -> Self {
        let notifications = Arc::new(NotificationManager::with_preferences(
            config.preferences.notify_status_changes,
            config.preferences.notify_job_completions,
        ));
        let auth = AuthManager::new();
        let mut runner_manager = RunnerManager::new(config.clone());
        runner_manager.set_auth_manager(auth.clone());
        Self {
            config: Arc::new(RwLock::new(config)),
            auth,
            runner_manager,
            metrics: Arc::new(MetricsCollector::new()),
            notifications,
            daemon_logs,
            scan_state: crate::api::scanner::ScanState::new(),
            daemon_start_time: std::time::Instant::now(),
            daemon_pid: std::process::id(),
        }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        let config = Config::with_base_dir(tempfile::tempdir().unwrap().keep().join(".homerun"));
        config.ensure_dirs().unwrap();
        let daemon_logs = DaemonLogState::new(&config.log_dir());
        Self::new(config, daemon_logs)
    }

    /// Create a test AppState with a pre-authenticated AuthManager.
    #[cfg(test)]
    pub fn new_test_authenticated() -> Self {
        let mut state = Self::new_test();
        state.auth = AuthManager::new_test_authenticated();
        state
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/token", post(api::auth::login_with_token))
        .route("/auth", delete(api::auth::logout))
        .route("/auth/status", get(api::auth::status))
        .route("/auth/device", post(api::auth::start_device_flow))
        .route("/auth/device/poll", post(api::auth::poll_device_flow))
        .route("/repos", get(api::repos::list_repos))
        .route(
            "/runners",
            get(api::runners::list_runners).post(api::runners::create_runner),
        )
        .route(
            "/runners/{id}",
            get(api::runners::get_runner)
                .patch(api::runners::update_runner)
                .delete(api::runners::delete_runner),
        )
        .route("/runners/{id}/start", post(api::runners::start_runner))
        .route("/runners/{id}/stop", post(api::runners::stop_runner))
        .route("/runners/{id}/restart", post(api::runners::restart_runner))
        .route("/runners/batch", post(api::groups::create_batch))
        .route(
            "/runners/groups/{group_id}/start",
            post(api::groups::start_group),
        )
        .route(
            "/runners/groups/{group_id}/stop",
            post(api::groups::stop_group),
        )
        .route(
            "/runners/groups/{group_id}/restart",
            post(api::groups::restart_group),
        )
        .route(
            "/runners/groups/{group_id}",
            patch(api::groups::scale_group).delete(api::groups::delete_group),
        )
        .route(
            "/runners/{id}/history",
            get(api::history::get_runner_history).delete(api::history::clear_runner_history),
        )
        .route(
            "/runners/{id}/history/entry",
            delete(api::history::delete_history_entry),
        )
        .route("/runners/{id}/rerun", post(api::history::rerun_workflow))
        .route(
            "/runners/{id}/run-status",
            post(api::history::get_run_status),
        )
        .route("/runners/{id}/logs", get(api::logs::stream_logs))
        .route("/runners/{id}/logs/recent", get(api::logs::recent_logs))
        .route("/runners/{id}/steps", get(api::steps::get_steps))
        .route(
            "/runners/{id}/steps/{step_number}/logs",
            get(api::steps::get_step_logs),
        )
        .route("/daemon/logs", get(api::daemon_logs::stream_daemon_logs))
        .route(
            "/daemon/logs/recent",
            get(api::daemon_logs::recent_daemon_logs),
        )
        .route("/events", get(api::events::events_ws))
        .route("/metrics", get(api::metrics::get_metrics))
        .route("/scan/local", post(api::scanner::scan_local_handler))
        .route("/scan/remote", post(api::scanner::scan_remote_handler))
        .route("/scan/local/stream", post(api::scanner::scan_local_stream))
        .route(
            "/scan/remote/stream",
            post(api::scanner::scan_remote_stream),
        )
        .route("/scan/cancel", post(api::scanner::cancel_scan))
        .route("/scan/results", get(api::scanner::get_scan_results))
        .route("/service/install", post(api_service::install_service))
        .route("/service/uninstall", post(api_service::uninstall_service))
        .route("/service/status", get(api_service::service_status))
        .route("/updates/check", get(api_updates::check_updates))
        .route(
            "/preferences",
            get(api::preferences::get_preferences).put(api::preferences::update_preferences),
        )
        .route("/daemon/shutdown", post(api::shutdown::shutdown_daemon))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
    }))
}

pub async fn serve(config: Config, daemon_logs: DaemonLogState) -> Result<()> {
    // --- Platform-specific: extract IPC address & check for running daemon ---

    #[cfg(unix)]
    let socket_path = config.socket_path();

    #[cfg(unix)]
    {
        if socket_path.exists() {
            match tokio::net::UnixStream::connect(&socket_path).await {
                Ok(_) => {
                    anyhow::bail!(
                        "Daemon already running (socket {} is active). Stop the existing daemon first.",
                        socket_path.display()
                    );
                }
                Err(_) => {
                    tracing::info!("Removing stale socket file: {}", socket_path.display());
                    std::fs::remove_file(&socket_path)?;
                }
            }
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    #[cfg(windows)]
    let pipe_name = config.pipe_name();

    #[cfg(windows)]
    {
        if crate::platform::ipc::is_daemon_reachable(&pipe_name).await {
            anyhow::bail!(
                "Daemon already running (pipe {} is active). Stop the existing daemon first.",
                pipe_name
            );
        }
    }

    // --- Shared business logic: state, auth, runner restoration ---

    let state = AppState::new(config, daemon_logs);

    // Restore auth token from credential store
    if let Err(e) = state.auth.try_restore().await {
        tracing::warn!("Failed to restore auth from keychain: {}", e);
    }

    // Sync auth token to runner manager so it can query GitHub API for job context
    if let Some(token) = state.auth.token().await {
        state.runner_manager.set_auth_token(Some(token)).await;
    }

    // Load persisted runner configs from disk.
    // Returns IDs of runners that were previously running but whose process is dead.
    // Runners whose process is still alive are reattached automatically.
    let need_restart = match state.runner_manager.load_from_disk().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("Failed to load runners from disk: {}", e);
            Vec::new()
        }
    };

    // Monitor reattached runners (still-alive orphaned processes)
    {
        let runners = state.runner_manager.list().await;
        for runner in &runners {
            if runner.state == crate::runner::state::RunnerState::Online {
                if let Some(pid) = runner.pid {
                    tracing::info!(
                        "Reattached to running process for {} (PID {})",
                        runner.config.name,
                        pid
                    );
                    state
                        .runner_manager
                        .monitor_orphaned_process(&runner.config.id, pid);
                }
            }
        }
    }

    // Restore previously-running runners (dead processes) if preference is enabled
    let restore = state
        .config
        .read()
        .await
        .preferences
        .start_runners_on_launch;
    if restore && !need_restart.is_empty() {
        if let Some(token) = state.auth.token().await {
            tracing::info!(
                "Restoring {} previously-running runner(s)",
                need_restart.len()
            );
            for runner_id in need_restart {
                let manager = state.runner_manager.clone();
                let tok = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager
                        .update_state(&runner_id, crate::runner::state::RunnerState::Registering)
                        .await
                    {
                        tracing::error!("Failed to transition runner {}: {}", runner_id, e);
                        return;
                    }
                    if let Err(e) = manager
                        .register_and_start_from_registering(&runner_id, &tok)
                        .await
                    {
                        tracing::error!("Failed to restore runner {}: {}", runner_id, e);
                        let _ = manager
                            .update_state_with_error(
                                &runner_id,
                                crate::runner::state::RunnerState::Error,
                                Some(format!("{e:#}")),
                            )
                            .await;
                    }
                });
            }
        } else {
            tracing::warn!("Cannot restore runners: no auth token available. Sign in and restart.");
        }
    }

    // Start background poller for job context (branch/PR info)
    state.runner_manager.start_job_context_poller();

    let app = create_router(state);

    // --- Platform-specific: bind listener, serve, shutdown ---

    #[cfg(unix)]
    {
        let listener = UnixListener::bind(&socket_path)?;
        tracing::info!("Listening on Unix socket: {}", socket_path.display());

        let server = axum::serve(listener, app);

        let shutdown_signal = async {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            let sigint = tokio::signal::ctrl_c();
            tokio::select! {
                _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
                _ = sigint => tracing::info!("Received SIGINT"),
            }
        };

        server.with_graceful_shutdown(shutdown_signal).await?;

        // Clean up socket after graceful shutdown
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    #[cfg(windows)]
    {
        let listener = crate::platform::ipc::named_pipe::NamedPipeListener::bind(&pipe_name)?;
        tracing::info!("Listening on named pipe: {}", pipe_name);

        let server = axum::serve(listener, app);

        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to register Ctrl-C handler");
            tracing::info!("Received Ctrl-C");
        };

        server.with_graceful_shutdown(shutdown_signal).await?;

        // Named pipes are kernel objects — no file cleanup needed.
    }

    tracing::info!("Daemon shut down gracefully");
    Ok(())
}

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
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoint_contains_status_ok() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json.get("version").is_some());
    }

    #[tokio::test]
    async fn test_auth_status_unauthenticated() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["authenticated"] == false);
        assert!(json["user"].is_null());
    }

    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent-route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_health_includes_pid() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            json["pid"].is_number(),
            "pid should be a number in health response"
        );
    }

    #[tokio::test]
    async fn test_app_state_new_test_creates_valid_state() {
        let state = AppState::new_test();
        // Verify the state has sensible defaults
        assert!(!state.auth.status().await.authenticated);
        let runners = state.runner_manager.list().await;
        assert!(runners.is_empty());
    }
}
