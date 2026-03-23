use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

pub async fn configure_runner(
    runner_dir: &Path,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
) -> Result<()> {
    let labels_str = labels.join(",");
    let dir_str = runner_dir.to_string_lossy().to_string();
    let output = Command::new(runner_dir.join("config.sh"))
        .env("HOMERUN_RUNNER_DIR", &dir_str)
        .env("HOMERUN_MANAGED", "1")
        .args([
            "--url",
            url,
            "--token",
            token,
            "--name",
            name,
            "--labels",
            &labels_str,
            "--unattended",
            "--replace",
        ])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        anyhow::bail!(
            "config.sh failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            detail
        );
    }
    Ok(())
}

/// Kill any orphaned runner processes from a previous daemon session.
/// Uses `pgrep` to find processes whose command line contains the runner_dir path,
/// then kills the entire process group for each match and waits for them to die.
pub async fn kill_orphaned_processes(runner_dir: &Path) {
    let dir_str = runner_dir.to_string_lossy().to_string();

    let pids = find_runner_pids(&dir_str).await;
    if pids.is_empty() {
        return;
    }

    tracing::info!(
        "Killing {} orphaned process(es) for runner dir {}",
        pids.len(),
        dir_str
    );

    // SIGTERM the process groups for graceful shutdown
    for pid in &pids {
        unsafe {
            libc::kill(-*pid, libc::SIGTERM);
            libc::kill(*pid, libc::SIGTERM);
        }
    }

    // Wait up to 5s for processes to die, checking every 500ms
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if find_runner_pids(&dir_str).await.is_empty() {
            tracing::info!("Orphaned processes terminated cleanly");
            return;
        }
    }

    // Force-kill any stragglers
    let remaining = find_runner_pids(&dir_str).await;
    if !remaining.is_empty() {
        tracing::warn!("Force-killing {} remaining process(es)", remaining.len());
        for pid in &remaining {
            unsafe {
                libc::kill(-*pid, libc::SIGKILL);
                libc::kill(*pid, libc::SIGKILL);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn find_runner_pids(dir_str: &str) -> Vec<i32> {
    let output = Command::new("pgrep")
        .args(["-f", dir_str])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<i32>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// Find the session-leader PID (run.sh) for a runner directory, if still alive.
pub async fn find_runner_pid(runner_dir: &Path) -> Option<i32> {
    let dir_str = runner_dir.to_string_lossy();
    // pgrep -f matches command line; filter to the run.sh session leader
    let output = Command::new("pgrep")
        .args(["-f", &format!("{}/run.sh", dir_str)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<i32>().ok())
        .next()
}

pub async fn start_runner(runner_dir: &Path) -> Result<Child> {
    let dir_str = runner_dir.to_string_lossy().to_string();
    let mut cmd = Command::new(runner_dir.join("run.sh"));
    cmd.current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Tag all child processes so we can always find them
        .env("HOMERUN_RUNNER_DIR", &dir_str)
        .env("HOMERUN_MANAGED", "1");

    // Spawn in its own process group so we can signal the entire tree
    // (run.sh spawns child .NET processes that hold the GitHub session).
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let child = cmd.spawn()?;
    Ok(child)
}

pub async fn remove_runner(runner_dir: &Path, token: &str) -> Result<()> {
    let status = Command::new(runner_dir.join("config.sh"))
        .args(["remove", "--token", token])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await?;

    if !status.success() {
        tracing::warn!("config.sh remove failed — runner may need manual cleanup on GitHub");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_configure_runner_fails_without_script() {
        let dir = tempfile::tempdir().unwrap();
        let result = configure_runner(
            dir.path(),
            "https://github.com/test/repo",
            "fake-token",
            "test-runner",
            &["self-hosted".to_string()],
        )
        .await;
        // Should fail because config.sh doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_runner_fails_without_run_sh() {
        let dir = tempfile::tempdir().unwrap();
        let result = start_runner(dir.path()).await;
        // Should fail because run.sh doesn't exist in the temp dir
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_runner_fails_without_config_sh() {
        let dir = tempfile::tempdir().unwrap();
        let result = remove_runner(dir.path(), "fake-token").await;
        // Should fail because config.sh doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_configure_runner_with_multiple_labels() {
        let dir = tempfile::tempdir().unwrap();
        let result = configure_runner(
            dir.path(),
            "https://github.com/owner/repo",
            "token123",
            "runner-name",
            &[
                "self-hosted".to_string(),
                "macOS".to_string(),
                "arm64".to_string(),
            ],
        )
        .await;
        // Still fails without script, but we verify the call compiles and runs
        assert!(result.is_err());
    }
}
