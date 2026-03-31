use std::path::Path;

// ── Script name helpers ──────────────────────────────────────────────

#[cfg(unix)]
pub fn runner_script(name: &str) -> String {
    format!("{name}.sh")
}

#[cfg(windows)]
pub fn runner_script(name: &str) -> String {
    format!("{name}.cmd")
}

pub fn run_script() -> String {
    runner_script("run")
}

pub fn config_script() -> String {
    runner_script("config")
}

// ── Process discovery ────────────────────────────────────────────────

#[cfg(unix)]
pub async fn find_runner_pids(dir_str: &str) -> Vec<u32> {
    use std::process::Stdio;
    use tokio::process::Command;

    let output = Command::new("pgrep")
        .args(["-f", dir_str])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(windows)]
pub async fn find_runner_pids(dir_str: &str) -> Vec<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let dir_lower = dir_str.to_lowercase();
    sys.processes()
        .iter()
        .filter(|(_pid, proc)| {
            proc.cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().to_lowercase().contains(&dir_lower))
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

#[cfg(unix)]
pub async fn find_runner_pid(runner_dir: &Path) -> Option<u32> {
    use std::process::Stdio;
    use tokio::process::Command;

    let dir_str = runner_dir.to_string_lossy();
    let pattern = format!("{}/{}", dir_str, run_script());

    let output = Command::new("pgrep")
        .args(["-f", &pattern])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .next()
}

#[cfg(windows)]
pub async fn find_runner_pid(runner_dir: &Path) -> Option<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let run = run_script();
    let dir_str = runner_dir.to_string_lossy().to_lowercase();

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    sys.processes()
        .iter()
        .find(|(_pid, proc)| {
            proc.cmd().iter().any(|arg| {
                let arg_lower = arg.to_string_lossy().to_lowercase();
                arg_lower.contains(&dir_str) && arg_lower.contains(&run)
            })
        })
        .map(|(pid, _)| pid.as_u32())
}

// ── Kill orphaned processes ──────────────────────────────────────────

#[cfg(unix)]
pub async fn kill_orphaned_processes(runner_dir: &Path) {
    let dir_str = runner_dir.to_string_lossy().to_string();

    let pids = find_runner_pids(&dir_str).await;
    if pids.is_empty() {
        return;
    }

    tracing::info!(
        "Killing {} orphaned process(es) for runner dir {}",
        pids.len(),
        dir_str
    );

    // SIGTERM the process groups for graceful shutdown
    for pid in &pids {
        let pid_i32 = *pid as i32;
        unsafe {
            libc::kill(-pid_i32, libc::SIGTERM);
            libc::kill(pid_i32, libc::SIGTERM);
        }
    }

    // Wait up to 5s for processes to die, checking every 500ms
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if find_runner_pids(&dir_str).await.is_empty() {
            tracing::info!("Orphaned processes terminated cleanly");
            return;
        }
    }

    // Force-kill any stragglers
    let remaining = find_runner_pids(&dir_str).await;
    if !remaining.is_empty() {
        tracing::warn!("Force-killing {} remaining process(es)", remaining.len());
        for pid in &remaining {
            let pid_i32 = *pid as i32;
            unsafe {
                libc::kill(-pid_i32, libc::SIGKILL);
                libc::kill(pid_i32, libc::SIGKILL);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(windows)]
pub async fn kill_orphaned_processes(runner_dir: &Path) {
    use std::process::Stdio;
    use tokio::process::Command;

    let dir_str = runner_dir.to_string_lossy().to_string();

    let pids = find_runner_pids(&dir_str).await;
    if pids.is_empty() {
        return;
    }

    tracing::info!(
        "Killing {} orphaned process(es) for runner dir {}",
        pids.len(),
        dir_str
    );

    for pid in &pids {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }

    // Wait up to 5s for processes to die, checking every 500ms
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if find_runner_pids(&dir_str).await.is_empty() {
            tracing::info!("Orphaned processes terminated cleanly");
            return;
        }
    }

    let remaining = find_runner_pids(&dir_str).await;
    if !remaining.is_empty() {
        tracing::warn!(
            "{} process(es) still alive after taskkill — may need manual cleanup",
            remaining.len()
        );
    }
}

// ── Process group configuration ──────────────────────────────────────

#[cfg(unix)]
pub fn configure_process_group(cmd: &mut tokio::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

#[cfg(windows)]
pub fn configure_process_group(cmd: &mut tokio::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_script_returns_platform_extension() {
        let script = runner_script("run");
        #[cfg(unix)]
        assert_eq!(script, "run.sh");
        #[cfg(windows)]
        assert_eq!(script, "run.cmd");
    }

    #[test]
    fn test_run_script() {
        let script = run_script();
        #[cfg(unix)]
        assert_eq!(script, "run.sh");
        #[cfg(windows)]
        assert_eq!(script, "run.cmd");
    }

    #[test]
    fn test_config_script() {
        let script = config_script();
        #[cfg(unix)]
        assert_eq!(script, "config.sh");
        #[cfg(windows)]
        assert_eq!(script, "config.cmd");
    }

    #[test]
    fn test_runner_script_custom_name() {
        let script = runner_script("setup");
        #[cfg(unix)]
        assert_eq!(script, "setup.sh");
        #[cfg(windows)]
        assert_eq!(script, "setup.cmd");
    }
}
