use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub const RECENT_DAEMON_LOGS_MAX: usize = 2000;
pub const DAEMON_LOG_BROADCAST_CAPACITY: usize = 1024;
pub const DAEMON_LOG_FILE_MAX_BYTES: u64 = 10 * 1024 * 1024; // 10MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Clone)]
pub struct DaemonLogState {
    pub log_tx: Arc<broadcast::Sender<DaemonLogEntry>>,
    pub recent_logs: Arc<Mutex<VecDeque<DaemonLogEntry>>>,
    log_file_path: PathBuf,
}

impl DaemonLogState {
    pub fn new(log_dir: &Path) -> Self {
        let log_file_path = log_dir.join("daemon.log");

        // Rotate existing log file on startup
        if log_file_path.exists() {
            let backup = log_dir.join("daemon.log.1");
            let _ = fs::rename(&log_file_path, &backup);
        }

        let (log_tx, _) = broadcast::channel(DAEMON_LOG_BROADCAST_CAPACITY);

        Self {
            log_tx: Arc::new(log_tx),
            recent_logs: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_DAEMON_LOGS_MAX))),
            log_file_path,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DaemonLogEntry> {
        self.log_tx.subscribe()
    }

    pub async fn push(&self, entry: DaemonLogEntry) {
        // Broadcast to SSE subscribers
        let _ = self.log_tx.send(entry.clone());

        // Push to ring buffer
        let mut recent = self.recent_logs.lock().await;
        if recent.len() >= RECENT_DAEMON_LOGS_MAX {
            recent.pop_front();
        }
        recent.push_back(entry.clone());
        drop(recent);

        // Append to file
        self.append_to_file(&entry);
    }

    fn append_to_file(&self, entry: &DaemonLogEntry) {
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
        else {
            eprintln!("Failed to open daemon log file: {:?}", self.log_file_path);
            return;
        };

        // Check file size for mid-session rotation
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > DAEMON_LOG_FILE_MAX_BYTES {
                drop(file);
                let backup = self.log_file_path.with_extension("log.1");
                let _ = fs::rename(&self.log_file_path, &backup);
                let Ok(new_file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.log_file_path)
                else {
                    return;
                };
                file = new_file;
            }
        }

        if let Ok(json) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{}", json);
        }
    }

    pub async fn get_recent(
        &self,
        level: Option<&str>,
        limit: usize,
        search: Option<&str>,
    ) -> Vec<DaemonLogEntry> {
        let recent = self.recent_logs.lock().await;
        recent
            .iter()
            .filter(|e| {
                if let Some(min_level) = level {
                    level_value(&e.level) >= level_value(min_level)
                } else {
                    true
                }
            })
            .filter(|e| {
                if let Some(s) = search {
                    e.message.to_lowercase().contains(&s.to_lowercase())
                } else {
                    true
                }
            })
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

pub fn level_value(level: &str) -> u8 {
    match level.to_uppercase().as_str() {
        "ERROR" => 5,
        "WARN" => 4,
        "INFO" => 3,
        "DEBUG" => 2,
        "TRACE" => 1,
        _ => 0,
    }
}

pub struct DaemonLogLayer {
    state: DaemonLogState,
    runtime: tokio::runtime::Handle,
}

impl DaemonLogLayer {
    pub fn new(state: DaemonLogState, runtime: tokio::runtime::Handle) -> Self {
        Self { state, runtime }
    }
}

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

impl<S: Subscriber> Layer<S> for DaemonLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        let entry = DaemonLogEntry {
            timestamp: Utc::now(),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
        };

        let state = self.state.clone();
        self.runtime.spawn(async move {
            state.push(entry).await;
        });
    }
}
