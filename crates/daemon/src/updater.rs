use anyhow::Result;
use std::path::Path;

const VERSION_CACHE_FILE: &str = "cached_runner_version.txt";

/// Reads the locally cached runner version from `cache_dir/cached_runner_version.txt`.
/// Returns `None` if the file does not exist or cannot be read.
pub fn read_cached_version(cache_dir: &Path) -> Option<String> {
    let path = cache_dir.join(VERSION_CACHE_FILE);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Writes `version` to `cache_dir/cached_runner_version.txt`.
pub fn write_cached_version(cache_dir: &Path, version: &str) -> Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(cache_dir.join(VERSION_CACHE_FILE), version)?;
    Ok(())
}

/// Fetches the latest GitHub Actions runner version from the GitHub releases API.
/// Returns the version string without the "v" prefix (e.g., "2.322.0").
pub async fn fetch_latest_version() -> Result<String> {
    crate::runner::binary::get_latest_runner_version().await
}

fn parse_runner_version(version: &str) -> Option<Vec<u64>> {
    let trimmed = version.trim().trim_start_matches('v');
    let parsed: Option<Vec<u64>> = trimmed
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect();
    parsed.filter(|parts| !parts.is_empty())
}

pub fn is_newer_runner_version(current: &str, latest: &str) -> bool {
    match (parse_runner_version(current), parse_runner_version(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

/// Checks whether a newer GitHub Actions runner version is available.
///
/// Compares the version recorded in `cache_dir/cached_runner_version.txt`
/// with the latest version from GitHub's releases API.
///
/// Returns `Some(latest_version)` when an update is available,
/// or `None` when already up-to-date or the cached version is unknown.
pub async fn check_for_update(cache_dir: &Path) -> Option<String> {
    let current = read_cached_version(cache_dir)?;
    let latest = fetch_latest_version().await.ok()?;
    if is_newer_runner_version(&current, &latest) {
        Some(latest)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_version_comparison_only_flags_newer_versions() {
        assert!(is_newer_runner_version("2.321.0", "2.322.0"));
        assert!(!is_newer_runner_version("2.322.0", "2.322.0"));
        assert!(!is_newer_runner_version("2.323.0", "2.322.0"));
        assert!(is_newer_runner_version("v2.321.9", "2.322.0"));
        assert!(!is_newer_runner_version("not-a-version", "2.322.0"));
    }

    #[test]
    fn test_read_cached_version_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_cached_version(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_write_and_read_cached_version() {
        let dir = tempfile::tempdir().unwrap();
        write_cached_version(dir.path(), "2.321.0").unwrap();
        let version = read_cached_version(dir.path());
        assert_eq!(version, Some("2.321.0".to_string()));
    }

    #[test]
    fn test_write_cached_version_trims_on_read() {
        let dir = tempfile::tempdir().unwrap();
        // Write with trailing newline (common on Unix)
        std::fs::write(dir.path().join(VERSION_CACHE_FILE), "2.321.0\n").unwrap();
        let version = read_cached_version(dir.path());
        assert_eq!(version, Some("2.321.0".to_string()));
    }

    #[test]
    fn test_read_cached_version_empty_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(VERSION_CACHE_FILE), "").unwrap();
        let result = read_cached_version(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_overwrite_cached_version() {
        let dir = tempfile::tempdir().unwrap();
        write_cached_version(dir.path(), "2.321.0").unwrap();
        write_cached_version(dir.path(), "2.322.0").unwrap();
        let version = read_cached_version(dir.path());
        assert_eq!(version, Some("2.322.0".to_string()));
    }
}
