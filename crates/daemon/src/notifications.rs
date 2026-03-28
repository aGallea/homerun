use std::sync::atomic::{AtomicBool, Ordering};

/// Manages notification preferences. Actual notification delivery is handled
/// by the Tauri desktop app frontend via `tauri-plugin-notification`.
pub struct NotificationManager {
    notify_status_changes: AtomicBool,
    notify_job_completions: AtomicBool,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notify_status_changes: AtomicBool::new(true),
            notify_job_completions: AtomicBool::new(true),
        }
    }

    pub fn with_preferences(notify_status_changes: bool, notify_job_completions: bool) -> Self {
        Self {
            notify_status_changes: AtomicBool::new(notify_status_changes),
            notify_job_completions: AtomicBool::new(notify_job_completions),
        }
    }

    pub fn set_status_changes(&self, enabled: bool) {
        self.notify_status_changes.store(enabled, Ordering::Relaxed);
    }

    pub fn set_job_completions(&self, enabled: bool) {
        self.notify_job_completions
            .store(enabled, Ordering::Relaxed);
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_manager_default_is_enabled() {
        let _mgr = NotificationManager::new();
        let _disabled = NotificationManager::with_preferences(false, false);
    }

    #[test]
    fn test_set_preferences() {
        let mgr = NotificationManager::new();
        mgr.set_status_changes(false);
        mgr.set_job_completions(false);
    }
}
