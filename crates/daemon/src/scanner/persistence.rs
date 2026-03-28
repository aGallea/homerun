use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::DiscoveredRepo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResults {
    pub last_scan_at: DateTime<Utc>,
    pub local_results: Vec<DiscoveredRepo>,
    pub remote_results: Vec<DiscoveredRepo>,
    pub merged_results: Vec<DiscoveredRepo>,
}

pub async fn save_scan_results(path: &Path, results: &ScanResults) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    tokio::fs::write(path, json).await?;
    Ok(())
}

pub async fn load_scan_results(path: &Path) -> Result<Option<ScanResults>> {
    match tokio::fs::read_to_string(path).await {
        Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::DiscoverySource;

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan-results.json");

        let results = ScanResults {
            last_scan_at: Utc::now(),
            local_results: vec![DiscoveredRepo {
                full_name: "acme/api".to_string(),
                source: DiscoverySource::Local,
                workflow_files: vec!["ci.yml".to_string()],
                matched_labels: vec!["self-hosted".to_string()],
                local_path: None,
            }],
            remote_results: vec![],
            merged_results: vec![DiscoveredRepo {
                full_name: "acme/api".to_string(),
                source: DiscoverySource::Local,
                workflow_files: vec!["ci.yml".to_string()],
                matched_labels: vec!["self-hosted".to_string()],
                local_path: None,
            }],
        };

        save_scan_results(&path, &results).await.unwrap();
        let loaded = load_scan_results(&path).await.unwrap().unwrap();
        assert_eq!(loaded.merged_results.len(), 1);
        assert_eq!(loaded.merged_results[0].full_name, "acme/api");
    }

    #[tokio::test]
    async fn test_load_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let result = load_scan_results(&path).await.unwrap();
        assert!(result.is_none());
    }
}
