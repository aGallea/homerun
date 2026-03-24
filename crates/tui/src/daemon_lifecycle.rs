use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::client::DaemonClient;

fn default_socket_path() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join(".homerun/daemon.sock")
}

async fn is_daemon_running(socket: &Path) -> bool {
    if !socket.exists() {
        return false;
    }
    let client = DaemonClient::new(socket.to_path_buf());
    client.health().await.is_ok()
}

pub async fn start_daemon() -> Result<()> {
    let socket = default_socket_path();
    if is_daemon_running(&socket).await {
        bail!("Daemon is already running");
    }
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }
    let binary = which::which("homerund")
        .context("homerund not found in PATH. Install it or add it to your PATH.")?;
    std::process::Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn homerund")?;
    let client = DaemonClient::new(socket.clone());
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
    let socket = default_socket_path();
    if !socket.exists() {
        bail!("Daemon is not running (no socket file)");
    }
    let client = DaemonClient::new(socket.clone());
    match client.shutdown().await {
        Ok(()) => {}
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("launchd") || msg.contains("Uninstall the service") {
                bail!(
                    "Daemon is managed by launchd. Uninstall the service first \
                     (Settings > Startup) or run: launchctl unload ~/Library/LaunchAgents/com.homerun.daemon.plist"
                );
            }
            if socket.exists() {
                std::fs::remove_file(&socket)?;
            }
            return Ok(());
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if !socket.exists() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
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
