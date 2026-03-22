use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
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

#[derive(Parser)]
#[command(
    name = "homerun",
    about = "HomeRun — GitHub Actions self-hosted runner manager"
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
    Status,
    /// Scan for repos that use self-hosted runners
    ///
    /// Examples:
    ///   homerun --no-tui scan ~/workspace
    ///   homerun --no-tui scan --remote
    ///   homerun --no-tui scan ~/workspace --remote
    Scan {
        /// Local workspace directory to scan for self-hosted workflows
        path: Option<String>,
        /// Also scan GitHub repos via the API (requires authentication)
        #[arg(long)]
        remote: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        return homerun::cli::run(cli.command.map(|c| match c {
            Commands::List => homerun::cli::CliCommand::List,
            Commands::Status => homerun::cli::CliCommand::Status,
            Commands::Scan { path, remote } => homerun::cli::CliCommand::Scan { path, remote },
        }))
        .await;
    }

    run_tui().await
}

async fn run_tui() -> Result<()> {
    let client = DaemonClient::default_socket();
    let mut app = App::new();

    // Check daemon connectivity
    match client.health().await {
        Ok(_) => app.daemon_connected = true,
        Err(_) => {
            eprintln!(
                "Cannot connect to HomeRun daemon.\n\
                 Make sure homerund is running:\n\n  \
                 homerund\n"
            );
            std::process::exit(1);
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

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start event loop — get both sender and receiver
    let tick_rate = Duration::from_millis(2000);
    let (event_tx, mut events) = start_event_loop(tick_rate)?;

    // Try to connect WebSocket for real-time updates
    if let Ok(ws_read) = client.connect_events().await {
        start_ws_forwarding(event_tx, ws_read);
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
                        handle_action(&client, &mut app, action).await;
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
                }
                AppEvent::DaemonEvent(_json) => {
                    // Real-time event received — refresh runner list
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        app.rebuild_display_items();
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_action(client: &DaemonClient, app: &mut App, action: Action) {
    let result: anyhow::Result<()> = match &action {
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
    };

    match result {
        Ok(_) => {
            app.status_message = Some(format!("{:?} succeeded", action));
            // Refresh runners after any runner/group mutation action
            match &action {
                Action::StartRunner(_)
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
