use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;

use crate::github::GitHubClient;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredRepo {
    pub full_name: String,
    pub source: DiscoverySource,
    /// Workflow files (relative paths) that contain `runs-on: self-hosted`
    pub workflow_files: Vec<String>,
    pub local_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoverySource {
    Local,
    Remote,
    Both,
}

// ---------------------------------------------------------------------------
// Local scan
// ---------------------------------------------------------------------------

/// Recursively walk `workspace_dir`, find all `.github/workflows/*.yml` files,
/// and return repos that have at least one workflow with `runs-on: self-hosted`.
pub async fn scan_local(workspace_dir: &Path) -> Result<Vec<DiscoveredRepo>> {
    // Map: repo_root -> (full_name, matching_workflow_files)
    let mut found: HashMap<PathBuf, (String, Vec<String>)> = HashMap::new();

    collect_workflow_files(workspace_dir, &mut found).await?;

    let mut results: Vec<DiscoveredRepo> = found
        .into_iter()
        .map(|(root, (full_name, workflow_files))| DiscoveredRepo {
            full_name,
            source: DiscoverySource::Local,
            workflow_files,
            local_path: Some(root),
        })
        .collect();

    // Stable ordering for tests and display
    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
}

/// Recurse into `dir`. When we find a `.github/workflows` directory, check
/// whether its parent is a git repo and inspect the workflow files.
async fn collect_workflow_files(
    dir: &Path,
    found: &mut HashMap<PathBuf, (String, Vec<String>)>,
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
                    process_workflows_dir(repo_root, &workflows_dir, found).await;
                }
            }
            // Don't recurse into .github itself
            continue;
        }

        // Recurse
        Box::pin(collect_workflow_files(&path, found)).await?;
    }

    Ok(())
}

async fn process_workflows_dir(
    repo_root: &Path,
    workflows_dir: &Path,
    found: &mut HashMap<PathBuf, (String, Vec<String>)>,
) {
    let mut rd = match fs::read_dir(workflows_dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut matching_files: Vec<String> = Vec::new();

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yml" && ext != "yaml" {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path).await {
            if content.contains("runs-on: self-hosted") {
                let rel = path
                    .strip_prefix(repo_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                matching_files.push(rel);
            }
        }
    }

    if matching_files.is_empty() {
        return;
    }

    matching_files.sort();

    // Determine the repo full name from the git remote URL
    let full_name = git_remote_full_name(repo_root).await.unwrap_or_else(|| {
        repo_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });

    let entry = found
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| (full_name.clone(), Vec::new()));
    // If we already visited this root (shouldn't happen but be safe), merge files
    for f in matching_files {
        if !entry.1.contains(&f) {
            entry.1.push(f);
        }
    }
    entry.1.sort();
}

/// Run `git config --get remote.origin.url` in `repo_root` and extract
/// `owner/repo` from the result.
async fn git_remote_full_name(repo_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(repo_root)
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
/// files via the API, and return repos that use `runs-on: self-hosted`.
pub async fn scan_remote(github_client: &GitHubClient) -> Result<Vec<DiscoveredRepo>> {
    let repos = github_client.list_repos().await?;
    let mut results: Vec<DiscoveredRepo> = Vec::new();

    for repo in repos {
        let workflow_files = github_client
            .list_self_hosted_workflows(&repo.owner, &repo.name)
            .await
            .unwrap_or_default();

        if !workflow_files.is_empty() {
            results.push(DiscoveredRepo {
                full_name: repo.full_name.clone(),
                source: DiscoverySource::Remote,
                workflow_files,
                local_path: None,
            });
        }
    }

    results.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(results)
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
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["remote", "add", "origin", remote_url])
            .current_dir(dir)
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

        let repos = scan_local(tmp.path()).await.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "acme/my-project");
        assert!(repos[0].workflow_files.iter().any(|f| f.contains("ci.yml")));
        assert_eq!(repos[0].source, DiscoverySource::Local);
        assert!(repos[0].local_path.is_some());
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

        let repos = scan_local(tmp.path()).await.unwrap();
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

        let repos = scan_local(tmp.path()).await.unwrap();
        assert_eq!(repos.len(), 2);

        let names: Vec<&str> = repos.iter().map(|r| r.full_name.as_str()).collect();
        assert!(names.contains(&"acme/repo1"));
        assert!(names.contains(&"acme/repo3"));

        let repo3_result = repos.iter().find(|r| r.full_name == "acme/repo3").unwrap();
        assert_eq!(repo3_result.workflow_files.len(), 1);
        assert!(repo3_result.workflow_files[0].contains("deploy.yml"));
    }

    // --- DiscoveredRepo serialization ---

    #[test]
    fn test_discovered_repo_serialization() {
        let repo = DiscoveredRepo {
            full_name: "owner/repo".to_string(),
            source: DiscoverySource::Both,
            workflow_files: vec![".github/workflows/ci.yml".to_string()],
            local_path: Some(PathBuf::from("/Users/dev/workspace/repo")),
        };

        let json = serde_json::to_string(&repo).unwrap();
        let deserialized: DiscoveredRepo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.full_name, "owner/repo");
        assert_eq!(deserialized.source, DiscoverySource::Both);
        assert_eq!(deserialized.workflow_files.len(), 1);
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
}
