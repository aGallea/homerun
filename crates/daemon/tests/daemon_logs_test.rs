use chrono::Utc;
use homerund::logging::{level_value, DaemonLogEntry, DaemonLogState};
use tempfile::TempDir;

#[tokio::test]
async fn test_push_and_get_recent() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    let entry = DaemonLogEntry {
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "hello world".to_string(),
    };
    state.push(entry).await;

    let recent = state.get_recent(None, 500, None).await;
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].message, "hello world");
}

#[tokio::test]
async fn test_level_filtering() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for (level, msg) in [
        ("DEBUG", "debug msg"),
        ("INFO", "info msg"),
        ("WARN", "warn msg"),
        ("ERROR", "error msg"),
    ] {
        state
            .push(DaemonLogEntry {
                timestamp: Utc::now(),
                level: level.to_string(),
                target: "test".to_string(),
                message: msg.to_string(),
            })
            .await;
    }

    let warn_and_above = state.get_recent(Some("WARN"), 500, None).await;
    assert_eq!(warn_and_above.len(), 2);
    assert_eq!(warn_and_above[0].level, "WARN");
    assert_eq!(warn_and_above[1].level, "ERROR");
}

#[tokio::test]
async fn test_text_search() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    state
        .push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Runner started successfully".to_string(),
        })
        .await;
    state
        .push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Auth token loaded".to_string(),
        })
        .await;

    let results = state.get_recent(None, 500, Some("runner")).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].message.contains("Runner"));
}

#[tokio::test]
async fn test_ring_buffer_cap() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for i in 0..2100 {
        state
            .push(DaemonLogEntry {
                timestamp: Utc::now(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg {}", i),
            })
            .await;
    }

    let recent = state.get_recent(None, 2000, None).await;
    assert_eq!(recent.len(), 2000);
    assert_eq!(recent[0].message, "msg 100");
}

#[tokio::test]
async fn test_broadcast_subscription() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());
    let mut rx = state.subscribe();

    state
        .push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "broadcast test".to_string(),
        })
        .await;

    let received = rx.recv().await.unwrap();
    assert_eq!(received.message, "broadcast test");
}

#[tokio::test]
async fn test_log_file_rotation_on_startup() {
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("daemon.log");
    std::fs::write(&log_path, "old log content").unwrap();

    let _state = DaemonLogState::new(tmp.path());

    let backup = tmp.path().join("daemon.log.1");
    assert!(backup.exists());
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "old log content");
}

#[tokio::test]
async fn test_get_recent_with_limit() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for i in 0..10 {
        state
            .push(DaemonLogEntry {
                timestamp: Utc::now(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg {}", i),
            })
            .await;
    }

    let recent = state.get_recent(None, 3, None).await;
    assert_eq!(recent.len(), 3);
    // limit takes the last N entries (most recent)
    assert_eq!(recent[0].message, "msg 7");
    assert_eq!(recent[1].message, "msg 8");
    assert_eq!(recent[2].message, "msg 9");
}

#[tokio::test]
async fn test_combined_level_and_search_filtering() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    for (level, msg) in [
        ("DEBUG", "runner started"),
        ("INFO", "runner connected"),
        ("WARN", "runner timeout"),
        ("ERROR", "runner crashed"),
        ("INFO", "auth token refreshed"),
        ("WARN", "disk space low"),
    ] {
        state
            .push(DaemonLogEntry {
                timestamp: Utc::now(),
                level: level.to_string(),
                target: "test".to_string(),
                message: msg.to_string(),
            })
            .await;
    }

    // WARN and above + search for "runner"
    let results = state.get_recent(Some("WARN"), 500, Some("runner")).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].message, "runner timeout");
    assert_eq!(results[1].message, "runner crashed");
}

#[test]
fn test_level_value_all_levels() {
    assert_eq!(level_value("TRACE"), 1);
    assert_eq!(level_value("DEBUG"), 2);
    assert_eq!(level_value("INFO"), 3);
    assert_eq!(level_value("WARN"), 4);
    assert_eq!(level_value("ERROR"), 5);
    // Case insensitive
    assert_eq!(level_value("info"), 3);
    assert_eq!(level_value("warn"), 4);
    // Unknown level returns 0
    assert_eq!(level_value("UNKNOWN"), 0);
    assert_eq!(level_value(""), 0);
    assert_eq!(level_value("FATAL"), 0);
}

#[tokio::test]
async fn test_log_file_is_written() {
    let tmp = TempDir::new().unwrap();
    let state = DaemonLogState::new(tmp.path());

    state
        .push(DaemonLogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test_target".to_string(),
            message: "file write test".to_string(),
        })
        .await;

    let log_path = tmp.path().join("daemon.log");
    assert!(log_path.exists(), "daemon.log should exist after push");

    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(!contents.is_empty(), "log file should not be empty");

    // Each line should be valid JSON
    let line = contents.lines().next().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(parsed["level"], "INFO");
    assert_eq!(parsed["target"], "test_target");
    assert_eq!(parsed["message"], "file write test");
}
