use std::time::Duration;

#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};

use crate::client::DaemonClient;

#[cfg(unix)]
fn default_socket_path() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join(".homerun/daemon.sock")
}

#[cfg(windows)]
fn default_pipe_name() -> String {
    r"\\.\pipe\homerun-daemon".to_string()
}

#[cfg(unix)]
async fn is_daemon_running(socket: &Path) -> bool {
    if !socket.exists() {
        return false;
    }
    let client = DaemonClient::new(socket.to_path_buf());
    client.health().await.is_ok()
}

#[cfg(windows)]
async fn is_daemon_running() -> bool {
    let client = DaemonClient::new_pipe(default_pipe_name());
    client.health().await.is_ok()
}

pub async fn start_daemon() -> Result<()> {
    #[cfg(unix)]
    {
        let socket = default_socket_path();
        if is_daemon_running(&socket).await {
            bail!("Daemon is already running");
        }
        if socket.exists() {
            std::fs::remove_file(&socket)?;
        }
    }
    #[cfg(windows)]
    {
        if is_daemon_running().await {
            bail!("Daemon is already running");
        }
    }

    let binary = which::which("homerund")
        .context("homerund not found in PATH. Install it or add it to your PATH.")?;
    std::process::Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn homerund")?;

    #[cfg(unix)]
    let client = DaemonClient::new(default_socket_path());
    #[cfg(windows)]
    let client = DaemonClient::new_pipe(default_pipe_name());

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if client.health().await.is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("Daemon failed to start within 5 seconds — check logs at ~/.homerun/logs/");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub async fn stop_daemon() -> Result<()> {
    #[cfg(unix)]
    let socket = default_socket_path();

    #[cfg(unix)]
    if !socket.exists() {
        bail!("Daemon is not running (no socket file)");
    }

    #[cfg(windows)]
    if !is_daemon_running().await {
        bail!("Daemon is not running");
    }

    #[cfg(unix)]
    let client = DaemonClient::new(socket.clone());
    #[cfg(windows)]
    let client = DaemonClient::new_pipe(default_pipe_name());

    let active_runners = match client.shutdown().await {
        Ok(count) => count,
        Err(e) => {
            let msg = format!("{e}");
            #[cfg(unix)]
            if msg.contains("launchd") || msg.contains("Uninstall the service") {
                bail!(
                    "Daemon is managed by launchd. Uninstall the service first \
                     (Settings > Startup) or run: launchctl unload ~/Library/LaunchAgents/com.homerun.daemon.plist"
                );
            }
            #[cfg(windows)]
            if msg.contains("Uninstall the service") {
                bail!(
                    "Daemon is managed as a Windows service. Uninstall the service first \
                     (Settings > Startup) or use: sc delete homerun-daemon"
                );
            }
            #[cfg(unix)]
            if socket.exists() {
                std::fs::remove_file(&socket)?;
            }
            return Ok(());
        }
    };

    // Scale timeout: 5s base + 15s per active runner (each runner may take up to 15s to stop).
    // Runners are stopped concurrently, so we use a single 15s window, not N * 15s.
    let timeout_secs = 5 + if active_runners > 0 { 15 } else { 0 };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        #[cfg(unix)]
        {
            if !socket.exists() {
                return Ok(());
            }
        }
        #[cfg(windows)]
        {
            // On Windows, check if the pipe is no longer reachable.
            if tokio::net::windows::named_pipe::ClientOptions::new()
                .open(default_pipe_name())
                .is_err()
            {
                return Ok(());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            #[cfg(unix)]
            if socket.exists() {
                let _ = std::fs::remove_file(&socket);
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub async fn restart_daemon() -> Result<()> {
    let _ = stop_daemon().await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    start_daemon().await
}
