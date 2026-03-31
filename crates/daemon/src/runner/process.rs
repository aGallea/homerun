use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

use crate::platform::process::{config_script, configure_process_group, run_script};
use crate::platform::shell::SHELL_PATH;

pub use crate::platform::process::{find_runner_pid, kill_orphaned_processes};

pub async fn configure_runner(
    runner_dir: &Path,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
) -> Result<()> {
    // Remove stale local config so the config script doesn't refuse to reconfigure
    for file in &[".runner", ".credentials", ".credentials_rsaparams"] {
        let path = runner_dir.join(file);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    let labels_str = labels.join(",");
    let dir_str = runner_dir.to_string_lossy().to_string();
    let script = config_script();
    let mut config_cmd = Command::new(runner_dir.join(&script));
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
            "{} failed (exit {}): {}",
            script,
            output.status.code().unwrap_or(-1),
            detail
        );
    }
    Ok(())
}

pub async fn start_runner(runner_dir: &Path) -> Result<Child> {
    let dir_str = runner_dir.to_string_lossy().to_string();
    let mut cmd = Command::new(runner_dir.join(run_script()));
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
    configure_process_group(&mut cmd);

    let child = cmd.spawn()?;
    Ok(child)
}

/// Remove stale configuration files left by a previous config script run.
/// The GitHub Actions runner checks `.runner` (or `.runner_migrated` in newer
/// versions) and refuses to configure if either exists.  This helper removes
/// all four known config files so that a subsequent config script succeeds.
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
    let script = config_script();
    let status = Command::new(runner_dir.join(&script))
        .args(["remove", "--token", token])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await?;

    if !status.success() {
        tracing::warn!(
            "{} remove failed — runner may need manual cleanup on GitHub",
            script
        );
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
        // Should fail because the config script doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_runner_fails_without_run_script() {
        let dir = tempfile::tempdir().unwrap();
        let result = start_runner(dir.path()).await;
        // On Unix, spawn fails because the script doesn't exist.
        // On Windows, cmd.exe may spawn but the script won't be found.
        // Either way, the result should indicate failure.
        assert!(
            result.is_err()
                || result
                    .unwrap()
                    .wait()
                    .await
                    .map(|s| !s.success())
                    .unwrap_or(true)
        );
    }

    #[tokio::test]
    async fn test_remove_runner_fails_without_config_script() {
        let dir = tempfile::tempdir().unwrap();
        let result = remove_runner(dir.path(), "fake-token").await;
        // On Unix, spawn fails because the script doesn't exist.
        // On Windows, the command may run but exit with failure.
        // Both are acceptable — the function handles non-success exits.
        // Just verify it doesn't panic.
        let _ = result;
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
        let run = run_script();
        let config = config_script();
        std::fs::write(dir.path().join(".runner"), "test").unwrap();
        std::fs::write(dir.path().join(&config), "#!/bin/bash").unwrap();
        std::fs::write(dir.path().join(&run), "#!/bin/bash").unwrap();
        clean_runner_config(dir.path());
        assert!(!dir.path().join(".runner").exists());
        assert!(
            dir.path().join(&config).exists(),
            "{config} should be preserved"
        );
        assert!(dir.path().join(&run).exists(), "{run} should be preserved");
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
