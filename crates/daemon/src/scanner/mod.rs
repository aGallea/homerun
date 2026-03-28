pub mod persistence;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;

use tokio_util::sync::CancellationToken;

use crate::github::GitHubClient;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredRepo {
    pub full_name: String,
    pub source: DiscoverySource,
    /// Workflow files (relative paths) that contain a matching `runs-on:` label
    pub workflow_files: Vec<String>,
    /// Labels that were found in the workflow files
    pub matched_labels: Vec<String>,
    pub local_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoverySource {
    Local,
    Remote,
    Both,
}

/// Scan progress event emitted during scanning.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ScanProgressEvent {
    Started {
        scan_type: String,
        total: usize,
    },
    Checking {
        repo: String,
        index: usize,
        total: usize,
    },
    Found {
        #[serde(flatten)]
        repo: DiscoveredRepo,
    },
    Done {
        scan_type: String,
        total_found: usize,
        total_checked: usize,
    },
    Cancelled {
        scan_type: String,
        checked: usize,
        total: usize,
    },
}

// ---------------------------------------------------------------------------
// Local scan
// ---------------------------------------------------------------------------

/// Recursively walk `workspace_dir`, find all `.github/workflows/*.yml` files,
/// and return repos that have at least one workflow with a matching `runs-on:` label.
pub async fn scan_local(workspace_dir: &Path, labels: &[String]) -> Result<Vec<DiscoveredRepo>> {
    // Map: repo_root -> (full_name, matching_workflow_files, matched_labels)
    let mut found: HashMap<PathBuf, (String, Vec<String>, Vec<String>)> = HashMap::new();

    collect_workflow_files(workspace_dir, labels, &mut found).await?;

    let mut results: Vec<DiscoveredRepo> = found
        .into_iter()
        .map(
            |(root, (full_name, workflow_files, matched_labels))| DiscoveredRepo {
                full_name,
                source: DiscoverySource::Local,
                workflow_files,
                matched_labels,
                local_path: Some(root),
            },
        )
        .collect();

    // Stable ordering for tests and display
    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
}

/// Recurse into `dir`. When we find a `.github/workflows` directory, check
/// whether its parent is a git repo and inspect the workflow files.
async fn collect_workflow_files(
    dir: &Path,
    labels: &[String],
    found: &mut HashMap<PathBuf, (String, Vec<String>, Vec<String>)>,
) -> Result<()> {
    let mut read_dir = match fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(()), // skip unreadable dirs
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden directories other than `.github`
        if name_str.starts_with('.') && name_str != ".github" {
            continue;
        }

        if name_str == ".github" {
            // Look for workflows subdirectory
            let workflows_dir = path.join("workflows");
            if workflows_dir.is_dir() {
                // The repo root is the parent of .github
                if let Some(repo_root) = path.parent() {
                    process_workflows_dir(repo_root, &workflows_dir, labels, found).await;
                }
            }
            // Don't recurse into .github itself
            continue;
        }

        // Recurse
        Box::pin(collect_workflow_files(&path, labels, found)).await?;
    }

    Ok(())
}

async fn process_workflows_dir(
    repo_root: &Path,
    workflows_dir: &Path,
    labels: &[String],
    found: &mut HashMap<PathBuf, (String, Vec<String>, Vec<String>)>,
) {
    let mut rd = match fs::read_dir(workflows_dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut matching_files: Vec<String> = Vec::new();
    let mut matched_labels: Vec<String> = Vec::new();

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yml" && ext != "yaml" {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path).await {
            let file_matched = labels
                .iter()
                .any(|label| content.contains(&format!("runs-on: {}", label)));
            if file_matched {
                let rel = path
                    .strip_prefix(repo_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                matching_files.push(rel);
                for label in labels {
                    if content.contains(&format!("runs-on: {}", label))
                        && !matched_labels.contains(label)
                    {
                        matched_labels.push(label.clone());
                    }
                }
            }
        }
    }

    if matching_files.is_empty() {
        return;
    }

    matching_files.sort();
    matched_labels.sort();

    // Determine the repo full name from the git remote URL
    let full_name = git_remote_full_name(repo_root).await.unwrap_or_else(|| {
        repo_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });

    let entry = found
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| (full_name.clone(), Vec::new(), Vec::new()));
    // If we already visited this root (shouldn't happen but be safe), merge files
    for f in matching_files {
        if !entry.1.contains(&f) {
            entry.1.push(f);
        }
    }
    entry.1.sort();
    for l in matched_labels {
        if !entry.2.contains(&l) {
            entry.2.push(l);
        }
    }
    entry.2.sort();
}

// ---------------------------------------------------------------------------
// Two-phase local scan with progress
// ---------------------------------------------------------------------------

/// Phase 1: Find all repos that have a `.github/workflows/` directory, without
/// checking workflow file contents. Returns `(full_name, repo_root, workflows_dir)` tuples.
pub async fn discover_local_repos(workspace_dir: &Path) -> Result<Vec<(String, PathBuf, PathBuf)>> {
    let mut repos = Vec::new();
    discover_repos_recursive(workspace_dir, &mut repos).await?;
    repos.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(repos)
}

async fn discover_repos_recursive(
    dir: &Path,
    repos: &mut Vec<(String, PathBuf, PathBuf)>,
) -> Result<()> {
    let mut read_dir = match fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') && name_str != ".github" {
            continue;
        }

        if name_str == ".github" {
            let workflows_dir = path.join("workflows");
            if workflows_dir.is_dir() {
                if let Some(repo_root) = path.parent() {
                    let full_name = git_remote_full_name(repo_root).await.unwrap_or_else(|| {
                        repo_root
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    });
                    repos.push((full_name, repo_root.to_path_buf(), workflows_dir));
                }
            }
            continue;
        }

        Box::pin(discover_repos_recursive(&path, repos)).await?;
    }
    Ok(())
}

/// Phase 2 helper: Check a single repo's workflow files for matching labels.
async fn check_workflows_dir(
    workflows_dir: &Path,
    repo_root: &Path,
    labels: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut rd = match fs::read_dir(workflows_dir).await {
        Ok(rd) => rd,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut matching_files = Vec::new();
    let mut matched_labels = Vec::new();

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yml" && ext != "yaml" {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path).await {
            let file_matches = labels
                .iter()
                .any(|label| content.contains(&format!("runs-on: {}", label)));
            if file_matches {
                let rel = path
                    .strip_prefix(repo_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                matching_files.push(rel);
                for label in labels {
                    if content.contains(&format!("runs-on: {}", label))
                        && !matched_labels.contains(label)
                    {
                        matched_labels.push(label.clone());
                    }
                }
            }
        }
    }

    matching_files.sort();
    matched_labels.sort();
    (matching_files, matched_labels)
}

/// Two-phase local scan with progress events and cancellation support.
///
/// Phase 1 discovers all repos with `.github/workflows/` dirs, then phase 2
/// checks each repo's workflow files for matching labels, emitting progress
/// events throughout.
pub async fn scan_local_with_progress<F>(
    workspace_dir: &Path,
    labels: &[String],
    cancel: CancellationToken,
    on_progress: F,
) -> Result<Vec<DiscoveredRepo>>
where
    F: Fn(ScanProgressEvent) + Send,
{
    let repos = discover_local_repos(workspace_dir).await?;
    let total = repos.len();

    on_progress(ScanProgressEvent::Started {
        scan_type: "local".to_string(),
        total,
    });

    let mut results = Vec::new();

    for (index, (full_name, repo_root, workflows_dir)) in repos.into_iter().enumerate() {
        if cancel.is_cancelled() {
            on_progress(ScanProgressEvent::Cancelled {
                scan_type: "local".to_string(),
                checked: index,
                total,
            });
            return Ok(results);
        }

        on_progress(ScanProgressEvent::Checking {
            repo: full_name.clone(),
            index: index + 1,
            total,
        });

        let (workflow_files, matched_labels) =
            check_workflows_dir(&workflows_dir, &repo_root, labels).await;

        if !workflow_files.is_empty() {
            let repo = DiscoveredRepo {
                full_name,
                source: DiscoverySource::Local,
                workflow_files,
                matched_labels,
                local_path: Some(repo_root),
            };
            on_progress(ScanProgressEvent::Found { repo: repo.clone() });
            results.push(repo);
        }
    }

    on_progress(ScanProgressEvent::Done {
        scan_type: "local".to_string(),
        total_found: results.len(),
        total_checked: total,
    });

    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
}

/// Run `git config --get remote.origin.url` in `repo_root` and extract
/// `owner/repo` from the result.
async fn git_remote_full_name(repo_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(repo_root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_github_full_name(&url)
}

/// Parse `owner/repo` out of various GitHub URL formats:
/// - `https://github.com/owner/repo.git`
/// - `git@github.com:owner/repo.git`
/// - `https://github.com/owner/repo`
fn parse_github_full_name(url: &str) -> Option<String> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let name = rest.trim_end_matches(".git");
        return Some(name.to_string());
    }

    // HTTPS: https://github.com/owner/repo[.git]
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let name = rest.trim_end_matches(".git");
        return Some(name.to_string());
    }

    None
}

// ---------------------------------------------------------------------------
// Remote scan
// ---------------------------------------------------------------------------

/// Fetch the authenticated user's repos from GitHub, check their workflow
/// files via the API, and return repos that use any of the given `runs-on:` labels.
pub async fn scan_remote(
    github_client: &GitHubClient,
    labels: &[String],
) -> Result<Vec<DiscoveredRepo>> {
    let repos = github_client.list_repos().await?;
    let mut results: Vec<DiscoveredRepo> = Vec::new();

    for repo in repos {
        let (workflow_files, matched_labels) = github_client
            .list_workflows_with_labels(&repo.owner, &repo.name, labels)
            .await
            .unwrap_or_default();

        if !workflow_files.is_empty() {
            results.push(DiscoveredRepo {
                full_name: repo.full_name.clone(),
                source: DiscoverySource::Remote,
                workflow_files,
                matched_labels,
                local_path: None,
            });
        }
    }

    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
}

/// Remote scan with progress. Fetches repo list first (phase 1), then checks each (phase 2).
pub async fn scan_remote_with_progress<F>(
    github_client: &GitHubClient,
    labels: &[String],
    cancel: CancellationToken,
    on_progress: F,
) -> Result<Vec<DiscoveredRepo>>
where
    F: Fn(ScanProgressEvent) + Send,
{
    let repos = github_client.list_repos().await?;
    let total = repos.len();

    on_progress(ScanProgressEvent::Started {
        scan_type: "remote".to_string(),
        total,
    });

    let mut results = Vec::new();

    for (index, repo) in repos.into_iter().enumerate() {
        if cancel.is_cancelled() {
            on_progress(ScanProgressEvent::Cancelled {
                scan_type: "remote".to_string(),
                checked: index,
                total,
            });
            return Ok(results);
        }

        on_progress(ScanProgressEvent::Checking {
            repo: repo.full_name.clone(),
            index: index + 1,
            total,
        });

        let (workflow_files, matched_labels) = github_client
            .list_workflows_with_labels(&repo.owner, &repo.name, labels)
            .await
            .unwrap_or_default();

        if !workflow_files.is_empty() {
            let discovered = DiscoveredRepo {
                full_name: repo.full_name.clone(),
                source: DiscoverySource::Remote,
                workflow_files,
                matched_labels,
                local_path: None,
            };
            on_progress(ScanProgressEvent::Found {
                repo: discovered.clone(),
            });
            results.push(discovered);
        }
    }

    on_progress(ScanProgressEvent::Done {
        scan_type: "remote".to_string(),
        total_found: results.len(),
        total_checked: total,
    });

    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge local and remote scan results. Repos found in both get `source: Both`.
pub fn merge_results(
    local: Vec<DiscoveredRepo>,
    remote: Vec<DiscoveredRepo>,
) -> Vec<DiscoveredRepo> {
    let mut by_name: HashMap<String, DiscoveredRepo> = HashMap::new();

    for repo in local {
        by_name.insert(repo.full_name.clone(), repo);
    }

    for repo in remote {
        by_name
            .entry(repo.full_name.clone())
            .and_modify(|existing| {
                existing.source = DiscoverySource::Both;
                for wf in &repo.workflow_files {
                    if !existing.workflow_files.contains(wf) {
                        existing.workflow_files.push(wf.clone());
                    }
                }
                for label in &repo.matched_labels {
                    if !existing.matched_labels.contains(label) {
                        existing.matched_labels.push(label.clone());
                    }
                }
            })
            .or_insert(repo);
    }

    let mut results: Vec<DiscoveredRepo> = by_name.into_values().collect();
    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as std_fs;
    use tempfile::TempDir;

    fn create_workflow(dir: &Path, filename: &str, content: &str) {
        let workflows_dir = dir.join(".github/workflows");
        std_fs::create_dir_all(&workflows_dir).unwrap();
        std_fs::write(workflows_dir.join(filename), content).unwrap();
    }

    fn init_git_remote(dir: &Path, remote_url: &str) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["remote", "add", "origin", remote_url])
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap();
    }

    // --- parse_github_full_name ---

    #[test]
    fn test_parse_ssh_url() {
        assert_eq!(
            parse_github_full_name("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_https_url() {
        assert_eq!(
            parse_github_full_name("https://github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
        assert_eq!(
            parse_github_full_name("https://github.com/owner/repo"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn test_parse_non_github_url() {
        assert_eq!(
            parse_github_full_name("https://gitlab.com/owner/repo.git"),
            None
        );
    }

    // --- scan_local ---

    #[tokio::test]
    async fn test_local_scan_finds_self_hosted_repo() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("my-project");
        std_fs::create_dir_all(&repo_dir).unwrap();

        init_git_remote(&repo_dir, "git@github.com:acme/my-project.git");

        create_workflow(
            &repo_dir,
            "ci.yml",
            "jobs:\n  build:\n    runs-on: self-hosted\n",
        );

        let labels = vec!["self-hosted".to_string()];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "acme/my-project");
        assert!(repos[0].workflow_files.iter().any(|f| f.contains("ci.yml")));
        assert_eq!(repos[0].source, DiscoverySource::Local);
        assert!(repos[0].local_path.is_some());
        assert_eq!(repos[0].matched_labels, vec!["self-hosted".to_string()]);
    }

    #[tokio::test]
    async fn test_local_scan_skips_repo_without_self_hosted() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("other-project");
        std_fs::create_dir_all(&repo_dir).unwrap();

        init_git_remote(&repo_dir, "git@github.com:acme/other-project.git");

        create_workflow(
            &repo_dir,
            "ci.yml",
            "jobs:\n  build:\n    runs-on: ubuntu-latest\n",
        );

        let labels = vec!["self-hosted".to_string()];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert!(repos.is_empty());
    }

    #[tokio::test]
    async fn test_local_scan_multiple_repos() {
        let tmp = TempDir::new().unwrap();

        // Repo 1 — uses self-hosted
        let repo1 = tmp.path().join("repo1");
        std_fs::create_dir_all(&repo1).unwrap();
        init_git_remote(&repo1, "git@github.com:acme/repo1.git");
        create_workflow(&repo1, "ci.yml", "runs-on: self-hosted\n");

        // Repo 2 — does NOT use self-hosted
        let repo2 = tmp.path().join("repo2");
        std_fs::create_dir_all(&repo2).unwrap();
        init_git_remote(&repo2, "git@github.com:acme/repo2.git");
        create_workflow(&repo2, "ci.yml", "runs-on: ubuntu-latest\n");

        // Repo 3 — uses self-hosted in one of two workflows
        let repo3 = tmp.path().join("repo3");
        std_fs::create_dir_all(&repo3).unwrap();
        init_git_remote(&repo3, "git@github.com:acme/repo3.git");
        create_workflow(&repo3, "deploy.yml", "runs-on: self-hosted\n");
        create_workflow(&repo3, "lint.yml", "runs-on: ubuntu-latest\n");

        let labels = vec!["self-hosted".to_string()];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert_eq!(repos.len(), 2);

        let names: Vec<&str> = repos.iter().map(|r| r.full_name.as_str()).collect();
        assert!(names.contains(&"acme/repo1"));
        assert!(names.contains(&"acme/repo3"));

        let repo3_result = repos.iter().find(|r| r.full_name == "acme/repo3").unwrap();
        assert_eq!(repo3_result.workflow_files.len(), 1);
        assert!(repo3_result.workflow_files[0].contains("deploy.yml"));
        assert_eq!(repo3_result.matched_labels, vec!["self-hosted".to_string()]);
    }

    // --- Custom label tests ---

    #[tokio::test]
    async fn test_local_scan_with_custom_labels() {
        let tmp = TempDir::new().unwrap();
        let repo1 = tmp.path().join("gpu-project");
        std_fs::create_dir_all(&repo1).unwrap();
        init_git_remote(&repo1, "git@github.com:acme/gpu-project.git");
        create_workflow(&repo1, "train.yml", "runs-on: gpu\n");
        let repo2 = tmp.path().join("web-app");
        std_fs::create_dir_all(&repo2).unwrap();
        init_git_remote(&repo2, "git@github.com:acme/web-app.git");
        create_workflow(&repo2, "ci.yml", "runs-on: self-hosted\n");
        let labels = vec!["gpu".to_string()];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "acme/gpu-project");
        assert_eq!(repos[0].matched_labels, vec!["gpu".to_string()]);
    }

    #[tokio::test]
    async fn test_local_scan_with_multiple_labels() {
        let tmp = TempDir::new().unwrap();
        let repo1 = tmp.path().join("project");
        std_fs::create_dir_all(&repo1).unwrap();
        init_git_remote(&repo1, "git@github.com:acme/project.git");
        create_workflow(
            &repo1,
            "ci.yml",
            "jobs:\n  build:\n    runs-on: self-hosted\n  train:\n    runs-on: gpu\n",
        );
        let labels = vec!["self-hosted".to_string(), "gpu".to_string()];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert_eq!(repos.len(), 1);
        assert!(repos[0].matched_labels.contains(&"self-hosted".to_string()));
        assert!(repos[0].matched_labels.contains(&"gpu".to_string()));
    }

    #[tokio::test]
    async fn test_local_scan_empty_labels_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("project");
        std_fs::create_dir_all(&repo).unwrap();
        init_git_remote(&repo, "git@github.com:acme/project.git");
        create_workflow(&repo, "ci.yml", "runs-on: self-hosted\n");
        let labels: Vec<String> = vec![];
        let repos = scan_local(tmp.path(), &labels).await.unwrap();
        assert!(repos.is_empty());
    }

    // --- DiscoveredRepo serialization ---

    #[test]
    fn test_discovered_repo_serialization() {
        let repo = DiscoveredRepo {
            full_name: "owner/repo".to_string(),
            source: DiscoverySource::Both,
            workflow_files: vec![".github/workflows/ci.yml".to_string()],
            matched_labels: vec!["self-hosted".to_string()],
            local_path: Some(PathBuf::from("/Users/dev/workspace/repo")),
        };

        let json = serde_json::to_string(&repo).unwrap();
        let deserialized: DiscoveredRepo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.full_name, "owner/repo");
        assert_eq!(deserialized.source, DiscoverySource::Both);
        assert_eq!(deserialized.workflow_files.len(), 1);
        assert_eq!(deserialized.matched_labels, vec!["self-hosted".to_string()]);
        assert_eq!(
            deserialized.local_path,
            Some(PathBuf::from("/Users/dev/workspace/repo"))
        );
    }

    #[test]
    fn test_discovery_source_serialization() {
        assert_eq!(
            serde_json::to_string(&DiscoverySource::Local).unwrap(),
            "\"local\""
        );
        assert_eq!(
            serde_json::to_string(&DiscoverySource::Remote).unwrap(),
            "\"remote\""
        );
        assert_eq!(
            serde_json::to_string(&DiscoverySource::Both).unwrap(),
            "\"both\""
        );
    }

    #[test]
    fn test_merge_results_combines_sources() {
        let local = vec![DiscoveredRepo {
            full_name: "acme/api".to_string(),
            source: DiscoverySource::Local,
            workflow_files: vec!["ci.yml".to_string()],
            matched_labels: vec!["self-hosted".to_string()],
            local_path: Some(PathBuf::from("/workspace/api")),
        }];
        let remote = vec![DiscoveredRepo {
            full_name: "acme/api".to_string(),
            source: DiscoverySource::Remote,
            workflow_files: vec!["ci.yml".to_string()],
            matched_labels: vec!["self-hosted".to_string()],
            local_path: None,
        }];

        let merged = merge_results(local, remote);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, DiscoverySource::Both);
        assert!(merged[0].local_path.is_some());
    }

    #[test]
    fn test_merge_results_keeps_unique() {
        let local = vec![DiscoveredRepo {
            full_name: "acme/api".to_string(),
            source: DiscoverySource::Local,
            workflow_files: vec!["ci.yml".to_string()],
            matched_labels: vec!["self-hosted".to_string()],
            local_path: None,
        }];
        let remote = vec![DiscoveredRepo {
            full_name: "acme/web".to_string(),
            source: DiscoverySource::Remote,
            workflow_files: vec!["deploy.yml".to_string()],
            matched_labels: vec!["gpu".to_string()],
            local_path: None,
        }];

        let merged = merge_results(local, remote);
        assert_eq!(merged.len(), 2);
    }

    // --- Two-phase scan tests ---

    #[tokio::test]
    async fn test_discover_local_repos_finds_git_repos() {
        let tmp = TempDir::new().unwrap();

        let repo1 = tmp.path().join("project-a");
        std_fs::create_dir_all(repo1.join(".github/workflows")).unwrap();
        init_git_remote(&repo1, "git@github.com:acme/project-a.git");
        std_fs::write(
            repo1.join(".github/workflows/ci.yml"),
            "runs-on: self-hosted\n",
        )
        .unwrap();

        let repo2 = tmp.path().join("project-b");
        std_fs::create_dir_all(repo2.join(".github/workflows")).unwrap();
        init_git_remote(&repo2, "git@github.com:acme/project-b.git");
        std_fs::write(
            repo2.join(".github/workflows/ci.yml"),
            "runs-on: ubuntu-latest\n",
        )
        .unwrap();

        let repos = discover_local_repos(tmp.path()).await.unwrap();
        assert_eq!(repos.len(), 2);
    }

    #[tokio::test]
    async fn test_scan_local_with_progress_emits_events() {
        use std::sync::{Arc, Mutex};

        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("project");
        std_fs::create_dir_all(&repo).unwrap();
        init_git_remote(&repo, "git@github.com:acme/project.git");
        create_workflow(&repo, "ci.yml", "runs-on: self-hosted\n");

        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let labels = vec!["self-hosted".to_string()];
        let cancel = CancellationToken::new();

        let results = scan_local_with_progress(tmp.path(), &labels, cancel, move |event| {
            let tag = match &event {
                ScanProgressEvent::Started { .. } => "started",
                ScanProgressEvent::Checking { .. } => "checking",
                ScanProgressEvent::Found { .. } => "found",
                ScanProgressEvent::Done { .. } => "done",
                ScanProgressEvent::Cancelled { .. } => "cancelled",
            };
            events_clone.lock().unwrap().push(tag.to_string());
        })
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        let ev = events.lock().unwrap();
        assert_eq!(ev[0], "started");
        assert_eq!(ev[1], "checking");
        assert_eq!(ev[2], "found");
        assert_eq!(ev[3], "done");
    }

    #[tokio::test]
    async fn test_scan_local_with_progress_cancellation() {
        let tmp = TempDir::new().unwrap();

        for name in &["repo1", "repo2"] {
            let repo = tmp.path().join(name);
            std_fs::create_dir_all(&repo).unwrap();
            init_git_remote(&repo, &format!("git@github.com:acme/{}.git", name));
            create_workflow(&repo, "ci.yml", "runs-on: self-hosted\n");
        }

        let cancel = CancellationToken::new();
        cancel.cancel();

        let labels = vec!["self-hosted".to_string()];
        let results = scan_local_with_progress(tmp.path(), &labels, cancel, |_| {})
            .await
            .unwrap();
        assert!(results.is_empty());
    }
}
