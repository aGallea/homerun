use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

/// Global lock to prevent concurrent downloads/extractions of the runner binary.
static DOWNLOAD_LOCK: Mutex<()> = Mutex::const_new(());

/// Written only after a runner archive has been extracted and its required
/// scripts have been verified. Its presence distinguishes a complete cache
/// from a directory left behind by an interrupted/failed extraction.
const CACHE_READY_MARKER: &str = ".homerun-cache-ready";

fn runner_script_name(os: &str, base: &str) -> String {
    if os == "win" {
        format!("{base}.cmd")
    } else {
        format!("{base}.sh")
    }
}

fn cache_is_ready(runner_dir: &Path, os: &str) -> bool {
    runner_dir.join(CACHE_READY_MARKER).is_file()
        && runner_dir.join(runner_script_name(os, "run")).is_file()
        && runner_dir.join(runner_script_name(os, "config")).is_file()
}

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
/// A cache is considered complete only after extraction has written the
/// completion marker and both required scripts are present. Interrupted caches
/// are discarded and rebuilt on the next call.
/// Returns the path to the versioned runner directory.
pub async fn ensure_runner_binary(cache_dir: &Path) -> Result<PathBuf> {
    let (os, arch) = detect_platform();
    let version = get_latest_runner_version()
        .await
        .context("Failed to determine latest runner version")?;
    let runner_dir = cache_dir.join(format!("runner-{version}"));
    fetch_and_cache(runner_dir, &version, os, arch).await
}

/// Ensures a **Linux** GitHub Actions runner binary is downloaded and
/// extracted to cache_dir, for bind-mounting into a Docker container.
///
/// Always resolves to `os = "linux"` regardless of the daemon's host OS —
/// a Docker container, even one launched from Docker Desktop on
/// macOS/Windows, runs a Linux kernel and needs the Linux runner build.
/// Cached under a distinct `runner-{version}-linux-{arch}` directory so it
/// never collides with (or reuses) the host-native cache entry.
pub async fn ensure_runner_binary_for_container(cache_dir: &Path, arch: &str) -> Result<PathBuf> {
    let version = get_latest_runner_version()
        .await
        .context("Failed to determine latest runner version")?;
    let runner_dir = cache_dir.join(format!("runner-{version}-linux-{arch}"));
    fetch_and_cache(runner_dir, &version, "linux", arch).await
}

/// Shared download/extract/cache logic for a given `(os, arch)` runner build,
/// written directly into `runner_dir` (already versioned by the caller).
async fn fetch_and_cache(
    runner_dir: PathBuf,
    version: &str,
    os: &str,
    arch: &str,
) -> Result<PathBuf> {
    let run_script_path = runner_dir.join(runner_script_name(os, "run"));
    let config_script_path = runner_dir.join(runner_script_name(os, "config"));

    // Fast path: use only a cache that was explicitly marked complete.
    if cache_is_ready(&runner_dir, os) {
        tracing::debug!("Runner binary already cached at {:?}", runner_dir);
        return Ok(runner_dir);
    }

    // Serialize concurrent downloads — only one caller extracts at a time.
    let _guard = DOWNLOAD_LOCK.lock().await;

    // Re-check after acquiring lock (another caller may have finished).
    if cache_is_ready(&runner_dir, os) {
        tracing::debug!("Runner binary already cached at {:?}", runner_dir);
        return Ok(runner_dir);
    }

    // A previous download/extraction may have died after creating `run.sh`
    // but before the archive was complete. Never layer a new extraction over
    // a directory whose completeness is unknown.
    if runner_dir.exists() {
        tracing::warn!(
            "Discarding incomplete runner cache at {:?} before re-download",
            runner_dir
        );
        tokio::fs::remove_dir_all(&runner_dir)
            .await
            .with_context(|| {
                format!("Failed to remove incomplete runner cache {:?}", runner_dir)
            })?;
    }

    tokio::fs::create_dir_all(&runner_dir)
        .await
        .with_context(|| format!("Failed to create runner directory {:?}", runner_dir))?;

    let url = runner_download_url(version, os, arch);

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

    // Branch on the archive's actual format (`ext`, derived from the *target*
    // os), not the host OS: container mode always downloads a Linux
    // `tar.gz`, even when the daemon itself runs on Windows. Windows 10
    // (1803+) ships `tar.exe` (bsdtar), so shelling out to `tar` works on
    // both platforms for the `tar.gz` case.
    if ext == "zip" {
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
    } else {
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

    tokio::fs::remove_file(&archive_path)
        .await
        .with_context(|| format!("Failed to remove runner archive {:?}", archive_path))?;

    if !run_script_path.is_file() || !config_script_path.is_file() {
        anyhow::bail!(
            "Runner archive extraction incomplete: expected {:?} and {:?}",
            run_script_path,
            config_script_path
        );
    }

    // This must be the last write in the successful extraction path. If the
    // process exits before here, the next caller will discard and rebuild the
    // incomplete directory instead of accepting it as a cache hit.
    let marker_path = runner_dir.join(CACHE_READY_MARKER);
    tokio::fs::write(&marker_path, b"ready\n")
        .await
        .with_context(|| format!("Failed to write runner cache marker {:?}", marker_path))?;

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

    #[test]
    fn test_cache_requires_completion_marker() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::write(tmp.path().join("run.sh"), "#!/bin/bash\n").unwrap();
        std::fs::write(tmp.path().join("config.sh"), "#!/bin/bash\n").unwrap();

        assert!(
            !cache_is_ready(tmp.path(), "linux"),
            "scripts alone must not make an interrupted extraction a cache hit"
        );

        std::fs::write(tmp.path().join(CACHE_READY_MARKER), "ready\n").unwrap();
        assert!(cache_is_ready(tmp.path(), "linux"));
    }

    #[test]
    fn test_cache_marker_requires_both_scripts() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::write(tmp.path().join(CACHE_READY_MARKER), "ready\n").unwrap();
        std::fs::write(tmp.path().join("run.sh"), "#!/bin/bash\n").unwrap();

        assert!(!cache_is_ready(tmp.path(), "linux"));

        std::fs::write(tmp.path().join("config.sh"), "#!/bin/bash\n").unwrap();
        assert!(cache_is_ready(tmp.path(), "linux"));
    }

    #[test]
    fn test_cache_ready_uses_target_os_script_extension() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::write(tmp.path().join(CACHE_READY_MARKER), "ready\n").unwrap();
        std::fs::write(tmp.path().join("run.sh"), "#!/bin/bash\n").unwrap();
        std::fs::write(tmp.path().join("config.sh"), "#!/bin/bash\n").unwrap();

        assert!(cache_is_ready(tmp.path(), "linux"));
        assert!(
            !cache_is_ready(tmp.path(), "win"),
            "container Linux cache must not be mistaken for a Windows cache"
        );
    }

    /// Test the cache-hit early-return building blocks without making a network call.
    #[test]
    fn test_complete_cache_is_recognized() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let runner_dir = tmp.path().join("runner-2.999.0");
        std::fs::create_dir_all(&runner_dir).expect("failed to create runner dir");
        std::fs::write(runner_dir.join("run.sh"), "#!/bin/bash\necho runner")
            .expect("failed to write run script");
        std::fs::write(runner_dir.join("config.sh"), "#!/bin/bash\necho config")
            .expect("failed to write config script");
        std::fs::write(runner_dir.join(CACHE_READY_MARKER), "ready\n")
            .expect("failed to write marker");

        assert!(cache_is_ready(&runner_dir, "linux"));
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
