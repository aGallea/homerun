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
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
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
    fn test_plist_path_ends_with_plist_filename() {
        let path = plist_path().unwrap();
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), PLIST_FILENAME);
    }

    #[test]
    fn test_plist_path_is_absolute() {
        let path = plist_path().unwrap();
        assert!(path.is_absolute());
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
    fn test_build_plist_is_valid_xml_structure() {
        let daemon_path = PathBuf::from("/usr/local/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.starts_with("<?xml version=\"1.0\""));
        assert!(plist.contains("<plist version=\"1.0\">"));
        assert!(plist.contains("</plist>"));
        assert!(plist.contains("<dict>"));
        assert!(plist.contains("</dict>"));
    }

    #[test]
    fn test_build_plist_label_value() {
        let daemon_path = PathBuf::from("/tmp/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        // The label string should appear as plist string value
        assert!(plist.contains(&format!("<string>{PLIST_LABEL}</string>")));
    }

    #[test]
    fn test_build_plist_uses_provided_daemon_path() {
        let daemon_path = PathBuf::from("/custom/path/to/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("/custom/path/to/homerund"));
    }

    #[test]
    fn test_is_daemon_installed_returns_bool() {
        // Just verify it doesn't panic; actual value depends on the machine state
        let _ = is_daemon_installed();
    }

    #[test]
    fn test_is_daemon_installed_false_when_no_plist_in_temp() {
        // We can verify the function returns false when the expected plist doesn't exist
        // by checking that it matches the actual filesystem state
        let expected_path = plist_path().unwrap();
        let installed = is_daemon_installed();
        assert_eq!(installed, expected_path.exists());
    }

    #[test]
    fn test_uninstall_service_noop_when_not_installed() {
        // If not installed, uninstall_daemon_service should succeed without error
        // (it logs "nothing to uninstall" and returns Ok)
        let path = plist_path().unwrap();
        if !path.exists() {
            let result = uninstall_daemon_service();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_home_dir_str_is_non_empty() {
        let home = home_dir_str().unwrap();
        assert!(!home.is_empty());
    }

    #[test]
    fn test_home_dir_str_is_absolute_path() {
        let home = home_dir_str().unwrap();
        assert!(home.starts_with('/'), "home dir should be absolute: {home}");
    }

    #[test]
    fn test_build_plist_program_arguments_contains_path() {
        let daemon_path = PathBuf::from("/usr/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("<key>ProgramArguments</key>"));
        assert!(plist.contains("<array>"));
        assert!(plist.contains("/usr/bin/homerund"));
    }

    #[test]
    fn test_build_plist_standard_out_path() {
        let daemon_path = PathBuf::from("/usr/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("<key>StandardOutPath</key>"));
        assert!(plist.contains("daemon.log"));
    }

    #[test]
    fn test_build_plist_standard_error_path() {
        let daemon_path = PathBuf::from("/usr/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("<key>StandardErrorPath</key>"));
        assert!(plist.contains("daemon.err"));
    }

    #[test]
    fn test_build_plist_keep_alive_true() {
        let daemon_path = PathBuf::from("/usr/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<true/>"));
    }

    #[test]
    fn test_build_plist_contains_path_env() {
        let daemon_path = PathBuf::from("/usr/bin/homerund");
        let plist = build_plist(&daemon_path).unwrap();
        assert!(plist.contains("<key>EnvironmentVariables</key>"));
        assert!(plist.contains("<key>PATH</key>"));
        assert!(plist.contains("/usr/local/bin"));
        assert!(plist.contains("/opt/homebrew/bin"));
    }
}
