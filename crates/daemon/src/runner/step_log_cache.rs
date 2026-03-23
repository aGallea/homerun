use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

struct CacheEntry {
    raw_log: String,
    fetched_at: Instant,
    job_completed: bool,
}

#[derive(Clone)]
pub struct StepLogCache {
    entries: Arc<RwLock<HashMap<u64, CacheEntry>>>,
    refresh_interval: Duration,
    ttl_after_completion: Duration,
}

impl StepLogCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            refresh_interval: Duration::from_secs(5),
            ttl_after_completion: Duration::from_secs(300),
        }
    }

    /// Return cached log for `job_id` if fresh enough, otherwise fetch from GitHub API and cache.
    pub async fn get_or_fetch(
        &self,
        job_id: u64,
        gh: &crate::github::GitHubClient,
        owner: &str,
        repo: &str,
    ) -> anyhow::Result<String> {
        // Check if there's a valid cached entry
        {
            let entries = self.entries.read().await;
            if let Some(entry) = entries.get(&job_id) {
                let age = entry.fetched_at.elapsed();
                let is_fresh = if entry.job_completed {
                    age <= self.ttl_after_completion
                } else {
                    age <= self.refresh_interval
                };
                if is_fresh {
                    return Ok(entry.raw_log.clone());
                }
            }
        }

        // Cache miss or stale — fetch from GitHub API
        let raw_log = gh.get_job_logs(owner, repo, job_id).await?;

        {
            let mut entries = self.entries.write().await;
            // Preserve job_completed flag if entry already exists
            let job_completed = entries
                .get(&job_id)
                .map(|e| e.job_completed)
                .unwrap_or(false);
            entries.insert(
                job_id,
                CacheEntry {
                    raw_log: raw_log.clone(),
                    fetched_at: Instant::now(),
                    job_completed,
                },
            );
        }

        Ok(raw_log)
    }

    /// Mark a job as completed, starting the TTL countdown for eviction.
    pub async fn mark_completed(&self, job_id: u64) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&job_id) {
            entry.job_completed = true;
        }
    }

    /// Remove entries whose TTL has expired (only applies to completed jobs).
    pub async fn evict_expired(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| {
            if entry.job_completed {
                entry.fetched_at.elapsed() <= self.ttl_after_completion
            } else {
                true
            }
        });
    }
}

impl Default for StepLogCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_returns_cached_value_within_interval() {
        let cache = StepLogCache::new();

        // Manually insert a fresh entry
        {
            let mut entries = cache.entries.write().await;
            entries.insert(
                42,
                CacheEntry {
                    raw_log: "cached log content".to_string(),
                    fetched_at: Instant::now(),
                    job_completed: false,
                },
            );
        }

        // Read it back — should be fresh (age << 5s refresh_interval)
        let entries = cache.entries.read().await;
        let entry = entries.get(&42).expect("entry should exist");
        let age = entry.fetched_at.elapsed();
        assert!(
            age <= cache.refresh_interval,
            "entry should be within refresh interval"
        );
        assert_eq!(entry.raw_log, "cached log content");
    }

    #[tokio::test]
    async fn test_mark_completed_and_evict() {
        let cache = StepLogCache {
            entries: Arc::new(RwLock::new(HashMap::new())),
            refresh_interval: Duration::from_secs(5),
            // Use a zero TTL so the entry expires immediately after mark_completed
            ttl_after_completion: Duration::from_secs(0),
        };

        // Insert an entry
        {
            let mut entries = cache.entries.write().await;
            entries.insert(
                99,
                CacheEntry {
                    raw_log: "some log".to_string(),
                    fetched_at: Instant::now(),
                    job_completed: false,
                },
            );
        }

        // Mark as completed — entry is now subject to TTL
        cache.mark_completed(99).await;

        // Small sleep to ensure elapsed() > 0s TTL
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Evict expired entries
        cache.evict_expired().await;

        // Entry should be gone
        let entries = cache.entries.read().await;
        assert!(
            !entries.contains_key(&99),
            "expired completed entry should have been evicted"
        );
    }
}
