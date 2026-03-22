use std::collections::{HashSet, VecDeque};

use serde::Serialize;
use sysinfo::{Disks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub struct RingBuffer<T> {
    data: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.data.len() == self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(item);
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.data.iter().cloned()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerMetrics {
    pub runner_id: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonMetrics {
    pub pid: u32,
    pub uptime_seconds: u64,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub child_processes: Vec<ChildProcessInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChildProcessInfo {
    pub pid: u32,
    pub runner_id: String,
    pub runner_name: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

pub struct MetricsCollector {
    system: std::sync::Mutex<System>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            system: std::sync::Mutex::new(System::new()),
        }
    }

    pub fn system_snapshot(&self) -> SystemMetrics {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_percent = sys.global_cpu_usage() as f64;
        let memory_used_bytes = sys.used_memory();
        let memory_total_bytes = sys.total_memory();

        // Collect disk info
        let disks = Disks::new_with_refreshed_list();
        let disk_total_bytes: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let disk_available_bytes: u64 = disks.list().iter().map(|d| d.available_space()).sum();
        let disk_used_bytes = disk_total_bytes.saturating_sub(disk_available_bytes);

        SystemMetrics {
            cpu_percent,
            memory_used_bytes,
            memory_total_bytes,
            disk_used_bytes,
            disk_total_bytes,
        }
    }

    /// Refresh the process list. Call once before querying individual runners
    /// to avoid resetting CPU baselines between runners.
    pub fn refresh_processes(&self) {
        let mut sys = self.system.lock().unwrap();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
    }

    /// Collect metrics for a runner by aggregating its process tree.
    /// Call `refresh_processes()` once before calling this for each runner.
    pub fn runner_metrics(&self, pid: u32) -> Option<RunnerMetrics> {
        let sys = self.system.lock().unwrap();
        Self::runner_metrics_with_sys(&sys, pid)
    }

    /// Internal: collect runner metrics using an already-locked System reference.
    fn runner_metrics_with_sys(sys: &System, pid: u32) -> Option<RunnerMetrics> {
        let root_pid = Pid::from_u32(pid);

        // Check the root process exists
        sys.process(root_pid)?;

        // Build a parent→children index in one pass
        let mut children: std::collections::HashMap<Pid, Vec<Pid>> =
            std::collections::HashMap::new();
        for (pid, process) in sys.processes() {
            if let Some(parent) = process.parent() {
                children.entry(parent).or_default().push(*pid);
            }
        }

        // BFS from root_pid to collect all PIDs in the tree
        let mut tree_pids = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(root_pid);
        while let Some(current) = queue.pop_front() {
            if tree_pids.insert(current) {
                if let Some(kids) = children.get(&current) {
                    for &child in kids {
                        queue.push_back(child);
                    }
                }
            }
        }

        // Aggregate CPU and memory across the tree
        let mut total_cpu: f64 = 0.0;
        let mut total_memory: u64 = 0;
        for pid in &tree_pids {
            if let Some(process) = sys.process(*pid) {
                total_cpu += process.cpu_usage() as f64;
                total_memory += process.memory();
            }
        }

        Some(RunnerMetrics {
            runner_id: String::new(),
            cpu_percent: total_cpu,
            memory_bytes: total_memory,
        })
    }

    /// Collect metrics for the daemon process and its child runner processes.
    pub fn daemon_metrics(
        &self,
        daemon_pid: u32,
        uptime: std::time::Duration,
        runners: &[(String, String, Option<u32>)], // (runner_id, runner_name, pid)
    ) -> DaemonMetrics {
        let sys = self.system.lock().unwrap();
        let pid = Pid::from_u32(daemon_pid);

        let (cpu_percent, memory_bytes) = sys
            .process(pid)
            .map(|p| (p.cpu_usage() as f64, p.memory()))
            .unwrap_or((0.0, 0));

        let child_processes = runners
            .iter()
            .filter_map(|(id, name, runner_pid)| {
                runner_pid.and_then(|rpid| {
                    Self::runner_metrics_with_sys(&sys, rpid).map(|m| ChildProcessInfo {
                        pid: rpid,
                        runner_id: id.clone(),
                        runner_name: name.clone(),
                        cpu_percent: m.cpu_percent,
                        memory_bytes: m.memory_bytes,
                    })
                })
            })
            .collect();

        DaemonMetrics {
            pid: daemon_pid,
            uptime_seconds: uptime.as_secs(),
            cpu_percent,
            memory_bytes,
            child_processes,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_capacity() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1.0_f64);
        buffer.push(2.0_f64);
        buffer.push(3.0_f64);
        buffer.push(4.0_f64);
        let values: Vec<f64> = buffer.iter().collect();
        assert_eq!(values, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buffer: RingBuffer<f64> = RingBuffer::new(5);
        let values: Vec<f64> = buffer.iter().collect();
        assert!(values.is_empty());
    }

    #[test]
    fn test_ring_buffer_under_capacity() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(10_i32);
        buffer.push(20_i32);
        let values: Vec<i32> = buffer.iter().collect();
        assert_eq!(values, vec![10, 20]);
    }

    #[test]
    fn test_ring_buffer_exactly_at_capacity() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1_i32);
        buffer.push(2_i32);
        buffer.push(3_i32);
        let values: Vec<i32> = buffer.iter().collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_ring_buffer_overwrites_oldest() {
        let mut buffer = RingBuffer::new(2);
        buffer.push("a");
        buffer.push("b");
        buffer.push("c");
        let values: Vec<&str> = buffer.iter().collect();
        assert_eq!(values, vec!["b", "c"]);
    }

    #[test]
    fn test_ring_buffer_capacity_one() {
        let mut buffer = RingBuffer::new(1);
        buffer.push(42_i32);
        buffer.push(99_i32);
        let values: Vec<i32> = buffer.iter().collect();
        assert_eq!(values, vec![99]);
    }

    #[test]
    fn test_system_metrics_snapshot() {
        let collector = MetricsCollector::new();
        let metrics = collector.system_snapshot();
        assert!(metrics.cpu_percent >= 0.0);
        assert!(metrics.memory_total_bytes > 0);
    }

    #[test]
    fn test_metrics_collector_default() {
        let collector = MetricsCollector::default();
        let metrics = collector.system_snapshot();
        assert!(metrics.memory_total_bytes > 0);
    }

    #[test]
    fn test_runner_metrics_includes_child_processes() {
        use std::process::Command;

        // Spawn a shell that itself spawns children — creates a process tree.
        // Use short sleep durations since we kill the process group after.
        let mut parent = Command::new("sh")
            .arg("-c")
            .arg("sleep 10 & sleep 10 & sleep 10 & wait")
            .spawn()
            .expect("failed to spawn parent process");

        let parent_pid = parent.id();

        // Give children time to spawn
        std::thread::sleep(std::time::Duration::from_millis(500));

        let collector = MetricsCollector::new();
        collector.refresh_processes();
        let metrics = collector.runner_metrics(parent_pid);

        // Kill parent and orphaned children via pkill
        let _ = std::process::Command::new("pkill")
            .args(["-P", &parent_pid.to_string()])
            .status();
        let _ = parent.kill();
        let _ = parent.wait();

        let metrics = metrics.expect("should find the parent process");
        // The aggregated memory should be > 0 (includes parent + children)
        assert!(metrics.memory_bytes > 0, "aggregated memory should be > 0");
    }

    #[test]
    fn test_runner_metrics_nonexistent_pid_returns_none() {
        let collector = MetricsCollector::new();
        // PID 0 should never be a valid user process
        let result = collector.runner_metrics(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_runner_metrics_very_large_pid_returns_none() {
        let collector = MetricsCollector::new();
        // Use a very large PID that almost certainly doesn't exist
        let result = collector.runner_metrics(u32::MAX);
        assert!(result.is_none());
    }

    #[test]
    fn test_system_metrics_disk_bytes_are_sane() {
        let collector = MetricsCollector::new();
        let metrics = collector.system_snapshot();
        // disk_used_bytes should not exceed disk_total_bytes
        assert!(metrics.disk_used_bytes <= metrics.disk_total_bytes);
    }

    #[test]
    fn test_system_metrics_memory_used_not_exceeds_total() {
        let collector = MetricsCollector::new();
        let metrics = collector.system_snapshot();
        assert!(metrics.memory_used_bytes <= metrics.memory_total_bytes);
    }
}
