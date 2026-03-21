// Plain CLI mode (--no-tui)
use anyhow::Result;

use crate::client::DaemonClient;

pub enum CliCommand {
    List,
    Status,
}

pub async fn run(command: Option<CliCommand>) -> Result<()> {
    let client = DaemonClient::default_socket();

    // Check daemon connectivity first
    if client.health().await.is_err() {
        eprintln!(
            "Cannot connect to HomeRun daemon.\n\
             Make sure homerund is running:\n\n  \
             homerund\n"
        );
        std::process::exit(1);
    }

    match command {
        Some(CliCommand::List) => cmd_list(&client).await,
        Some(CliCommand::Status) => cmd_status(&client).await,
        None => {
            eprintln!(
                "No command specified. Use `homerun --no-tui list` or `homerun --no-tui status`."
            );
            std::process::exit(1);
        }
    }
}

async fn cmd_list(client: &DaemonClient) -> Result<()> {
    let runners = client.list_runners().await?;
    let metrics = client.get_metrics().await.ok();

    // Column widths
    let name_w = 20;
    let repo_w = 22;
    let status_w = 8;
    let mode_w = 9;

    println!(
        "{:<name_w$} {:<repo_w$} {:<status_w$} {:<mode_w$} CPU",
        "NAME",
        "REPO",
        "STATUS",
        "MODE",
        name_w = name_w,
        repo_w = repo_w,
        status_w = status_w,
        mode_w = mode_w,
    );

    for runner in &runners {
        let repo = format!("{}/{}", runner.config.repo_owner, runner.config.repo_name);

        // Look up per-runner CPU if available
        let cpu_str = metrics
            .as_ref()
            .and_then(|m| {
                m.runners
                    .iter()
                    .find(|r| r.runner_id == runner.config.id)
                    .map(|r| format!("{:.0}%", r.cpu_percent))
            })
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<name_w$} {:<repo_w$} {:<status_w$} {:<mode_w$} {}",
            runner.config.name,
            repo,
            runner.state,
            runner.config.mode,
            cpu_str,
            name_w = name_w,
            repo_w = repo_w,
            status_w = status_w,
            mode_w = mode_w,
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

    println!("HomeRun Status");
    println!("  Daemon: running");
    println!("  User: {user}");
    println!("  Runners: {online} online, {busy} busy, {offline} offline");

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
