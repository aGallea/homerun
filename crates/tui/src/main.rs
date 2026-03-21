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
#[command(name = "homerun", about = "HomeRun — GitHub Actions self-hosted runner manager")]
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        return homerun::cli::run(cli.command.map(|c| match c {
            Commands::List => homerun::cli::CliCommand::List,
            Commands::Status => homerun::cli::CliCommand::Status,
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
                        if app.selected_runner_index >= app.runners.len()
                            && !app.runners.is_empty()
                        {
                            app.selected_runner_index = app.runners.len() - 1;
                        }
                    }
                    // Refresh metrics every 5 ticks (~10 seconds)
                    if poll_counter % 5 == 0 {
                        if let Ok(metrics) = client.get_metrics().await {
                            app.metrics = Some(metrics);
                        }
                    }
                }
                AppEvent::DaemonEvent(_json) => {
                    // Real-time event received — refresh runner list
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
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
        Action::RefreshRunners => {
            if let Ok(runners) = client.list_runners().await {
                app.runners = runners;
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
    };

    match result {
        Ok(_) => {
            app.status_message = Some(format!("{:?} succeeded", action));
            // Refresh runners after any runner mutation action
            match &action {
                Action::StartRunner(_)
                | Action::StopRunner(_)
                | Action::RestartRunner(_)
                | Action::DeleteRunner(_) => {
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                        if app.selected_runner_index >= app.runners.len()
                            && !app.runners.is_empty()
                        {
                            app.selected_runner_index = app.runners.len() - 1;
                        }
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
