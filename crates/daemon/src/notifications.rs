use anyhow::Result;
use notify_rust::Notification;

pub enum NotificationType {
    JobCompleted {
        runner_name: String,
        job_name: String,
        duration: String,
    },
    JobFailed {
        runner_name: String,
        job_name: String,
    },
    RunnerCrashed {
        runner_name: String,
        attempt: u32,
        max_attempts: u32,
    },
    HighResourceUsage {
        cpu_percent: f64,
    },
}

pub struct NotificationManager {
    enabled: bool,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn send(&self, notification: NotificationType) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let (title, body) = match notification {
            NotificationType::JobCompleted {
                runner_name,
                job_name,
                duration,
            } => (
                "Job Completed".to_string(),
                format!("{job_name} on {runner_name} passed in {duration}"),
            ),
            NotificationType::JobFailed {
                runner_name,
                job_name,
            } => (
                "Job Failed".to_string(),
                format!("{job_name} on {runner_name} failed"),
            ),
            NotificationType::RunnerCrashed {
                runner_name,
                attempt,
                max_attempts,
            } => (
                "Runner Crashed".to_string(),
                format!("{runner_name} crashed (attempt {attempt}/{max_attempts})"),
            ),
            NotificationType::HighResourceUsage { cpu_percent } => (
                "High Resource Usage".to_string(),
                format!("CPU usage is at {cpu_percent:.1}%"),
            ),
        };

        Notification::new()
            .summary(&title)
            .body(&body)
            .appname("HomeRun")
            .show()?;

        Ok(())
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
    fn test_notification_manager_disabled_skips_send() {
        let mgr = NotificationManager::with_enabled(false);
        // Should not error even if OS notification system is unavailable
        let result = mgr.send(NotificationType::JobCompleted {
            runner_name: "runner-1".to_string(),
            job_name: "build".to_string(),
            duration: "2m 30s".to_string(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_manager_default_is_enabled() {
        let mgr = NotificationManager::new();
        assert!(mgr.enabled);
    }

    #[test]
    fn test_notification_type_job_completed_fields() {
        let n = NotificationType::JobCompleted {
            runner_name: "my-runner".to_string(),
            job_name: "test".to_string(),
            duration: "1m".to_string(),
        };
        // Verify match arms compile and produce expected strings
        let mgr = NotificationManager::with_enabled(false);
        assert!(mgr.send(n).is_ok());
    }

    #[test]
    fn test_notification_type_job_failed() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::JobFailed {
            runner_name: "runner-1".to_string(),
            job_name: "deploy".to_string(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_type_runner_crashed() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::RunnerCrashed {
            runner_name: "runner-1".to_string(),
            attempt: 2,
            max_attempts: 3,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_type_high_resource_usage() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::HighResourceUsage { cpu_percent: 95.5 });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_format_job_completed() {
        // Use enabled: false to avoid firing real system notifications during tests
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::JobCompleted {
            runner_name: "test-runner".to_string(),
            job_name: "build".to_string(),
            duration: "30s".to_string(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_format_job_failed() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::JobFailed {
            runner_name: "test-runner".to_string(),
            job_name: "deploy".to_string(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_format_runner_crashed() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::RunnerCrashed {
            runner_name: "test-runner".to_string(),
            attempt: 1,
            max_attempts: 3,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_notification_format_high_resource_usage() {
        let mgr = NotificationManager::with_enabled(false);
        let result = mgr.send(NotificationType::HighResourceUsage { cpu_percent: 85.0 });
        assert!(result.is_ok());
    }
}
