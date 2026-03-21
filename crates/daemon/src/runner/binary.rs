use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Constructs the GitHub Actions runner download URL for the given version, OS, and architecture.
pub fn runner_download_url(version: &str, os: &str, arch: &str) -> String {
    format!(
        "https://github.com/actions/runner/releases/download/v{version}/actions-runner-{os}-{arch}-{version}.tar.gz"
    )
}

/// Returns (os, arch) for the current platform.
/// OS is always "osx" (macOS only for now).
/// Arch is "arm64" for aarch64, otherwise "x64".
pub fn detect_platform() -> (&'static str, &'static str) {
    let os = "osx";
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    (os, arch)
}

/// Fetches the latest GitHub Actions runner version from the GitHub releases API.
/// Returns the version string without the "v" prefix (e.g., "2.321.0").
pub async fn get_latest_runner_version() -> Result<String> {
    let octocrab = octocrab::instance();
    let release = octocrab
        .repos("actions", "runner")
        .releases()
        .get_latest()
        .await
        .context("Failed to fetch latest runner release from GitHub")?;

    let tag = release.tag_name;
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    Ok(version)
}

/// Ensures the GitHub Actions runner binary is downloaded and extracted to cache_dir.
/// If `cache_dir/runner-{version}/run.sh` already exists, returns early.
/// Otherwise downloads the tar.gz, extracts it, and cleans up the archive.
/// Returns the path to the versioned runner directory.
pub async fn ensure_runner_binary(cache_dir: &Path) -> Result<PathBuf> {
    let version = get_latest_runner_version()
        .await
        .context("Failed to determine latest runner version")?;

    let runner_dir = cache_dir.join(format!("runner-{version}"));
    let run_sh = runner_dir.join("run.sh");

    if run_sh.exists() {
        tracing::debug!("Runner binary already cached at {:?}", runner_dir);
        return Ok(runner_dir);
    }

    tokio::fs::create_dir_all(&runner_dir)
        .await
        .with_context(|| format!("Failed to create runner directory {:?}", runner_dir))?;

    let (os, arch) = detect_platform();
    let url = runner_download_url(&version, os, arch);

    tracing::info!("Downloading runner from {}", url);

    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("Failed to download runner from {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download runner: HTTP {}", response.status());
    }

    let archive_path = runner_dir.join(format!("actions-runner-{os}-{arch}-{version}.tar.gz"));

    let bytes = response
        .bytes()
        .await
        .context("Failed to read runner archive bytes")?;

    tokio::fs::write(&archive_path, &bytes)
        .await
        .with_context(|| format!("Failed to write runner archive to {:?}", archive_path))?;

    tracing::info!("Extracting runner archive to {:?}", runner_dir);

    let status = tokio::process::Command::new("tar")
        .arg("xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&runner_dir)
        .status()
        .await
        .context("Failed to run tar to extract runner archive")?;

    if !status.success() {
        anyhow::bail!("tar extraction failed with status: {}", status);
    }

    tokio::fs::remove_file(&archive_path)
        .await
        .with_context(|| format!("Failed to remove runner archive {:?}", archive_path))?;

    tracing::info!("Runner binary ready at {:?}", runner_dir);

    Ok(runner_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_url_macos_arm64() {
        let url = runner_download_url("2.321.0", "osx", "arm64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-osx-arm64-2.321.0.tar.gz"
        );
    }

    #[test]
    fn test_download_url_macos_x64() {
        let url = runner_download_url("2.321.0", "osx", "x64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-osx-x64-2.321.0.tar.gz"
        );
    }

    #[test]
    fn test_download_url_different_version() {
        let url = runner_download_url("2.300.0", "osx", "arm64");
        assert!(url.contains("v2.300.0"));
        assert!(url.contains("arm64"));
        assert!(url.ends_with(".tar.gz"));
    }

    #[test]
    fn test_download_url_contains_github_actions_runner() {
        let url = runner_download_url("2.321.0", "osx", "arm64");
        assert!(url.starts_with("https://github.com/actions/runner/releases/download/"));
    }

    #[test]
    fn test_detect_platform() {
        let (os, arch) = detect_platform();
        assert_eq!(os, "osx");
        assert!(arch == "arm64" || arch == "x64");
    }

    #[test]
    fn test_detect_platform_os_is_always_osx() {
        // HomeRun only supports macOS
        let (os, _arch) = detect_platform();
        assert_eq!(os, "osx");
    }

    #[test]
    fn test_detect_platform_arch_is_valid() {
        let (_os, arch) = detect_platform();
        assert!(
            arch == "arm64" || arch == "x64",
            "unexpected arch: {arch}"
        );
    }
}
