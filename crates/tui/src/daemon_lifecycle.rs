use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::DaemonClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownErrorDisposition {
    ServiceManaged,
    AlreadyStopped,
    Fatal,
}

fn classify_shutdown_error(message: &str, daemon_healthy: bool) -> ShutdownErrorDisposition {
    if message.contains("launchd")
        || message.contains("Uninstall the service")
        || message.contains("auto-start service")
        || message.contains("system service")
    {
        ShutdownErrorDisposition::ServiceManaged
    } else if daemon_healthy {
        ShutdownErrorDisposition::Fatal
    } else {
        ShutdownErrorDisposition::AlreadyStopped
    }
}

#[cfg(unix)]
fn configure_spawned_daemon(command: &mut Command) {
    // Give the daemon its own process group so failed startup cleanup can
    // terminate any runner children it spawned before becoming healthy.
    command.process_group(0);
}

#[cfg(windows)]
fn configure_spawned_daemon(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_daemon_process_tree(child: &mut Child) -> Result<()> {
    let pid = child.id() as i32;
    // The daemon was spawned in its own process group, so a negative PID targets
    // the whole startup tree instead of only the homerund parent process.
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let result = unsafe { kill(-pid, 9) };
    if result != 0
        && child
            .try_wait()
            .context("Failed to inspect homerund after process-group cleanup")?
            .is_none()
    {
        return Err(std::io::Error::last_os_error())
            .context("Failed to terminate timed-out homerund process group");
    }
    Ok(())
}

#[cfg(windows)]
fn terminate_daemon_process_tree(child: &mut Child) -> Result<()> {
    let pid = child.id().to_string();
    let status = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to invoke taskkill for timed-out homerund process tree")?;
    if !status.success()
        && child
            .try_wait()
            .context("Failed to inspect homerund after taskkill")?
            .is_none()
    {
        bail!("taskkill failed to terminate timed-out homerund process tree");
    }
    Ok(())
}

fn terminate_spawned_daemon(child: &mut Child) -> Result<()> {
    if child
        .try_wait()
        .context("Failed to inspect spawned homerund process")?
        .is_some()
    {
        return Ok(());
    }
    terminate_daemon_process_tree(child)?;
    child
        .wait()
        .context("Failed to reap timed-out homerund process")?;
    Ok(())
}

#[cfg(unix)]
fn socket_path_from_home(home: Option<PathBuf>) -> Result<PathBuf> {
    home.map(|path| path.join(".homerun/daemon.sock"))
        .context("Cannot determine home directory for HomeRun daemon socket")
}

#[cfg(unix)]
fn default_socket_path() -> Result<PathBuf> {
    socket_path_from_home(dirs::home_dir())
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
    let socket = default_socket_path()?;

    #[cfg(unix)]
    {
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
    let mut command = Command::new(&binary);
    configure_spawned_daemon(&mut command);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn homerund")?;

    #[cfg(unix)]
    let client = DaemonClient::new(socket);
    #[cfg(windows)]
    let client = DaemonClient::new_pipe(default_pipe_name());

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if client.health().await.is_ok() {
            // The daemon now owns its own lifetime. Dropping the Child handle is
            // intentional here; only failed starts are cleaned up by this CLI.
            return Ok(());
        }

        if let Some(status) = child
            .try_wait()
            .context("Failed to inspect spawned homerund process")?
        {
            bail!(
                "Daemon exited before becoming healthy ({status}) — check logs at ~/.homerun/logs/"
            );
        }

        if tokio::time::Instant::now() >= deadline {
            if let Err(cleanup_error) = terminate_spawned_daemon(&mut child) {
                bail!(
                    "Daemon failed to start within 5 seconds and cleanup failed: {cleanup_error:#}"
                );
            }
            bail!("Daemon failed to start within 5 seconds — check logs at ~/.homerun/logs/");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub async fn stop_daemon() -> Result<()> {
    #[cfg(unix)]
    let socket = default_socket_path()?;

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
        Err(error) => {
            let message = error.to_string();
            let healthy = client.health().await.is_ok();
            match classify_shutdown_error(&message, healthy) {
                ShutdownErrorDisposition::ServiceManaged => {
                    #[cfg(target_os = "macos")]
                    bail!(
                        "Daemon is managed by launchd. Disable Launch at login first \
                         (Settings > Startup) or run: launchctl unload ~/Library/LaunchAgents/com.homerun.daemon.plist"
                    );
                    #[cfg(windows)]
                    bail!(
                        "Daemon is managed by Windows autostart. Disable Launch at login first \
                         (Settings > Startup)."
                    );
                    #[cfg(all(unix, not(target_os = "macos")))]
                    bail!(
                        "Daemon is managed by a system service. Disable that service before stopping it directly."
                    );
                }
                ShutdownErrorDisposition::AlreadyStopped => {
                    #[cfg(unix)]
                    if socket.exists() {
                        std::fs::remove_file(&socket)?;
                    }
                    return Ok(());
                }
                ShutdownErrorDisposition::Fatal => {
                    bail!("Failed to stop daemon: {message}");
                }
            }
        }
    };

    let timeout_secs = 5 + if active_runners > 0 { 15 } else { 0 };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        #[cfg(unix)]
        if !socket.exists() {
            return Ok(());
        }
        #[cfg(windows)]
        if !is_daemon_running().await {
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            let healthy = client.health().await.is_ok();
            if healthy {
                bail!("Daemon did not shut down in time and is still responding");
            }
            #[cfg(unix)]
            if socket.exists() {
                std::fs::remove_file(&socket)?;
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub async fn restart_daemon() -> Result<()> {
    #[cfg(unix)]
    if is_daemon_running(&default_socket_path()?).await {
        stop_daemon().await?;
    }
    #[cfg(windows)]
    if is_daemon_running().await {
        stop_daemon().await?;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    start_daemon().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_error_classification_is_fail_closed() {
        assert_eq!(
            classify_shutdown_error("connection refused", false),
            ShutdownErrorDisposition::AlreadyStopped
        );
        assert_eq!(
            classify_shutdown_error("transport reset", true),
            ShutdownErrorDisposition::Fatal
        );
        assert_eq!(
            classify_shutdown_error("Daemon is installed as a system service", true),
            ShutdownErrorDisposition::ServiceManaged
        );
    }

    #[test]
    fn cleanup_accepts_already_exited_child() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.wait().unwrap();
        terminate_spawned_daemon(&mut child).unwrap();
    }

    #[test]
    fn cleanup_terminates_running_child() {
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 30"]);
            configure_spawned_daemon(&mut command);
            command
        };
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("cmd");
            command.args(["/C", "ping -n 30 127.0.0.1 >NUL"]);
            configure_spawned_daemon(&mut command);
            command
        };
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        terminate_spawned_daemon(&mut child).unwrap();
        assert!(child.try_wait().unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn socket_path_requires_home_directory() {
        assert!(socket_path_from_home(None).is_err());
        assert_eq!(
            socket_path_from_home(Some(PathBuf::from("/tmp/homerun-home"))).unwrap(),
            PathBuf::from("/tmp/homerun-home/.homerun/daemon.sock")
        );
    }
}
