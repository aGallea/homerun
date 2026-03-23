use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a single job step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

/// Information about a single job step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub number: u16,
    pub name: String,
    pub status: StepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// API response for step progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsResponse {
    pub job_name: String,
    pub steps: Vec<StepInfo>,
    pub steps_discovered: usize,
}

/// Events parsed from Worker log lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepEvent {
    Discovered {
        name: String,
        timestamp: DateTime<Utc>,
    },
    Started {
        timestamp: DateTime<Utc>,
    },
    Completed {
        result: StepStatus,
        timestamp: DateTime<Utc>,
    },
}

/// Parse a Worker log timestamp in `YYYY-MM-DD HH:MM:SSZ` format.
fn parse_worker_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Expected: "2026-03-23 07:54:53Z" — strip trailing 'Z' for NaiveDateTime parse
    let trimmed = s.strip_suffix('Z')?;
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(naive.and_utc())
}

/// Parse a single Worker log line into a [`StepEvent`], if it matches a known pattern.
///
/// Expected patterns:
/// - `[<ts> INFO StepsRunner] Processing step: DisplayName='<name>'` → Discovered
/// - `[<ts> INFO StepsRunner] Starting the step.` → Started
/// - `[<ts> INFO StepsRunner] … current step result '<Result>'.` → Completed
pub fn parse_step_event(line: &str) -> Option<StepEvent> {
    // Early return if not a StepsRunner line
    if !line.contains("StepsRunner]") {
        return None;
    }

    // Extract timestamp from the leading bracket: [YYYY-MM-DD HH:MM:SSZ ...]
    let ts_start = line.find('[')? + 1;
    let ts_end = ts_start + "2026-03-23 07:54:53Z".len();
    if ts_end > line.len() {
        return None;
    }
    let timestamp = parse_worker_timestamp(&line[ts_start..ts_end])?;

    // Match patterns
    if let Some(idx) = line.find("DisplayName='") {
        let name_start = idx + "DisplayName='".len();
        let name_end = line[name_start..].find('\'')?;
        let name = line[name_start..name_start + name_end].to_string();
        return Some(StepEvent::Discovered { name, timestamp });
    }

    if line.contains("Starting the step.") {
        return Some(StepEvent::Started { timestamp });
    }

    if let Some(idx) = line.find("current step result '") {
        let result_start = idx + "current step result '".len();
        let result_end = line[result_start..].find('\'')?;
        let result_str = &line[result_start..result_start + result_end];
        let result = match result_str {
            "Succeeded" => StepStatus::Succeeded,
            "Failed" => StepStatus::Failed,
            "Skipped" => StepStatus::Skipped,
            _ => return None,
        };
        return Some(StepEvent::Completed { result, timestamp });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_step_discovered() {
        let line =
            "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Run actions/checkout@v6'";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Discovered { name, .. } => {
                assert_eq!(name, "Run actions/checkout@v6");
            }
            other => panic!("Expected Discovered, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_started() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Starting the step.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        assert!(matches!(event.unwrap(), StepEvent::Started { .. }));
    }

    #[test]
    fn test_parse_step_succeeded() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Succeeded'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Succeeded),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_failed() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] Updating job result with current step result 'Failed'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Failed),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_step_skipped() {
        let line = "[2026-03-23 07:54:55Z INFO StepsRunner] No need for updating job result with current step result 'Skipped'.";
        let event = parse_step_event(line);
        assert!(event.is_some());
        match event.unwrap() {
            StepEvent::Completed { result, .. } => assert_eq!(result, StepStatus::Skipped),
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unrelated_line_returns_none() {
        let line = "[2026-03-23 07:54:53Z INFO JobRunner] Starting job execution";
        assert!(parse_step_event(line).is_none());
    }

    #[test]
    fn test_parse_timestamp_extracted_correctly() {
        let line = "[2026-03-23 07:54:53Z INFO StepsRunner] Processing step: DisplayName='Build'";
        let event = parse_step_event(line).unwrap();
        let expected = Utc.with_ymd_and_hms(2026, 3, 23, 7, 54, 53).unwrap();
        match event {
            StepEvent::Discovered { timestamp, .. } => assert_eq!(timestamp, expected),
            other => panic!("Expected Discovered, got {:?}", other),
        }
    }
}
