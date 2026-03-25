use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

/// Resolve the full PATH from the user's login shell.
/// This picks up paths added by nvm, fnm, Homebrew, etc. that aren't
/// available in a bare launchd environment.
pub fn resolve_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(&shell)
        .args(["-l", "-c", "echo $PATH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Cached shell PATH resolved once at first use.
static SHELL_PATH: std::sync::LazyLock<Option<String>> = std::sync::LazyLock::new(|| {
    let path = resolve_shell_path();
    if let Some(ref p) = path {
        tracing::info!("Resolved shell PATH: {p}");
    }
    path
});

pub async fn configure_runner(
    runner_dir: &Path,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
) -> Result<()> {
    // Remove stale local config so config.sh doesn't refuse to reconfigure
    for file in &[".runner", ".credentials", ".credentials_rsaparams"] {
        let path = runner_dir.join(file);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    let labels_str = labels.join(",");
    let dir_str = runner_dir.to_string_lossy().to_string();
    let mut config_cmd = Command::new(runner_dir.join("config.sh"));
    config_cmd
        .env("HOMERUN_RUNNER_DIR", &dir_str)
        .env("HOMERUN_MANAGED", "1");
    if let Some(ref path) = *SHELL_PATH {
        config_cmd.env("PATH", path);
    }
    let output = config_cmd
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
    // Pass the user's full shell PATH so runners can find node, docker, etc.
    if let Some(ref path) = *SHELL_PATH {
        cmd.env("PATH", path);
    }

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

/// Remove stale configuration files left by a previous `config.sh` run.
/// The GitHub Actions runner checks `.runner` (or `.runner_migrated` in newer
/// versions) and refuses to configure if either exists.  This helper removes
/// all four known config files so that a subsequent `config.sh` succeeds.
pub fn clean_runner_config(runner_dir: &Path) {
    for file_name in [
        ".runner",
        ".runner_migrated",
        ".credentials",
        ".credentials_rsaparams",
    ] {
        let path = runner_dir.join(file_name);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Failed to remove {file_name}: {e}");
            }
        }
    }
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

    #[test]
    fn test_clean_runner_config_removes_all_config_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create all four config files
        for name in [
            ".runner",
            ".runner_migrated",
            ".credentials",
            ".credentials_rsaparams",
        ] {
            std::fs::write(dir.path().join(name), "test").unwrap();
        }
        clean_runner_config(dir.path());
        for name in [
            ".runner",
            ".runner_migrated",
            ".credentials",
            ".credentials_rsaparams",
        ] {
            assert!(!dir.path().join(name).exists(), "{name} should be removed");
        }
    }

    #[test]
    fn test_clean_runner_config_removes_only_runner_migrated() {
        let dir = tempfile::tempdir().unwrap();
        // Only .runner_migrated exists (newer runner version)
        std::fs::write(dir.path().join(".runner_migrated"), "{}").unwrap();
        std::fs::write(dir.path().join(".credentials"), "cred").unwrap();
        clean_runner_config(dir.path());
        assert!(!dir.path().join(".runner_migrated").exists());
        assert!(!dir.path().join(".credentials").exists());
    }

    #[test]
    fn test_clean_runner_config_noop_when_no_config_files() {
        let dir = tempfile::tempdir().unwrap();
        // No config files — should not panic or error
        clean_runner_config(dir.path());
    }

    #[test]
    fn test_clean_runner_config_preserves_other_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".runner"), "test").unwrap();
        std::fs::write(dir.path().join("config.sh"), "#!/bin/bash").unwrap();
        std::fs::write(dir.path().join("run.sh"), "#!/bin/bash").unwrap();
        clean_runner_config(dir.path());
        assert!(!dir.path().join(".runner").exists());
        assert!(
            dir.path().join("config.sh").exists(),
            "config.sh should be preserved"
        );
        assert!(
            dir.path().join("run.sh").exists(),
            "run.sh should be preserved"
        );
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
