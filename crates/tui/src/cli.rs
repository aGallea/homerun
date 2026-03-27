// Plain CLI mode (--no-tui)
use anyhow::Result;

use crate::client::DaemonClient;

pub enum CliCommand {
    List,
    Status,
    Scan {
        /// Local workspace path to scan (None = skip local scan)
        path: Option<String>,
        /// Also scan GitHub repos via API
        remote: bool,
    },
    Daemon(DaemonAction),
}

pub enum DaemonAction {
    Start,
    Stop,
    Restart,
}

pub async fn run(command: Option<CliCommand>) -> Result<()> {
    // Handle daemon commands first (don't require daemon to be running)
    if let Some(CliCommand::Daemon(action)) = &command {
        return match action {
            DaemonAction::Start => {
                println!("Starting daemon...");
                crate::daemon_lifecycle::start_daemon().await?;
                println!("Daemon started.");
                Ok(())
            }
            DaemonAction::Stop => {
                println!("Stopping daemon...");
                crate::daemon_lifecycle::stop_daemon().await?;
                println!("Daemon stopped.");
                Ok(())
            }
            DaemonAction::Restart => {
                println!("Restarting daemon...");
                crate::daemon_lifecycle::restart_daemon().await?;
                println!("Daemon restarted.");
                Ok(())
            }
        };
    }

    let client = DaemonClient::default_socket();

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
        Some(CliCommand::Status) => cmd_status(&client).await,
        Some(CliCommand::Scan { path, remote }) => cmd_scan(&client, path, remote).await,
        Some(CliCommand::Daemon(_)) => unreachable!(),
        None => {
            eprintln!(
                "No command specified. Use `homerun --no-tui list` or `homerun --no-tui status`."
            );
            std::process::exit(1);
        }
    }
}

fn colored(text: &str, color_code: &str) -> String {
    if atty_stdout() {
        format!("\x1b[{color_code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

fn color_for_state(state: &str) -> &'static str {
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

async fn cmd_list(client: &DaemonClient) -> Result<()> {
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

async fn cmd_status(client: &DaemonClient) -> Result<()> {
    let auth = client.auth_status().await?;
    let runners = client.list_runners().await?;
    let metrics = client.get_metrics().await.ok();

    let online = runners.iter().filter(|r| r.state == "online").count();
    let busy = runners.iter().filter(|r| r.state == "busy").count();
    let offline = runners.iter().filter(|r| r.state == "offline").count();

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
        "  Runners: {total} total ({} online, {} busy, {} offline)",
        colored(&online.to_string(), "32"),
        colored(&busy.to_string(), "33"),
        colored(&offline.to_string(), "90"),
    );

    if let Some(m) = &metrics {
        let mem_used_gb = m.system.memory_used_bytes as f64 / 1_073_741_824.0;
        let mem_total_gb = m.system.memory_total_bytes as f64 / 1_073_741_824.0;
        println!(
            "  CPU: {:.0}%  Memory: {:.1} GB / {:.1} GB",
            m.system.cpu_percent, mem_used_gb, mem_total_gb,
        );
    }

    Ok(())
}

async fn cmd_scan(client: &DaemonClient, path: Option<String>, remote: bool) -> Result<()> {
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
    // Format helpers tested inline — integration tests require a live daemon.
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
}
