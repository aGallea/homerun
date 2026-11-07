// Plain CLI mode (--no-tui)
use anyhow::{bail, Result};

use crate::client::{CreateRunnerRequest, DaemonClient, RunnerInfo};

pub enum CliCommand {
    List,
    Status {
        verbose: bool,
    },
    About,
    Login {
        token: Option<String>,
    },
    Logout,
    Add {
        name: String,
        repo: String,
        count: u8,
        labels: Option<Vec<String>>,
        mode: Option<String>,
    },
    Start {
        runner: String,
    },
    Stop {
        runner: String,
    },
    Restart {
        runner: String,
    },
    Remove {
        runner: String,
    },
    SetMode {
        runner: String,
        mode: String,
    },
    Scan {
        path: Option<String>,
        remote: bool,
    },
    Daemon(DaemonAction),
}

pub enum DaemonAction {
    Start,
    Stop,
    Restart,
    Autostart(AutostartAction),
}

pub enum AutostartAction {
    Enable,
    Disable,
    Status,
}

pub async fn run(command: Option<CliCommand>) -> Result<()> {
    if let Some(CliCommand::About) = &command {
        return cmd_about();
    }

    if let Some(CliCommand::Daemon(action)) = &command {
        match action {
            DaemonAction::Start => {
                println!("Starting daemon...");
                crate::daemon_lifecycle::start_daemon().await?;
                println!("Daemon started.");
                return Ok(());
            }
            DaemonAction::Stop => {
                println!("Stopping daemon...");
                crate::daemon_lifecycle::stop_daemon().await?;
                println!("Daemon stopped.");
                return Ok(());
            }
            DaemonAction::Restart => {
                println!("Restarting daemon...");
                crate::daemon_lifecycle::restart_daemon().await?;
                println!("Daemon restarted.");
                return Ok(());
            }
            DaemonAction::Autostart(_) => {}
        }
    }

    let client = DaemonClient::default_socket()?;
    if client.health().await.is_err() {
        eprintln!(
            "Cannot connect to HomeRun daemon.\n\
             Make sure homerund is running:\n\n  \
             homerund\n\n  \
             Or start it with: homerun --no-tui daemon start\n"
        );
        std::process::exit(1);
    }

    match command {
        Some(CliCommand::List) => cmd_list(&client).await,
        Some(CliCommand::Status { verbose }) => cmd_status(&client, verbose).await,
        Some(CliCommand::Login { token }) => cmd_login(&client, token).await,
        Some(CliCommand::Logout) => {
            client.logout().await?;
            println!("Logged out.");
            Ok(())
        }
        Some(CliCommand::Add {
            name,
            repo,
            count,
            labels,
            mode,
        }) => cmd_add(&client, name, repo, count, labels, mode).await,
        Some(CliCommand::Start { runner }) => cmd_lifecycle(&client, &runner, "start").await,
        Some(CliCommand::Stop { runner }) => cmd_lifecycle(&client, &runner, "stop").await,
        Some(CliCommand::Restart { runner }) => cmd_lifecycle(&client, &runner, "restart").await,
        Some(CliCommand::Remove { runner }) => cmd_lifecycle(&client, &runner, "remove").await,
        Some(CliCommand::SetMode { runner, mode }) => cmd_set_mode(&client, &runner, mode).await,
        Some(CliCommand::Scan { path, remote }) => cmd_scan(&client, path, remote).await,
        Some(CliCommand::Daemon(DaemonAction::Autostart(action))) => {
            cmd_autostart(&client, action).await
        }
        Some(CliCommand::About | CliCommand::Daemon(_)) => unreachable!(),
        None => {
            eprintln!("No command specified. Run `homerun --help` for available commands.");
            std::process::exit(1);
        }
    }
}

async fn resolve_runner(client: &DaemonClient, selector: &str) -> Result<RunnerInfo> {
    let runners = client.list_runners().await?;
    if let Some(exact) = runners.iter().find(|runner| runner.config.id == selector) {
        return Ok(exact.clone());
    }
    let matches: Vec<_> = runners
        .into_iter()
        .filter(|runner| {
            runner.config.name == selector
                || runner.config.display_name.as_deref() == Some(selector)
        })
        .collect();
    match matches.as_slice() {
        [] => bail!("Runner '{selector}' was not found"),
        [runner] => Ok(runner.clone()),
        _ => bail!("Runner selector '{selector}' is ambiguous; use the runner ID"),
    }
}

async fn cmd_login(client: &DaemonClient, token: Option<String>) -> Result<()> {
    if let Some(token) = token {
        let auth = client.login_with_token(&token).await?;
        let user = auth
            .user
            .map(|user| user.login)
            .unwrap_or_else(|| "unknown".to_string());
        println!("Logged in as {user}.");
        return Ok(());
    }

    let flow = client.start_device_flow().await?;
    println!(
        "Open {} and enter code {}",
        flow.verification_uri, flow.user_code
    );
    loop {
        match client
            .poll_device_flow(&flow.device_code, flow.interval)
            .await?
        {
            Some(auth) => {
                let user = auth
                    .user
                    .map(|user| user.login)
                    .unwrap_or_else(|| "unknown".to_string());
                println!("Logged in as {user}.");
                return Ok(());
            }
            None => continue,
        }
    }
}

fn normalize_cli_mode(mode: Option<String>) -> Result<Option<String>> {
    match mode.as_deref() {
        None => Ok(None),
        Some("app") => Ok(Some("app".to_string())),
        Some("service") => Ok(Some("service".to_string())),
        Some("container") => {
            bail!("container runners require an image and must be created from the desktop app")
        }
        Some(other) => bail!("unsupported runner mode '{other}'; use app or service"),
    }
}

async fn cmd_add(
    client: &DaemonClient,
    name: String,
    repo: String,
    count: u8,
    labels: Option<Vec<String>>,
    mode: Option<String>,
) -> Result<()> {
    let mode = normalize_cli_mode(mode)?;
    if count == 0 || count > 10 {
        bail!("count must be between 1 and 10");
    }
    if count == 1 {
        let runner = client
            .create_runner(&CreateRunnerRequest {
                repo_full_name: repo,
                name: Some(name),
                labels,
                mode,
            })
            .await?;
        println!(
            "Created runner {} ({})",
            runner.config.name, runner.config.id
        );
    } else {
        let result = client
            .create_batch(&repo, count, Some(name), labels, mode)
            .await?;
        for runner in result.runners {
            println!(
                "Created runner {} ({})",
                runner.config.name, runner.config.id
            );
        }
        for error in result.errors {
            eprintln!("Runner {} failed: {}", error.index + 1, error.error);
        }
    }
    Ok(())
}

async fn cmd_lifecycle(client: &DaemonClient, selector: &str, action: &str) -> Result<()> {
    let runner = resolve_runner(client, selector).await?;
    match action {
        "start" => client.start_runner(&runner.config.id).await?,
        "stop" => client.stop_runner(&runner.config.id).await?,
        "restart" => client.restart_runner(&runner.config.id).await?,
        "remove" => client.delete_runner(&runner.config.id).await?,
        _ => unreachable!(),
    }
    println!("{} {}.", action, runner.config.name);
    Ok(())
}

async fn cmd_set_mode(client: &DaemonClient, selector: &str, mode: String) -> Result<()> {
    let mode = normalize_cli_mode(Some(mode))?.expect("set-mode always supplies a mode");
    let runner = resolve_runner(client, selector).await?;
    client
        .update_runner(&runner.config.id, None, Some(mode.clone()), None)
        .await?;
    println!("Set {} mode to {mode}.", runner.config.name);
    Ok(())
}

async fn cmd_autostart(client: &DaemonClient, action: AutostartAction) -> Result<()> {
    match action {
        AutostartAction::Enable => {
            client.install_service().await?;
            println!("Daemon autostart enabled.");
        }
        AutostartAction::Disable => {
            client.uninstall_service().await?;
            println!("Daemon autostart disabled.");
        }
        AutostartAction::Status => {
            println!(
                "Daemon autostart: {}",
                if client.service_status().await? {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
    }
    Ok(())
}

pub fn colored(text: &str, color_code: &str) -> String {
    colored_impl(text, color_code, atty_stdout())
}

fn colored_impl(text: &str, color_code: &str, is_tty: bool) -> String {
    if is_tty {
        format!("\x1b[{color_code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

pub fn color_for_state(state: &str) -> &'static str {
    match state {
        "online" => "32",                   // green
        "busy" => "33",                     // yellow
        "offline" => "90",                  // gray
        "error" => "31",                    // red
        "creating" | "registering" => "36", // cyan
        "stopping" | "deleting" => "35",    // magenta
        _ => "0",                           // default
    }
}

pub fn cmd_about() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "{}\n",
        colored("HomeRun — GitHub Actions self-hosted runner manager", "1")
    );
    println!("  Version:     {version}");
    println!("  License:     MIT");
    println!("  Author:      aGallea (https://github.com/aGallea)");
    println!("  Repository:  https://github.com/lgg/homerun");
    println!();
    println!("Feedback:");
    println!("  Bug report:      https://github.com/lgg/homerun/issues/new?template=bug_report.md");
    println!(
        "  Feature request:  https://github.com/lgg/homerun/issues/new?template=feature_request.md"
    );
    Ok(())
}

pub async fn cmd_list(client: &DaemonClient) -> Result<()> {
    let runners = client.list_runners().await?;
    let metrics = client.get_metrics().await.ok();

    if runners.is_empty() {
        println!("No runners configured.");
        return Ok(());
    }

    // Calculate column widths dynamically
    let name_w = runners
        .iter()
        .map(|r| r.config.name.len())
        .max()
        .unwrap_or(4)
        .max(4); // "NAME"
    let repo_w = runners
        .iter()
        .map(|r| r.config.repo_owner.len() + 1 + r.config.repo_name.len())
        .max()
        .unwrap_or(4)
        .max(4); // "REPO"
    let status_w = 8; // "STATUS" + padding
    let mode_w = 9; // "MODE" + padding

    println!(
        "{:<name_w$} {:<repo_w$} {:<status_w$} {:<mode_w$} CPU",
        "NAME", "REPO", "STATUS", "MODE",
    );

    for runner in &runners {
        let repo = format!("{}/{}", runner.config.repo_owner, runner.config.repo_name);

        let cpu_str = metrics
            .as_ref()
            .and_then(|m| {
                m.runners
                    .iter()
                    .find(|r| r.runner_id == runner.config.id)
                    .map(|r| format!("{:.0}%", r.cpu_percent))
            })
            .unwrap_or_else(|| "-".to_string());

        let padded_state = format!("{:<status_w$}", runner.state);
        let colored_state = colored(&padded_state, color_for_state(&runner.state));

        println!(
            "{:<name_w$} {:<repo_w$} {} {:<mode_w$} {}",
            runner.config.name, repo, colored_state, runner.config.mode, cpu_str,
        );
    }

    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct RunnerStateCounts {
    online: usize,
    busy: usize,
    offline: usize,
    error: usize,
    transitioning: usize,
}

fn count_runner_states<'a>(states: impl IntoIterator<Item = &'a str>) -> RunnerStateCounts {
    let mut counts = RunnerStateCounts::default();
    for state in states {
        match state {
            "online" => counts.online += 1,
            "busy" => counts.busy += 1,
            "offline" => counts.offline += 1,
            "error" => counts.error += 1,
            _ => counts.transitioning += 1,
        }
    }
    counts
}

pub async fn cmd_status(client: &DaemonClient, verbose: bool) -> Result<()> {
    let auth = client.auth_status().await?;
    let runners = client.list_runners().await?;
    let metrics = client.get_metrics().await.ok();

    let counts = count_runner_states(runners.iter().map(|runner| runner.state.as_str()));

    let user = auth
        .user
        .as_ref()
        .map(|u| u.login.as_str())
        .unwrap_or("(not authenticated)");

    let version = env!("CARGO_PKG_VERSION");
    println!("HomeRun Status (v{version})");
    println!("  Daemon: {}", colored("running", "32"));

    let user_display = if auth.authenticated {
        colored(user, "32") // green
    } else {
        colored(user, "31") // red
    };
    println!("  User: {user_display}");

    let total = runners.len();
    println!(
        "  Runners: {total} total ({} online, {} busy, {} offline, {} error, {} transitioning)",
        colored(&counts.online.to_string(), "32"),
        colored(&counts.busy.to_string(), "33"),
        colored(&counts.offline.to_string(), "90"),
        colored(&counts.error.to_string(), "31"),
        colored(&counts.transitioning.to_string(), "36"),
    );

    if let Some(m) = &metrics {
        let mem_used_gb = m.system.memory_used_bytes as f64 / 1_073_741_824.0;
        let mem_total_gb = m.system.memory_total_bytes as f64 / 1_073_741_824.0;
        println!(
            "  CPU: {:.0}%  Memory: {:.1} GB / {:.1} GB",
            m.system.cpu_percent, mem_used_gb, mem_total_gb,
        );
    }

    if verbose && !runners.is_empty() {
        println!();
        cmd_list(client).await?;
    }

    Ok(())
}

pub async fn cmd_scan(client: &DaemonClient, path: Option<String>, remote: bool) -> Result<()> {
    use crate::client::DiscoveredRepo;
    use std::collections::HashMap;

    let mut all: HashMap<String, DiscoveredRepo> = HashMap::new();

    if let Some(ref p) = path {
        println!("Scanning local workspace: {p}");
        match client.scan_local(p).await {
            Ok(repos) => {
                for repo in repos {
                    all.insert(repo.full_name.clone(), repo);
                }
            }
            Err(e) => eprintln!("Local scan error: {e}"),
        }
    }

    if remote {
        println!("Scanning GitHub repos via API…");
        match client.scan_remote().await {
            Ok(repos) => {
                for repo in repos {
                    all.entry(repo.full_name.clone())
                        .and_modify(|existing| {
                            existing.source = "both".to_string();
                            for wf in &repo.workflow_files {
                                if !existing.workflow_files.contains(wf) {
                                    existing.workflow_files.push(wf.clone());
                                }
                            }
                            existing.workflow_files.sort();
                        })
                        .or_insert(repo);
                }
            }
            Err(e) => eprintln!("Remote scan error: {e}"),
        }
    }

    if all.is_empty() {
        println!("No repos with self-hosted runners found.");
        return Ok(());
    }

    let mut sorted: Vec<&DiscoveredRepo> = all.values().collect();
    sorted.sort_by(|a, b| a.full_name.cmp(&b.full_name));

    println!("\nRepos using self-hosted runners:");
    println!("{:-<60}", "");
    for repo in sorted {
        println!("  {} [{}]", repo.full_name, repo.source);
        for wf in &repo.workflow_files {
            println!("    - {wf}");
        }
        if let Some(ref p) = repo.local_path {
            println!("    path: {}", p.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_cli_mode_accepts_native_modes() {
        assert_eq!(
            normalize_cli_mode(Some("app".into())).unwrap(),
            Some("app".into())
        );
        assert_eq!(
            normalize_cli_mode(Some("service".into())).unwrap(),
            Some("service".into())
        );
    }

    #[test]
    fn test_normalize_cli_mode_rejects_container_without_config() {
        let error = normalize_cli_mode(Some("container".into())).unwrap_err();
        assert!(error.to_string().contains("desktop app"));
    }

    #[test]
    fn test_cmd_about_succeeds() {
        let result = cmd_about();
        assert!(result.is_ok());
    }

    #[test]
    fn test_memory_formatting() {
        let bytes: u64 = 4_509_715_456; // ~4.2 GB
        let gb = bytes as f64 / 1_073_741_824.0;
        let formatted = format!("{gb:.1}");
        assert_eq!(formatted, "4.2");
    }

    #[test]
    fn test_cpu_formatting() {
        let cpu: f64 = 23.4;
        let formatted = format!("{:.0}%", cpu);
        assert_eq!(formatted, "23%");
    }

    #[test]
    fn test_color_for_state_returns_correct_codes() {
        assert_eq!(color_for_state("online"), "32");
        assert_eq!(color_for_state("busy"), "33");
        assert_eq!(color_for_state("offline"), "90");
        assert_eq!(color_for_state("error"), "31");
        assert_eq!(color_for_state("creating"), "36");
        assert_eq!(color_for_state("registering"), "36");
        assert_eq!(color_for_state("stopping"), "35");
        assert_eq!(color_for_state("deleting"), "35");
        assert_eq!(color_for_state("unknown"), "0");
    }

    #[test]
    fn test_colored_without_terminal() {
        assert_eq!(colored_impl("hello", "32", false), "hello");
    }

    #[test]
    fn test_colored_with_terminal() {
        assert_eq!(colored_impl("hello", "32", true), "\x1b[32mhello\x1b[0m");
    }

    #[test]
    fn test_count_runner_states_includes_errors_and_transitions() {
        let counts = count_runner_states([
            "online",
            "busy",
            "offline",
            "error",
            "registering",
            "stopping",
        ]);
        assert_eq!(
            counts,
            RunnerStateCounts {
                online: 1,
                busy: 1,
                offline: 1,
                error: 1,
                transitioning: 2,
            }
        );
    }
}
