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
            "--url", url,
            "--token", token,
            "--name", name,
            "--labels", &labels_str,
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
    let child = Command::new(runner_dir.join("run.sh"))
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
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
        ).await;
        // Should fail because config.sh doesn't exist
        assert!(result.is_err());
    }
}
