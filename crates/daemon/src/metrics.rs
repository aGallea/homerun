use std::collections::VecDeque;

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

    pub fn runner_metrics(&self, pid: u32) -> Option<RunnerMetrics> {
        let mut sys = self.system.lock().unwrap();
        let sysinfo_pid = Pid::from_u32(pid);
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[sysinfo_pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        sys.process(sysinfo_pid).map(|p| RunnerMetrics {
            runner_id: String::new(),
            cpu_percent: p.cpu_usage() as f64,
            memory_bytes: p.memory(),
        })
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
    fn test_system_metrics_snapshot() {
        let collector = MetricsCollector::new();
        let metrics = collector.system_snapshot();
        assert!(metrics.cpu_percent >= 0.0);
        assert!(metrics.memory_total_bytes > 0);
    }
}
