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

fn build_plist(daemon_path: &Path) -> Result<String> {
    let home = home_dir_str()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_plist_path_contains_launch_agents() {
        let path = plist_path().unwrap();
        assert!(path.to_string_lossy().contains("LaunchAgents"));
        assert!(path.to_string_lossy().contains(PLIST_FILENAME));
    }

    #[test]
    fn test_build_plist_contains_label_and_path() {
        let daemon_path = PathBuf::from("/usr/local/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains(PLIST_LABEL));
        assert!(plist.contains("/usr/local/bin/homerund"));
        assert!(plist.contains("RunAtLoad"));
        assert!(plist.contains("KeepAlive"));
        assert!(plist.contains("daemon.log"));
        assert!(plist.contains("daemon.err"));
    }

    #[test]
    fn test_build_plist_contains_home_dir() {
        let daemon_path = PathBuf::from("/usr/local/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        let home = dirs::home_dir().unwrap();
        assert!(plist.contains(home.to_string_lossy().as_ref()));
    }

    #[test]
    fn test_is_daemon_installed_returns_bool() {
        // Just verify it doesn't panic; actual value depends on the machine state
        let _ = is_daemon_installed();
    }
}
