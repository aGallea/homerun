use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::Show,
    event::KeyEventKind,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use homerun::app::{Action, App};
use homerun::client::DaemonClient;
use homerun::event::{start_event_loop, start_ws_forwarding, AppEvent};
use homerun::ui;

struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, Show);
    }
}

#[derive(Parser)]
#[command(
    name = "homerun",
    about = "HomeRun — GitHub Actions self-hosted runner manager",
    version
)]
struct Cli {
    /// Disable TUI, use plain CLI output
    #[arg(long)]
    no_tui: bool,

    /// Subcommand for CLI mode
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// List all runners
    List,
    /// Show runner and system status
    Status {
        /// Include the full runner table
        #[arg(long)]
        verbose: bool,
    },
    /// Show version, author, and project information
    About,
    /// Authenticate through GitHub Device Flow or a personal access token
    Login {
        /// Authenticate directly with a GitHub token instead of Device Flow
        #[arg(long)]
        token: Option<String>,
    },
    /// Remove the stored GitHub authentication
    Logout,
    /// Create one or more runners
    Add {
        /// Runner name, or name prefix when --count is greater than one
        name: String,
        /// Repository in owner/name form
        #[arg(long)]
        repo: String,
        /// Number of runners to create
        #[arg(long, default_value_t = 1)]
        count: u8,
        /// Comma-separated runner labels
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        /// Runner mode: app or service; create container runners in the desktop app
        #[arg(long)]
        mode: Option<String>,
    },
    /// Start a runner by ID, technical name, or display name
    Start { runner: String },
    /// Stop a runner by ID, technical name, or display name
    Stop { runner: String },
    /// Restart a runner by ID, technical name, or display name
    Restart { runner: String },
    /// Delete a runner by ID, technical name, or display name
    Remove { runner: String },
    /// Change a stopped runner's mode
    SetMode { runner: String, mode: String },
    /// Scan for repos that use self-hosted runners
    Scan {
        path: Option<String>,
        #[arg(long)]
        remote: bool,
    },
    /// Manage the HomeRun daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(clap::Subcommand)]
enum DaemonAction {
    Start,
    Stop,
    Restart,
    /// Manage launch-at-login registration
    Autostart {
        #[command(subcommand)]
        action: AutostartAction,
    },
}

#[derive(clap::Subcommand)]
enum AutostartAction {
    Enable,
    Disable,
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        return homerun::cli::run(cli.command.map(|c| match c {
            Commands::List => homerun::cli::CliCommand::List,
            Commands::Status { verbose } => homerun::cli::CliCommand::Status { verbose },
            Commands::About => homerun::cli::CliCommand::About,
            Commands::Login { token } => homerun::cli::CliCommand::Login { token },
            Commands::Logout => homerun::cli::CliCommand::Logout,
            Commands::Add {
                name,
                repo,
                count,
                labels,
                mode,
            } => homerun::cli::CliCommand::Add {
                name,
                repo,
                count,
                labels,
                mode,
            },
            Commands::Start { runner } => homerun::cli::CliCommand::Start { runner },
            Commands::Stop { runner } => homerun::cli::CliCommand::Stop { runner },
            Commands::Restart { runner } => homerun::cli::CliCommand::Restart { runner },
            Commands::Remove { runner } => homerun::cli::CliCommand::Remove { runner },
            Commands::SetMode { runner, mode } => {
                homerun::cli::CliCommand::SetMode { runner, mode }
            }
            Commands::Scan { path, remote } => homerun::cli::CliCommand::Scan { path, remote },
            Commands::Daemon { action } => homerun::cli::CliCommand::Daemon(match action {
                DaemonAction::Start => homerun::cli::DaemonAction::Start,
                DaemonAction::Stop => homerun::cli::DaemonAction::Stop,
                DaemonAction::Restart => homerun::cli::DaemonAction::Restart,
                DaemonAction::Autostart { action } => {
                    homerun::cli::DaemonAction::Autostart(match action {
                        AutostartAction::Enable => homerun::cli::AutostartAction::Enable,
                        AutostartAction::Disable => homerun::cli::AutostartAction::Disable,
                        AutostartAction::Status => homerun::cli::AutostartAction::Status,
                    })
                }
            }),
        }))
        .await;
    }

    run_tui().await
}

async fn run_tui() -> Result<()> {
    let client = DaemonClient::default_socket()?;
    let mut app = App::new();

    // Check daemon connectivity
    match client.health().await {
        Ok(_) => app.daemon_connected = true,
        Err(_) => {
            app.daemon_connected = false;
            app.active_tab = homerun::app::Tab::Daemon;
        }
    }

    // Initial data load
    if let Ok(runners) = client.list_runners().await {
        app.runners = runners;
        app.rebuild_display_items();
    }
    if let Ok(auth) = client.auth_status().await {
        app.auth_status = Some(auth);
    }
    if let Ok(metrics) = client.get_metrics().await {
        app.metrics = Some(metrics);
    }

    // Auto-start device flow if not authenticated
    let auto_login =
        !app.auth_status.as_ref().is_some_and(|a| a.authenticated) && app.daemon_connected;

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal_restore = TerminalRestoreGuard;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start event loop — get both sender and receiver
    let tick_rate = Duration::from_millis(2000);
    let (event_tx, mut events) = start_event_loop(tick_rate)?;

    // Try to connect WebSocket for real-time updates
    if let Ok(ws_read) = client.connect_events().await {
        start_ws_forwarding(event_tx.clone(), ws_read);
    }

    if auto_login {
        if let Ok(flow) = client.start_device_flow().await {
            let device_code = flow.device_code.clone();
            let interval = flow.interval;
            app.login_state = Some(homerun::app::LoginState::Polling {
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                interval,
            });
            start_login_polling(event_tx.clone(), device_code, interval);
        }
    }

    // Main loop
    let mut poll_counter = 0u32;
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Some(event) = events.recv().await {
            match event {
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if let Some(action) = app.handle_key(key.code, key.modifiers) {
                        handle_action(&client, &mut app, action, event_tx.clone()).await;
                    }
                }
                AppEvent::Tick => {
                    poll_counter += 1;
                    // Refresh runners every tick
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        // Clamp selection
                        if app.selected_runner_index >= app.runners.len() && !app.runners.is_empty()
                        {
                            app.selected_runner_index = app.runners.len() - 1;
                        }
                        app.rebuild_display_items();
                    }
                    // Fetch steps for selected runner if it's busy
                    if let Some(runner) = app.selected_runner() {
                        if runner.state == "busy" {
                            let rid = runner.config.id.clone();
                            if let Ok(steps) = client.get_runner_steps(&rid).await {
                                app.selected_runner_steps = Some(steps);
                            }
                        } else {
                            app.selected_runner_steps = None;
                        }
                    } else {
                        app.selected_runner_steps = None;
                    }
                    // Fetch job history for selected runner
                    if let Some(runner) = app.selected_runner() {
                        let rid = runner.config.id.clone();
                        if let Ok(history) = client.get_job_history(&rid).await {
                            app.selected_runner_history = history;
                        }
                    } else {
                        app.selected_runner_history.clear();
                    }
                    // Refresh metrics every 5 ticks (~10 seconds)
                    if poll_counter.is_multiple_of(5) {
                        if let Ok(metrics) = client.get_metrics().await {
                            app.metrics = Some(metrics);
                        }
                    }
                    // Refresh daemon logs when on Daemon tab
                    if app.active_tab == homerun::app::Tab::Daemon {
                        refresh_daemon_logs(&client, &mut app).await;
                    }
                    // Auto-dismiss success state (set on the previous tick)
                    if let Some(homerun::app::LoginState::Success { ref username }) =
                        app.login_state
                    {
                        app.status_message = Some(format!("Logged in as {username}"));
                        app.login_state = None;
                        // Refresh repos now that we're authenticated
                        if let Ok(repos) = client.list_repos().await {
                            app.repos = repos;
                        }
                    }
                }
                AppEvent::DaemonEvent(_json) => {
                    // Real-time event received — refresh runner list
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        app.rebuild_display_items();
                    }
                }
                AppEvent::LoginCompleted(result) => match result {
                    Ok(auth) => {
                        let username = auth
                            .user
                            .as_ref()
                            .map(|user| user.login.clone())
                            .unwrap_or_else(|| "unknown".to_string());
                        app.auth_status = Some(auth);
                        app.login_state = Some(homerun::app::LoginState::Success { username });
                    }
                    Err(message) => {
                        app.login_state = Some(homerun::app::LoginState::Error { message });
                    }
                },
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Drop the event receiver so background tasks detect the closed channel and exit
    drop(events);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    std::mem::forget(terminal_restore);

    // Force exit — spawn_blocking tasks can keep the tokio runtime alive
    std::process::exit(0);
}

fn start_login_polling(
    event_tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    device_code: String,
    interval: u64,
) {
    tokio::spawn(async move {
        let client = match DaemonClient::default_socket() {
            Ok(client) => client,
            Err(error) => {
                let _ = event_tx.send(AppEvent::LoginCompleted(Err(error.to_string())));
                return;
            }
        };
        loop {
            match client.poll_device_flow(&device_code, interval).await {
                Ok(Some(auth)) => {
                    let _ = event_tx.send(AppEvent::LoginCompleted(Ok(auth)));
                    break;
                }
                Ok(None) => continue,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::LoginCompleted(Err(error.to_string())));
                    break;
                }
            }
        }
    });
}

async fn handle_action(
    client: &DaemonClient,
    app: &mut App,
    action: Action,
    event_tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    let result: anyhow::Result<()> = match &action {
        Action::AddRunner(repo) => client
            .create_runner(&homerun::client::CreateRunnerRequest {
                repo_full_name: repo.clone(),
                name: None,
                labels: None,
                mode: None,
            })
            .await
            .map(|_| ()),
        Action::LoadRunnerLogs(id) => client.get_runner_logs(id).await.map(|logs| {
            app.runner_logs = logs;
            app.show_runner_logs = true;
        }),
        Action::StartRunner(id) => client.start_runner(id).await,
        Action::StopRunner(id) => client.stop_runner(id).await,
        Action::RestartRunner(id) => client.restart_runner(id).await,
        Action::DeleteRunner(id) => client.delete_runner(id).await,
        Action::StartGroup(gid) => client.start_group(gid).await.map(|_| ()),
        Action::StopGroup(gid) => client.stop_group(gid).await.map(|_| ()),
        Action::RestartGroup(gid) => client.restart_group(gid).await.map(|_| ()),
        Action::DeleteGroup(gid) => client.delete_group(gid).await.map(|_| ()),
        Action::ScaleUp(gid) => {
            let runners = app
                .runners
                .iter()
                .filter(|r| r.config.group_id.as_deref() == Some(gid))
                .count();
            let target = (runners + 1).min(10) as u8;
            client.scale_group(gid, target).await.map(|_| ())
        }
        Action::ScaleDown(gid) => {
            let runners = app
                .runners
                .iter()
                .filter(|r| r.config.group_id.as_deref() == Some(gid))
                .count();
            let target = runners.saturating_sub(1).max(1) as u8;
            client.scale_group(gid, target).await.map(|_| ())
        }
        Action::RefreshRunners => {
            if let Ok(runners) = client.list_runners().await {
                app.runners = runners;
                app.rebuild_display_items();
            }
            Ok(())
        }
        Action::RefreshRepos => {
            if let Ok(repos) = client.list_repos().await {
                app.repos = repos;
            }
            Ok(())
        }
        Action::RefreshMetrics => {
            if let Ok(metrics) = client.get_metrics().await {
                app.metrics = Some(metrics);
            }
            Ok(())
        }
        Action::RefreshDaemonLogs => {
            refresh_daemon_logs(client, app).await;
            Ok(())
        }
        Action::StartDaemon => {
            match homerun::daemon_lifecycle::start_daemon().await {
                Ok(()) => {
                    app.daemon_connected = true;
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        app.rebuild_display_items();
                    }
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {e}"));
                }
            }
            Ok(())
        }
        Action::StopDaemon => {
            match homerun::daemon_lifecycle::stop_daemon().await {
                Ok(()) => app.daemon_connected = false,
                Err(e) => {
                    app.status_message = Some(format!("Error: {e}"));
                }
            }
            Ok(())
        }
        Action::RestartDaemon => {
            match homerun::daemon_lifecycle::restart_daemon().await {
                Ok(()) => {
                    app.daemon_connected = true;
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        app.rebuild_display_items();
                    }
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {e}"));
                }
            }
            Ok(())
        }
        Action::StartLogin => {
            match client.start_device_flow().await {
                Ok(flow) => {
                    let device_code = flow.device_code.clone();
                    let interval = flow.interval;
                    app.login_state = Some(homerun::app::LoginState::Polling {
                        device_code: flow.device_code,
                        user_code: flow.user_code,
                        verification_uri: flow.verification_uri,
                        interval,
                    });
                    start_login_polling(event_tx, device_code, interval);
                }
                Err(e) => {
                    app.login_state = Some(homerun::app::LoginState::Error {
                        message: e.to_string(),
                    });
                }
            }
            Ok(())
        }
    };

    match result {
        Ok(_) => {
            app.status_message = Some(format!("{:?} succeeded", action));
            // Refresh runners after any runner/group mutation action
            match &action {
                Action::AddRunner(_)
                | Action::StartRunner(_)
                | Action::StopRunner(_)
                | Action::RestartRunner(_)
                | Action::DeleteRunner(_)
                | Action::StartGroup(_)
                | Action::StopGroup(_)
                | Action::RestartGroup(_)
                | Action::DeleteGroup(_)
                | Action::ScaleUp(_)
                | Action::ScaleDown(_) => {
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        if app.selected_runner_index >= app.runners.len() && !app.runners.is_empty()
                        {
                            app.selected_runner_index = app.runners.len() - 1;
                        }
                        app.rebuild_display_items();
                    }
                }
                _ => {}
            }
        }
        Err(e) => {
            app.status_message = Some(format!("Error: {e}"));
        }
    }
}

async fn refresh_daemon_logs(client: &DaemonClient, app: &mut App) {
    let level = Some(app.daemon_log_level.as_str());
    let search = if app.daemon_search.is_empty() {
        None
    } else {
        Some(app.daemon_search.as_str())
    };
    if let Ok(logs) = client
        .get_daemon_logs_recent(level, Some(500), search)
        .await
    {
        let was_following = app.daemon_follow;
        app.daemon_logs = logs;
        if was_following && !app.daemon_logs.is_empty() {
            app.daemon_log_scroll = app.daemon_logs.len().saturating_sub(1);
        }
    }
}
