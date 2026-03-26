use crate::runner::types::JobHistoryEntry;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

const MAX_HISTORY_PER_RUNNER: usize = 100;

/// Load all job history from disk. Reads each `{runner_id}.json` file in the history directory.
pub fn load_all(history_dir: &Path) -> Result<HashMap<String, Vec<JobHistoryEntry>>> {
    if !history_dir.exists() {
        return Ok(HashMap::new());
    }

    let mut result = HashMap::new();

    for entry in std::fs::read_dir(history_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let runner_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let content = std::fs::read_to_string(&path)?;
        let entries: Vec<JobHistoryEntry> = serde_json::from_str(&content)?;
        result.insert(runner_id, entries);
    }

    Ok(result)
}

/// Save a single runner's history to disk.
pub fn save(history_dir: &Path, runner_id: &str, entries: &[JobHistoryEntry]) -> Result<()> {
    std::fs::create_dir_all(history_dir)?;
    let path = history_dir.join(format!("{}.json", runner_id));
    let content = serde_json::to_string_pretty(entries)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Delete a runner's history file.
pub fn delete(history_dir: &Path, runner_id: &str) -> Result<()> {
    let path = history_dir.join(format!("{}.json", runner_id));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Extract the numeric run ID from a GitHub Actions run URL.
/// Handles both `…/runs/12345` and `…/runs/12345/job/67890`.
fn extract_run_id(run_url: &str) -> Option<u64> {
    let parts: Vec<&str> = run_url.split('/').collect();
    let runs_idx = parts.iter().position(|&p| p == "runs")?;
    parts.get(runs_idx + 1)?.parse().ok()
}

/// Public version of run-ID extraction for use by the re-run poller.
pub fn extract_run_id_from_url(run_url: &str) -> Option<u64> {
    extract_run_id(run_url)
}

/// Append a history entry, keeping the list capped at MAX_HISTORY_PER_RUNNER.
///
/// When a workflow run is re-run, the new attempt shares the same `run_id` but
/// gets a different `job_id`. If an existing entry matches on both `run_id`
/// (extracted from `run_url`) and `job_name`, it is replaced with the new entry
/// so that history reflects the latest result for each run.
pub fn append(entries: &mut Vec<JobHistoryEntry>, entry: JobHistoryEntry) {
    if let Some(new_run_id) = entry.run_url.as_deref().and_then(extract_run_id) {
        if let Some(pos) = entries.iter().position(|e| {
            e.job_name == entry.job_name
                && e.run_url.as_deref().and_then(extract_run_id) == Some(new_run_id)
        }) {
            entries[pos] = entry;
            return;
        }
    }

    entries.push(entry);
    if entries.len() > MAX_HISTORY_PER_RUNNER {
        entries.remove(0);
    }
}

/// Compute the median duration (in seconds) of succeeded history entries matching `job_name`.
/// Returns `None` if there are no matching succeeded entries.
pub fn median_duration_secs(entries: &[JobHistoryEntry], job_name: &str) -> Option<u64> {
    let mut durations: Vec<u64> = entries
        .iter()
        .filter(|e| e.succeeded && e.job_name == job_name)
        .map(|e| (e.completed_at - e.started_at).num_seconds().max(0) as u64)
        .collect();

    if durations.is_empty() {
        return None;
    }

    durations.sort_unstable();
    let len = durations.len();
    let median = if len % 2 == 1 {
        durations[len / 2]
    } else {
        (durations[len / 2 - 1] + durations[len / 2]) / 2
    };

    Some(median)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::steps::{StepInfo, StepStatus};
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_entry(job_name: &str) -> JobHistoryEntry {
        JobHistoryEntry {
            job_name: job_name.to_string(),
            started_at: Utc::now(),
            completed_at: Utc::now(),
            succeeded: true,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        }
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("history");

        let entries = vec![make_entry("job-1"), make_entry("job-2")];
        save(&history_dir, "runner-abc", &entries).unwrap();

        let loaded = load_all(&history_dir).unwrap();
        assert_eq!(loaded.len(), 1);
        let runner_entries = loaded.get("runner-abc").unwrap();
        assert_eq!(runner_entries.len(), 2);
        assert_eq!(runner_entries[0].job_name, "job-1");
        assert_eq!(runner_entries[1].job_name, "job-2");
    }

    #[test]
    fn test_load_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("history");
        std::fs::create_dir_all(&history_dir).unwrap();

        let loaded = load_all(&history_dir).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("nonexistent");

        let loaded = load_all(&history_dir).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_delete_history() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("history");

        let entries = vec![make_entry("job-1")];
        save(&history_dir, "runner-xyz", &entries).unwrap();

        let file_path = history_dir.join("runner-xyz.json");
        assert!(file_path.exists());

        delete(&history_dir, "runner-xyz").unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("history");

        // Should not error even if file doesn't exist
        let result = delete(&history_dir, "runner-does-not-exist");
        assert!(result.is_ok());
    }

    #[test]
    fn test_append_caps_at_max() {
        let mut entries: Vec<JobHistoryEntry> = Vec::new();

        for i in 0..110 {
            append(&mut entries, make_entry(&format!("job-{}", i)));
        }

        assert_eq!(entries.len(), MAX_HISTORY_PER_RUNNER);
        // Oldest entries should have been removed; first remaining should be job-10
        assert_eq!(entries[0].job_name, "job-10");
        assert_eq!(entries[99].job_name, "job-109");
    }

    #[test]
    fn test_median_duration_no_entries() {
        let entries: Vec<JobHistoryEntry> = vec![];
        assert_eq!(median_duration_secs(&entries, "build"), None);
    }

    #[test]
    fn test_median_duration_no_succeeded_entries() {
        let now = Utc::now();
        let entries = vec![JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(120),
            completed_at: now,
            succeeded: false,
            branch: None,
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        }];
        assert_eq!(median_duration_secs(&entries, "build"), None);
    }

    #[test]
    fn test_median_duration_single_entry() {
        let now = Utc::now();
        let entries = vec![JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(300),
            completed_at: now,
            succeeded: true,
            branch: None,
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        }];
        assert_eq!(median_duration_secs(&entries, "build"), Some(300));
    }

    #[test]
    fn test_median_duration_odd_count() {
        let now = Utc::now();
        let make = |secs: i64| JobHistoryEntry {
            job_name: "test".to_string(),
            started_at: now - chrono::Duration::seconds(secs),
            completed_at: now,
            succeeded: true,
            branch: None,
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };
        let entries = vec![make(100), make(200), make(300)];
        assert_eq!(median_duration_secs(&entries, "test"), Some(200));
    }

    #[test]
    fn test_median_duration_even_count() {
        let now = Utc::now();
        let make = |secs: i64| JobHistoryEntry {
            job_name: "test".to_string(),
            started_at: now - chrono::Duration::seconds(secs),
            completed_at: now,
            succeeded: true,
            branch: None,
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };
        let entries = vec![make(100), make(200), make(300), make(400)];
        // median of [100, 200, 300, 400] = (200 + 300) / 2 = 250
        assert_eq!(median_duration_secs(&entries, "test"), Some(250));
    }

    #[test]
    fn test_median_duration_filters_by_job_name() {
        let now = Utc::now();
        let make = |name: &str, secs: i64| JobHistoryEntry {
            job_name: name.to_string(),
            started_at: now - chrono::Duration::seconds(secs),
            completed_at: now,
            succeeded: true,
            branch: None,
            pr_number: None,
            run_url: None,
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };
        let entries = vec![make("build", 100), make("test", 500), make("build", 300)];
        assert_eq!(median_duration_secs(&entries, "build"), Some(200));
        assert_eq!(median_duration_secs(&entries, "test"), Some(500));
    }

    #[test]
    fn test_median_duration_ignores_failed() {
        let now = Utc::now();
        let entries = vec![
            JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(100),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
            },
            JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(9999),
                completed_at: now,
                succeeded: false, // should be ignored
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
            },
            JobHistoryEntry {
                job_name: "build".to_string(),
                started_at: now - chrono::Duration::seconds(300),
                completed_at: now,
                succeeded: true,
                branch: None,
                pr_number: None,
                run_url: None,
                error_message: None,
                steps: vec![],
                latest_attempt: None,
            },
        ];
        assert_eq!(median_duration_secs(&entries, "build"), Some(200));
    }

    #[test]
    fn test_save_load_roundtrip_with_steps() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join("history");

        let entry = JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: Utc::now(),
            completed_at: Utc::now(),
            succeeded: true,
            branch: Some("feature/test".to_string()),
            pr_number: Some(42),
            run_url: Some("https://github.com/owner/repo/actions/runs/123".to_string()),
            error_message: None,
            steps: vec![
                StepInfo {
                    number: 1,
                    name: "Checkout".to_string(),
                    status: StepStatus::Succeeded,
                    started_at: Some(Utc::now()),
                    completed_at: Some(Utc::now()),
                },
                StepInfo {
                    number: 2,
                    name: "Build".to_string(),
                    status: StepStatus::Failed,
                    started_at: Some(Utc::now()),
                    completed_at: None,
                },
            ],
            latest_attempt: None,
        };

        save(&history_dir, "runner-steps", std::slice::from_ref(&entry)).unwrap();
        let loaded = load_all(&history_dir).unwrap();

        let runner_entries = loaded.get("runner-steps").unwrap();
        assert_eq!(runner_entries.len(), 1);

        let loaded_entry = &runner_entries[0];
        assert_eq!(loaded_entry.job_name, entry.job_name);
        assert_eq!(loaded_entry.steps.len(), 2);
        assert_eq!(loaded_entry.steps[0].name, "Checkout");
        assert_eq!(loaded_entry.steps[0].status, StepStatus::Succeeded);
        assert_eq!(loaded_entry.steps[1].name, "Build");
        assert_eq!(loaded_entry.steps[1].status, StepStatus::Failed);
        assert_eq!(loaded_entry.pr_number, Some(42));
        assert_eq!(
            loaded_entry.run_url,
            Some("https://github.com/owner/repo/actions/runs/123".to_string())
        );
    }

    #[test]
    fn test_extract_run_id_simple() {
        assert_eq!(
            extract_run_id("https://github.com/owner/repo/actions/runs/12345"),
            Some(12345)
        );
    }

    #[test]
    fn test_extract_run_id_with_job() {
        assert_eq!(
            extract_run_id("https://github.com/owner/repo/actions/runs/12345/job/67890"),
            Some(12345)
        );
    }

    #[test]
    fn test_extract_run_id_invalid() {
        assert_eq!(extract_run_id("not-a-url"), None);
        assert_eq!(extract_run_id("https://github.com/owner/repo"), None);
    }

    #[test]
    fn test_append_replaces_rerun_same_run_id_and_job_name() {
        let now = Utc::now();
        let mut entries = vec![JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(600),
            completed_at: now - chrono::Duration::seconds(300),
            succeeded: false,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/100/job/200".to_string()),
            error_message: Some("Process completed with exit code 1.".to_string()),
            steps: vec![],
            latest_attempt: None,
        }];

        // Re-run: same run_id (100), same job_name, different job_id (999)
        let rerun_entry = JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(120),
            completed_at: now,
            succeeded: true,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/100/job/999".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };

        append(&mut entries, rerun_entry);

        // Should replace, not append
        assert_eq!(entries.len(), 1);
        assert!(entries[0].succeeded);
        assert_eq!(
            entries[0].run_url.as_deref(),
            Some("https://github.com/owner/repo/actions/runs/100/job/999")
        );
        assert!(entries[0].error_message.is_none());
    }

    #[test]
    fn test_append_does_not_replace_different_run_id() {
        let now = Utc::now();
        let mut entries = vec![JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(600),
            completed_at: now - chrono::Duration::seconds(300),
            succeeded: false,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/100/job/200".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        }];

        // Different run_id (999), same job_name
        let new_entry = JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(120),
            completed_at: now,
            succeeded: true,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/999/job/888".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };

        append(&mut entries, new_entry);

        // Should append, not replace
        assert_eq!(entries.len(), 2);
        assert!(!entries[0].succeeded);
        assert!(entries[1].succeeded);
    }

    #[test]
    fn test_append_does_not_replace_different_job_name_same_run_id() {
        let now = Utc::now();
        let mut entries = vec![JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(600),
            completed_at: now - chrono::Duration::seconds(300),
            succeeded: true,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/100/job/200".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        }];

        // Same run_id (100), different job_name
        let new_entry = JobHistoryEntry {
            job_name: "test".to_string(),
            started_at: now - chrono::Duration::seconds(120),
            completed_at: now,
            succeeded: false,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/owner/repo/actions/runs/100/job/300".to_string()),
            error_message: None,
            steps: vec![],
            latest_attempt: None,
        };

        append(&mut entries, new_entry);

        // Should append — different jobs from the same run are separate entries
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].job_name, "build");
        assert_eq!(entries[1].job_name, "test");
    }

    #[test]
    fn test_append_no_run_url_still_appends() {
        let mut entries = vec![make_entry("build")];
        let mut new_entry = make_entry("build");
        new_entry.run_url = None;

        append(&mut entries, new_entry);

        // Without run_url we can't detect re-runs, so always append
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_latest_attempt_serialization_roundtrip() {
        let now = Utc::now();
        let entry = JobHistoryEntry {
            job_name: "build".to_string(),
            started_at: now - chrono::Duration::seconds(300),
            completed_at: now,
            succeeded: false,
            branch: Some("main".to_string()),
            pr_number: None,
            run_url: Some("https://github.com/o/r/actions/runs/100/job/200".to_string()),
            error_message: Some("exit code 1".to_string()),
            steps: vec![],
            latest_attempt: Some(crate::runner::types::RunAttempt {
                attempt: 2,
                succeeded: true,
                runner_name: "runner-2".to_string(),
                completed_at: now,
                run_url: Some("https://github.com/o/r/actions/runs/100/job/500".to_string()),
            }),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let back: JobHistoryEntry = serde_json::from_str(&json).unwrap();
        assert!(back.latest_attempt.is_some());
        let attempt = back.latest_attempt.unwrap();
        assert_eq!(attempt.attempt, 2);
        assert!(attempt.succeeded);
        assert_eq!(attempt.runner_name, "runner-2");
    }

    #[test]
    fn test_latest_attempt_none_by_default_in_old_json() {
        let json = r#"{
            "job_name": "build",
            "started_at": "2026-03-24T10:00:00Z",
            "completed_at": "2026-03-24T10:05:00Z",
            "succeeded": false,
            "run_url": "https://github.com/o/r/actions/runs/100/job/200",
            "steps": []
        }"#;
        let entry: JobHistoryEntry = serde_json::from_str(json).unwrap();
        assert!(entry.latest_attempt.is_none());
    }
}
