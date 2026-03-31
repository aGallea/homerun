#[cfg(target_os = "macos")]
mod macos {
    use anyhow::{Context, Result};
    use std::path::{Path, PathBuf};

    const PLIST_LABEL: &str = "com.homerun.daemon";
    const PLIST_FILENAME: &str = "com.homerun.daemon.plist";

    fn plist_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join("Library/LaunchAgents").join(PLIST_FILENAME))
    }

    fn home_dir_str() -> Result<String> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.display().to_string())
    }

    fn resolve_shell_path() -> String {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .args(["-l", "-c", "echo $PATH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
        // Fallback to a sensible default
        "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
    }

    fn build_plist(daemon_path: &Path) -> Result<String> {
        let home = home_dir_str()?;
        let path = resolve_shell_path();
        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{home}/logs/daemon.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/logs/daemon.err</string>
</dict>
</plist>"#,
            daemon_path.display(),
        ))
    }

    /// Install the HomeRun daemon as a launchd LaunchAgent so it starts on login.
    /// Writes the plist to ~/Library/LaunchAgents/com.homerun.daemon.plist and
    /// loads it with `launchctl load`.
    pub fn install_daemon_service(daemon_path: &Path) -> Result<()> {
        let plist_path = plist_path()?;

        // Ensure parent directory exists
        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create LaunchAgents directory: {}",
                    parent.display()
                )
            })?;
        }

        let plist = build_plist(daemon_path)?;
        std::fs::write(&plist_path, &plist)
            .with_context(|| format!("Failed to write plist to {}", plist_path.display()))?;

        tracing::info!("Wrote launchd plist to {}", plist_path.display());

        let status = std::process::Command::new("launchctl")
            .arg("load")
            .arg("-w")
            .arg(&plist_path)
            .status()
            .context("Failed to run launchctl load")?;

        if !status.success() {
            anyhow::bail!("launchctl load failed with exit code: {}", status);
        }

        tracing::info!("Daemon service installed and loaded via launchd");
        Ok(())
    }

    /// Unload and remove the HomeRun daemon launchd plist.
    pub fn uninstall_daemon_service() -> Result<()> {
        let plist_path = plist_path()?;

        if plist_path.exists() {
            let status = std::process::Command::new("launchctl")
                .arg("unload")
                .arg("-w")
                .arg(&plist_path)
                .status()
                .context("Failed to run launchctl unload")?;

            if !status.success() {
                // Log but don't fail — plist may already be unloaded
                tracing::warn!("launchctl unload exited with: {}", status);
            }

            std::fs::remove_file(&plist_path)
                .with_context(|| format!("Failed to remove plist at {}", plist_path.display()))?;

            tracing::info!("Daemon service uninstalled");
        } else {
            tracing::info!(
                "No plist found at {} — nothing to uninstall",
                plist_path.display()
            );
        }

        Ok(())
    }

    /// Returns true if the launchd plist is installed at the expected location.
    pub fn is_daemon_installed() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }
}

#[cfg(windows)]
mod windows {
    use anyhow::{Context, Result};
    use std::path::Path;

    const TASK_NAME: &str = "HomeRun Daemon";

    /// Install the HomeRun daemon as a Windows Task Scheduler task that runs on logon.
    pub fn install_daemon_service(daemon_path: &Path) -> Result<()> {
        let daemon_str = daemon_path.display().to_string();
        let tr_arg = format!("\"{}\"", daemon_str);

        let status = std::process::Command::new("schtasks")
            .args([
                "/Create", "/SC", "ONLOGON", "/TN", TASK_NAME, "/TR", &tr_arg, "/RL", "HIGHEST",
                "/F",
            ])
            .status()
            .context("Failed to run schtasks /Create")?;

        if !status.success() {
            anyhow::bail!("schtasks /Create failed with exit code: {}", status);
        }

        tracing::info!("Daemon service installed via Task Scheduler");
        Ok(())
    }

    /// Remove the HomeRun daemon from Windows Task Scheduler.
    pub fn uninstall_daemon_service() -> Result<()> {
        let status = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .status()
            .context("Failed to run schtasks /Delete")?;

        if !status.success() {
            anyhow::bail!("schtasks /Delete failed with exit code: {}", status);
        }

        tracing::info!("Daemon service uninstalled from Task Scheduler");
        Ok(())
    }

    /// Returns true if the HomeRun daemon task exists in Task Scheduler.
    pub fn is_daemon_installed() -> bool {
        std::process::Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod linux_stub {
    use anyhow::Result;
    use std::path::Path;

    pub fn install_daemon_service(_daemon_path: &Path) -> Result<()> {
        anyhow::bail!("Auto-start is not yet supported on Linux")
    }

    pub fn uninstall_daemon_service() -> Result<()> {
        anyhow::bail!("Auto-start is not yet supported on Linux")
    }

    pub fn is_daemon_installed() -> bool {
        false
    }
}

#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;
#[cfg(all(unix, not(target_os = "macos")))]
pub use linux_stub::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_daemon_installed_returns_bool() {
        // Just verify it doesn't panic; actual value depends on the machine state
        let _result: bool = is_daemon_installed();
    }
}
