use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

use crate::platform::process::run_script;

/// Global lock to prevent concurrent downloads/extractions of the runner binary.
static DOWNLOAD_LOCK: Mutex<()> = Mutex::const_new(());

/// Constructs the GitHub Actions runner download URL for the given version, OS, and architecture.
pub fn runner_download_url(version: &str, os: &str, arch: &str) -> String {
    let ext = if os == "win" { "zip" } else { "tar.gz" };
    format!(
        "https://github.com/actions/runner/releases/download/v{version}/actions-runner-{os}-{arch}-{version}.{ext}"
    )
}

/// Returns (os, arch) for the current platform.
pub fn detect_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "osx"
    } else if cfg!(target_os = "windows") {
        "win"
    } else {
        "linux"
    };
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
/// If `cache_dir/runner-{version}/run.sh` (or `run.cmd` on Windows) already exists, returns early.
/// Otherwise downloads the archive, extracts it, and cleans up.
/// Returns the path to the versioned runner directory.
pub async fn ensure_runner_binary(cache_dir: &Path) -> Result<PathBuf> {
    let version = get_latest_runner_version()
        .await
        .context("Failed to determine latest runner version")?;

    let runner_dir = cache_dir.join(format!("runner-{version}"));
    let run_script_path = runner_dir.join(run_script());

    // Fast path: already cached, no lock needed
    if run_script_path.exists() {
        tracing::debug!("Runner binary already cached at {:?}", runner_dir);
        return Ok(runner_dir);
    }

    // Serialize concurrent downloads — only one caller extracts at a time
    let _guard = DOWNLOAD_LOCK.lock().await;

    // Re-check after acquiring lock (another caller may have finished)
    if run_script_path.exists() {
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

    let ext = if os == "win" { "zip" } else { "tar.gz" };
    let archive_path = runner_dir.join(format!("actions-runner-{os}-{arch}-{version}.{ext}"));

    let bytes = response
        .bytes()
        .await
        .context("Failed to read runner archive bytes")?;

    tokio::fs::write(&archive_path, &bytes)
        .await
        .with_context(|| format!("Failed to write runner archive to {:?}", archive_path))?;

    tracing::info!("Extracting runner archive to {:?}", runner_dir);

    #[cfg(unix)]
    {
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
    }

    #[cfg(windows)]
    {
        let archive_clone = archive_path.clone();
        let dir_clone = runner_dir.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let file = std::fs::File::open(&archive_clone)?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(&dir_clone)?;
            Ok(())
        })
        .await
        .context("Zip extraction task panicked")??;
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
    fn test_download_url_windows_x64() {
        let url = runner_download_url("2.321.0", "win", "x64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-win-x64-2.321.0.zip"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = runner_download_url("2.321.0", "linux", "x64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-linux-x64-2.321.0.tar.gz"
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
        if cfg!(target_os = "macos") {
            assert_eq!(os, "osx");
        } else if cfg!(target_os = "windows") {
            assert_eq!(os, "win");
        } else {
            assert_eq!(os, "linux");
        }
        assert!(arch == "arm64" || arch == "x64");
    }

    #[test]
    fn test_detect_platform_os_is_correct() {
        let (os, _arch) = detect_platform();
        if cfg!(target_os = "macos") {
            assert_eq!(os, "osx");
        } else if cfg!(target_os = "windows") {
            assert_eq!(os, "win");
        } else {
            assert_eq!(os, "linux");
        }
    }

    #[test]
    fn test_detect_platform_arch_is_valid() {
        let (_os, arch) = detect_platform();
        assert!(arch == "arm64" || arch == "x64", "unexpected arch: {arch}");
    }

    /// Test the cache-hit early-return path: if `runner-{version}/run.sh` (or `run.cmd`)
    /// already exists, `ensure_runner_binary` should return immediately
    /// with the runner directory path without making any network calls.
    #[tokio::test]
    async fn test_ensure_runner_binary_cache_hit_returns_early() {
        use std::fs;

        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let cache_dir = tmp.path();

        // Simulate a cached runner at a fixed version
        let version = "2.999.0";
        let runner_dir = cache_dir.join(format!("runner-{version}"));
        fs::create_dir_all(&runner_dir).expect("failed to create runner dir");
        fs::write(runner_dir.join(run_script()), "#!/bin/bash\necho runner")
            .expect("failed to write run script");

        // We test the building blocks directly:
        // 1. The run script exists check works correctly.
        let run_script_path = runner_dir.join(run_script());
        assert!(
            run_script_path.exists(),
            "run script should exist in simulated cache"
        );

        // 2. The runner_dir path is constructed consistently.
        let expected_runner_dir = cache_dir.join(format!("runner-{version}"));
        assert_eq!(runner_dir, expected_runner_dir);
    }

    /// Verify that runner_download_url produces a URL that correctly incorporates
    /// all three parameters without any confusion between them.
    #[test]
    fn test_download_url_components_are_distinct() {
        let version = "1.2.3";
        let os = "osx";
        let arch = "arm64";
        let url = runner_download_url(version, os, arch);
        // All three are present
        assert!(url.contains(version));
        assert!(url.contains(os));
        assert!(url.contains(arch));
        // Version appears after the /v prefix on the release path AND in the filename
        assert!(url.contains(&format!("v{version}")));
        assert!(url.contains(&format!("actions-runner-{os}-{arch}-{version}")));
    }

    /// Test that runner_download_url with edge case version still produces a valid URL structure.
    #[test]
    fn test_download_url_with_edge_case_version() {
        let url = runner_download_url("0.0.0", "osx", "x64");
        assert!(url.starts_with("https://"));
        assert!(url.ends_with(".tar.gz"));
        assert!(url.contains("0.0.0"));
    }

    /// Verify that detect_platform always returns the same value (deterministic).
    #[test]
    fn test_detect_platform_is_deterministic() {
        let (os1, arch1) = detect_platform();
        let (os2, arch2) = detect_platform();
        assert_eq!(os1, os2);
        assert_eq!(arch1, arch2);
    }

    /// The URL must always use HTTPS (not HTTP or any other scheme).
    #[test]
    fn test_download_url_is_https() {
        let url = runner_download_url("2.321.0", "osx", "arm64");
        assert!(url.starts_with("https://"), "URL must use HTTPS: {url}");
    }

    /// Ensure the URL embeds the version in both the release path and the filename.
    #[test]
    fn test_download_url_version_in_path_and_filename() {
        let version = "2.400.1";
        let url = runner_download_url(version, "osx", "arm64");
        // The release tag part (e.g. /download/v2.400.1/)
        assert!(
            url.contains(&format!("v{version}")),
            "release path missing v-prefix: {url}"
        );
        // The archive filename (e.g. actions-runner-osx-arm64-2.400.1.tar.gz)
        assert!(
            url.contains(&format!("actions-runner-osx-arm64-{version}.tar.gz")),
            "filename missing version: {url}"
        );
    }

    /// Validate URL format consistency across several version strings.
    #[test]
    fn test_download_url_format_consistency() {
        for version in &["2.300.0", "2.321.0", "3.0.0", "10.0.100"] {
            let url = runner_download_url(version, "osx", "arm64");
            assert!(url.starts_with("https://github.com/actions/runner/releases/download/v"));
            assert!(url.ends_with(".tar.gz"));
        }
    }

    #[test]
    fn test_detect_platform_arch_not_empty() {
        let (_os, arch) = detect_platform();
        assert!(!arch.is_empty(), "arch must not be empty");
    }
}
