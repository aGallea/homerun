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
    let status = Command::new(runner_dir.join("config.sh"))
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
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("config.sh failed with exit code: {:?}", status.code());
    }
    Ok(())
}

pub async fn start_runner(runner_dir: &Path) -> Result<Child> {
    let mut cmd = Command::new(runner_dir.join("run.sh"));
    cmd.current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

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
