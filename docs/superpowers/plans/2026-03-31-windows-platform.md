# Windows Platform Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make HomeRun's daemon, TUI, and desktop client run on Windows using named pipes for IPC, Task Scheduler for auto-start, and sysinfo/taskkill for process management.

**Architecture:** Create a `crates/daemon/src/platform/` module with `#[cfg(unix)]`/`#[cfg(windows)]`-gated submodules for `shell`, `process`, `service`, and `ipc`. Move existing Unix-specific code into these modules and add Windows counterparts. Update callers (server.rs, runner/mod.rs, runner/binary.rs, clients, daemon_lifecycle) to use the platform module.

**Tech Stack:** Rust, tokio (named pipes), sysinfo, zip crate, schtasks.exe

**Spec:** `docs/superpowers/specs/2026-03-31-windows-platform-design.md`

---

## File Structure

### New files
| File | Responsibility |
|---|---|
| `crates/daemon/src/platform/mod.rs` | Re-exports all platform submodules |
| `crates/daemon/src/platform/shell.rs` | Shell PATH resolution (Unix: login shell, Windows: None) |
| `crates/daemon/src/platform/process.rs` | Process discovery, killing, process groups, runner script names |
| `crates/daemon/src/platform/service.rs` | Auto-start install/uninstall/status (launchd on macOS, schtasks on Windows) |
| `crates/daemon/src/platform/ipc.rs` | Named pipe listener (Windows), Unix socket helpers, client connectors |

### Modified files
| File | What changes |
|---|---|
| `crates/daemon/src/lib.rs` | Add `pub mod platform;`, keep `launchd` as `#[cfg(unix)]` for now |
| `crates/daemon/Cargo.toml` | Add `zip` dependency (Windows-only) |
| `crates/daemon/src/runner/process.rs` | Delegate to `platform::process` and `platform::shell` |
| `crates/daemon/src/runner/binary.rs` | Cross-platform `detect_platform()`, `.zip` extraction, `run.cmd` check |
| `crates/daemon/src/runner/mod.rs` | Use `platform::process::runner_script()` for script names |
| `crates/daemon/src/server.rs` | `#[cfg]`-gated serve: Unix socket vs named pipe, shutdown signals |
| `crates/daemon/src/config.rs` | Add `pipe_name()` for Windows |
| `crates/daemon/src/api/service.rs` | Delegate to `platform::service` instead of `crate::launchd` |
| `crates/tui/src/client.rs` | Add `NamedPipeConnector`, platform-aware `DaemonClient` |
| `crates/tui/src/daemon_lifecycle.rs` | Platform-aware IPC endpoint, error messages |
| `apps/desktop/src-tauri/src/client.rs` | Add `NamedPipeConnector`, platform-aware `DaemonClient` |

---

## Task 1: Create platform::shell module

**Files:**
- Create: `crates/daemon/src/platform/mod.rs`
- Create: `crates/daemon/src/platform/shell.rs`
- Modify: `crates/daemon/src/lib.rs:1-12`

This is the simplest platform module — start here to establish the pattern.

- [ ] **Step 1: Create `platform/mod.rs` with shell submodule**

```rust
// crates/daemon/src/platform/mod.rs
pub mod shell;
```

- [ ] **Step 2: Create `platform/shell.rs` with Unix implementation (move from process.rs)**

Move the `resolve_shell_path()` function and `SHELL_PATH` lazy static from `crates/daemon/src/runner/process.rs:9-32` into the new module. Add a Windows stub that returns `None`.

```rust
// crates/daemon/src/platform/shell.rs
use std::process::Stdio;

/// Cached shell PATH resolved once at first use.
pub static SHELL_PATH: std::sync::LazyLock<Option<String>> = std::sync::LazyLock::new(|| {
    let path = resolve_shell_path();
    if let Some(ref p) = path {
        tracing::info!("Resolved shell PATH: {p}");
    }
    path
});

/// Resolve the full PATH from the user's login shell.
/// On Unix, this picks up paths added by nvm, fnm, Homebrew, etc.
/// On Windows, returns None — PATH is inherited from the system environment.
#[cfg(unix)]
pub fn resolve_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(&shell)
        .args(["-l", "-c", "echo $PATH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(windows)]
pub fn resolve_shell_path() -> Option<String> {
    // Windows resolves PATH from the system environment automatically.
    // No shell PATH override needed.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_shell_path_returns_option() {
        // On any platform, this should not panic
        let result = resolve_shell_path();
        // On Unix it should return Some with a non-empty string containing /usr/bin
        #[cfg(unix)]
        {
            let path = result.expect("should resolve PATH on Unix");
            assert!(path.contains("/usr"), "PATH should contain /usr: {path}");
        }
        // On Windows it should return None
        #[cfg(windows)]
        assert!(result.is_none(), "Windows should return None");
    }
}
```

- [ ] **Step 3: Register the platform module in lib.rs**

Add `pub mod platform;` to `crates/daemon/src/lib.rs`. Insert it alphabetically:

In `crates/daemon/src/lib.rs`, add after `pub mod notifications;`:
```rust
pub mod platform;
```

- [ ] **Step 4: Update `runner/process.rs` to use `platform::shell`**

In `crates/daemon/src/runner/process.rs`, remove the `resolve_shell_path()` function (lines 6-23) and `SHELL_PATH` static (lines 25-32). Replace with an import:

```rust
use crate::platform::shell::SHELL_PATH;
```

All existing references to `SHELL_PATH` in `process.rs` (lines 55 and 191) remain unchanged — they already dereference the `LazyLock`.

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All existing tests pass. The `resolve_shell_path` and `SHELL_PATH` behavior is identical.

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/platform/ crates/daemon/src/lib.rs crates/daemon/src/runner/process.rs
git commit -m "refactor: extract platform::shell module from runner/process.rs (#112)"
```

---

## Task 2: Create platform::process module

**Files:**
- Create: `crates/daemon/src/platform/process.rs`
- Modify: `crates/daemon/src/platform/mod.rs`
- Modify: `crates/daemon/src/runner/process.rs`

Move process discovery, killing, process group setup, and runner script name logic into `platform::process`. The existing `runner/process.rs` becomes a thin wrapper.

- [ ] **Step 1: Add process to platform/mod.rs**

```rust
// crates/daemon/src/platform/mod.rs
pub mod process;
pub mod shell;
```

- [ ] **Step 2: Create `platform/process.rs` with `runner_script()` helper**

```rust
// crates/daemon/src/platform/process.rs

/// Returns the platform-specific runner script filename.
/// "run" -> "run.sh" (Unix) or "run.cmd" (Windows)
/// "config" -> "config.sh" (Unix) or "config.cmd" (Windows)
#[cfg(unix)]
pub fn runner_script(name: &str) -> String {
    format!("{name}.sh")
}

#[cfg(windows)]
pub fn runner_script(name: &str) -> String {
    format!("{name}.cmd")
}

/// Returns the platform-specific runner executable script filename.
/// On Windows, the runner uses `run.cmd`; on Unix, `run.sh`.
pub fn run_script() -> String {
    runner_script("run")
}

/// Returns the platform-specific runner config script filename.
pub fn config_script() -> String {
    runner_script("config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_script_run() {
        let script = runner_script("run");
        #[cfg(unix)]
        assert_eq!(script, "run.sh");
        #[cfg(windows)]
        assert_eq!(script, "run.cmd");
    }

    #[test]
    fn test_runner_script_config() {
        let script = runner_script("config");
        #[cfg(unix)]
        assert_eq!(script, "config.sh");
        #[cfg(windows)]
        assert_eq!(script, "config.cmd");
    }

    #[test]
    fn test_run_script_helper() {
        let script = run_script();
        #[cfg(unix)]
        assert_eq!(script, "run.sh");
        #[cfg(windows)]
        assert_eq!(script, "run.cmd");
    }

    #[test]
    fn test_config_script_helper() {
        let script = config_script();
        #[cfg(unix)]
        assert_eq!(script, "config.sh");
        #[cfg(windows)]
        assert_eq!(script, "config.cmd");
    }
}
```

- [ ] **Step 3: Add `find_runner_pids` and `find_runner_pid` to platform::process**

Append to `crates/daemon/src/platform/process.rs`:

```rust
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Find PIDs of processes associated with a runner directory.
#[cfg(unix)]
pub async fn find_runner_pids(dir_str: &str) -> Vec<u32> {
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
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .filter(|p| {
            let cmd_line = p.cmd().join(std::ffi::OsStr::new(" "));
            cmd_line.to_string_lossy().contains(dir_str)
        })
        .map(|p| p.pid().as_u32())
        .collect()
}

/// Find the session-leader PID (run.sh/run.cmd) for a runner directory, if still alive.
#[cfg(unix)]
pub async fn find_runner_pid(runner_dir: &Path) -> Option<u32> {
    let dir_str = runner_dir.to_string_lossy();
    let output = Command::new("pgrep")
        .args(["-f", &format!("{}/{}", dir_str, run_script())])
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
    use sysinfo::System;
    let dir_str = runner_dir.to_string_lossy();
    let script = run_script();
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .find(|p| {
            let cmd_line = p.cmd().join(std::ffi::OsStr::new(" "));
            let cmd_str = cmd_line.to_string_lossy();
            cmd_str.contains(dir_str.as_ref()) && cmd_str.contains(&script)
        })
        .map(|p| p.pid().as_u32())
}
```

- [ ] **Step 4: Add `kill_orphaned_processes` to platform::process**

Append to `crates/daemon/src/platform/process.rs`:

```rust
/// Kill any orphaned runner processes from a previous daemon session.
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
        unsafe {
            libc::kill(-(*pid as i32), libc::SIGTERM);
            libc::kill(*pid as i32, libc::SIGTERM);
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
            unsafe {
                libc::kill(-(*pid as i32), libc::SIGKILL);
                libc::kill(*pid as i32, libc::SIGKILL);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

#[cfg(windows)]
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

    // Use taskkill /T /F to kill process trees
    for pid in &pids {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }

    // Wait up to 5s for processes to die
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if find_runner_pids(&dir_str).await.is_empty() {
            tracing::info!("Orphaned processes terminated cleanly");
            return;
        }
    }
}
```

- [ ] **Step 5: Add `configure_process_group` to platform::process**

Append to `crates/daemon/src/platform/process.rs`:

```rust
/// Configure a Command to spawn in its own process group.
/// On Unix: setsid() so we can signal the entire tree.
/// On Windows: CREATE_NEW_PROCESS_GROUP flag.
#[cfg(unix)]
pub fn configure_process_group(cmd: &mut Command) {
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

#[cfg(windows)]
pub fn configure_process_group(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NEW_PROCESS_GROUP = 0x00000200
    cmd.creation_flags(0x00000200);
}
```

- [ ] **Step 6: Update `runner/process.rs` to delegate to `platform::process`**

Rewrite `crates/daemon/src/runner/process.rs` to import from platform and remove the duplicated logic. The file keeps `configure_runner`, `start_runner`, `remove_runner`, `clean_runner_config` (these call runner scripts and use platform helpers), but delegates process discovery and killing to platform:

Replace the imports at the top of `crates/daemon/src/runner/process.rs` (lines 1-4):
```rust
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

use crate::platform::process::{
    configure_process_group, runner_script, run_script, config_script,
};
use crate::platform::shell::SHELL_PATH;
```

Remove `resolve_shell_path()` (lines 6-23), `SHELL_PATH` (lines 25-32), `kill_orphaned_processes()` (lines 94-140), `find_runner_pids()` (lines 142-157), `find_runner_pid()` (lines 159-179) — these are all now in `platform::process`.

Re-export from platform so `runner/mod.rs` imports still work:
```rust
// Re-export platform functions used by runner/mod.rs
pub use crate::platform::process::{find_runner_pid, kill_orphaned_processes};
```

Update `configure_runner()` to use `config_script()`:
```rust
pub async fn configure_runner(
    runner_dir: &Path,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
) -> Result<()> {
    for file in &[".runner", ".credentials", ".credentials_rsaparams"] {
        let path = runner_dir.join(file);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    let labels_str = labels.join(",");
    let dir_str = runner_dir.to_string_lossy().to_string();
    let mut config_cmd = Command::new(runner_dir.join(config_script()));
    config_cmd
        .env("HOMERUN_RUNNER_DIR", &dir_str)
        .env("HOMERUN_MANAGED", "1");
    if let Some(ref path) = *SHELL_PATH {
        config_cmd.env("PATH", path);
    }
    let output = config_cmd
        .args([
            "--url", url, "--token", token, "--name", name,
            "--labels", &labels_str, "--unattended", "--replace",
        ])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        anyhow::bail!(
            "{} failed (exit {}): {}",
            config_script(),
            output.status.code().unwrap_or(-1),
            detail
        );
    }
    Ok(())
}
```

Update `start_runner()` to use `run_script()` and `configure_process_group()`:
```rust
pub async fn start_runner(runner_dir: &Path) -> Result<Child> {
    let dir_str = runner_dir.to_string_lossy().to_string();
    let mut cmd = Command::new(runner_dir.join(run_script()));
    cmd.current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HOMERUN_RUNNER_DIR", &dir_str)
        .env("HOMERUN_MANAGED", "1");
    if let Some(ref path) = *SHELL_PATH {
        cmd.env("PATH", path);
    }

    configure_process_group(&mut cmd);

    let child = cmd.spawn()?;
    Ok(child)
}
```

Update `remove_runner()` to use `config_script()`:
```rust
pub async fn remove_runner(runner_dir: &Path, token: &str) -> Result<()> {
    let status = Command::new(runner_dir.join(config_script()))
        .args(["remove", "--token", token])
        .current_dir(runner_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await?;

    if !status.success() {
        tracing::warn!("{} remove failed — runner may need manual cleanup on GitHub", config_script());
    }
    Ok(())
}
```

`clean_runner_config()` does not reference any scripts — leave it unchanged.

- [ ] **Step 7: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All existing tests pass. The process functions behave identically.

- [ ] **Step 8: Commit**

```bash
git add crates/daemon/src/platform/process.rs crates/daemon/src/platform/mod.rs crates/daemon/src/runner/process.rs
git commit -m "refactor: extract platform::process module with Windows support (#112)"
```

---

## Task 3: Create platform::service module

**Files:**
- Create: `crates/daemon/src/platform/service.rs`
- Modify: `crates/daemon/src/platform/mod.rs`
- Modify: `crates/daemon/src/api/service.rs:12-14, 27-28, 34`
- Modify: `crates/daemon/src/lib.rs`

Move launchd logic into `platform::service` and add Windows Task Scheduler implementation.

- [ ] **Step 1: Add service to platform/mod.rs**

```rust
// crates/daemon/src/platform/mod.rs
pub mod process;
pub mod service;
pub mod shell;
```

- [ ] **Step 2: Create `platform/service.rs`**

Move the contents of `crates/daemon/src/launchd.rs` into the Unix `#[cfg]` block and add the Windows `schtasks` implementation:

```rust
// crates/daemon/src/platform/service.rs
use anyhow::{Context, Result};
use std::path::Path;

// ── macOS: launchd ──────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::path::PathBuf;

    const PLIST_LABEL: &str = "com.homerun.daemon";
    const PLIST_FILENAME: &str = "com.homerun.daemon.plist";

    fn plist_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join("Library/LaunchAgents").join(PLIST_FILENAME))
    }

    fn home_dir_str() -> Result<String> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.display().to_string())
    }

    fn resolve_shell_path() -> String {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .args(["-l", "-c", "echo $PATH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
        "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
    }

    fn build_plist(daemon_path: &Path) -> Result<String> {
        let home = home_dir_str()?;
        let path = resolve_shell_path();
        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{home}/logs/daemon.log</string>
    <key>StandardErrorPath</key>
    <string>{home}/logs/daemon.err</string>
</dict>
</plist>"#,
            daemon_path.display(),
        ))
    }

    pub fn install_daemon_service(daemon_path: &Path) -> Result<()> {
        let plist_path = plist_path()?;
        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create LaunchAgents directory: {}", parent.display())
            })?;
        }
        let plist = build_plist(daemon_path)?;
        std::fs::write(&plist_path, &plist)
            .with_context(|| format!("Failed to write plist to {}", plist_path.display()))?;
        tracing::info!("Wrote launchd plist to {}", plist_path.display());

        let status = std::process::Command::new("launchctl")
            .arg("load").arg("-w").arg(&plist_path)
            .status()
            .context("Failed to run launchctl load")?;
        if !status.success() {
            anyhow::bail!("launchctl load failed with exit code: {}", status);
        }
        tracing::info!("Daemon service installed and loaded via launchd");
        Ok(())
    }

    pub fn uninstall_daemon_service() -> Result<()> {
        let plist_path = plist_path()?;
        if plist_path.exists() {
            let status = std::process::Command::new("launchctl")
                .arg("unload").arg("-w").arg(&plist_path)
                .status()
                .context("Failed to run launchctl unload")?;
            if !status.success() {
                tracing::warn!("launchctl unload exited with: {}", status);
            }
            std::fs::remove_file(&plist_path)
                .with_context(|| format!("Failed to remove plist at {}", plist_path.display()))?;
            tracing::info!("Daemon service uninstalled");
        } else {
            tracing::info!("No plist found at {} — nothing to uninstall", plist_path.display());
        }
        Ok(())
    }

    pub fn is_daemon_installed() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }
}

// ── Windows: Task Scheduler ─────────────────────────────────────────

#[cfg(windows)]
mod windows {
    use super::*;

    const TASK_NAME: &str = "HomeRun Daemon";

    pub fn install_daemon_service(daemon_path: &Path) -> Result<()> {
        let daemon_str = daemon_path.display().to_string();
        let status = std::process::Command::new("schtasks")
            .args([
                "/Create", "/SC", "ONLOGON",
                "/TN", TASK_NAME,
                "/TR", &format!("\"{}\"", daemon_str),
                "/RL", "HIGHEST",
                "/F",
            ])
            .status()
            .context("Failed to run schtasks /Create")?;

        if !status.success() {
            anyhow::bail!("schtasks /Create failed with exit code: {}", status);
        }
        tracing::info!("Daemon service installed via Task Scheduler");
        Ok(())
    }

    pub fn uninstall_daemon_service() -> Result<()> {
        let status = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .status()
            .context("Failed to run schtasks /Delete")?;

        if !status.success() {
            tracing::warn!("schtasks /Delete exited with: {}", status);
        }
        tracing::info!("Daemon service uninstalled from Task Scheduler");
        Ok(())
    }

    pub fn is_daemon_installed() -> bool {
        std::process::Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

// ── Linux: stub (no auto-start yet) ────────────────────────────────

#[cfg(all(unix, not(target_os = "macos")))]
mod linux_stub {
    use super::*;

    pub fn install_daemon_service(_daemon_path: &Path) -> Result<()> {
        anyhow::bail!("Auto-start is not yet supported on Linux. See issue #112.");
    }

    pub fn uninstall_daemon_service() -> Result<()> {
        anyhow::bail!("Auto-start is not yet supported on Linux. See issue #112.");
    }

    pub fn is_daemon_installed() -> bool {
        false
    }
}

// ── Public re-exports ───────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(windows)]
pub use windows::*;

#[cfg(all(unix, not(target_os = "macos")))]
pub use linux_stub::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_daemon_installed_returns_bool() {
        // Should not panic on any platform
        let _ = is_daemon_installed();
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_uninstall_when_not_installed() {
        // Should not panic — schtasks /Delete may warn but function handles it
        let result = uninstall_daemon_service();
        // Result may be Ok or Err depending on whether task exists
        let _ = result;
    }
}
```

- [ ] **Step 3: Update `api/service.rs` to use `platform::service`**

In `crates/daemon/src/api/service.rs`, replace the three `crate::launchd::` calls:

Line 12: `crate::launchd::install_daemon_service(&daemon_path)` → `crate::platform::service::install_daemon_service(&daemon_path)`

Line 27: `crate::launchd::uninstall_daemon_service()` → `crate::platform::service::uninstall_daemon_service()`

Line 34: `crate::launchd::is_daemon_installed()` → `crate::platform::service::is_daemon_installed()`

- [ ] **Step 4: Mark `launchd.rs` as deprecated/cfg-gated in `lib.rs`**

In `crates/daemon/src/lib.rs`, gate the old module so it only compiles on macOS (for backward compat with any direct imports), or simply remove it since `api/service.rs` was the only consumer:

Replace `pub mod launchd;` with:
```rust
#[cfg(target_os = "macos")]
pub mod launchd; // Deprecated: use platform::service instead
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All tests pass. The `api/service.rs` tests exercise the same logic via the new path.

- [ ] **Step 6: Commit**

```bash
git add crates/daemon/src/platform/service.rs crates/daemon/src/platform/mod.rs crates/daemon/src/api/service.rs crates/daemon/src/lib.rs
git commit -m "refactor: extract platform::service with Windows Task Scheduler support (#112)"
```

---

## Task 4: Cross-platform binary download and extraction

**Files:**
- Modify: `crates/daemon/src/runner/binary.rs`
- Modify: `crates/daemon/Cargo.toml`

Update `detect_platform()` for Windows, add `.zip` extraction, and use `platform::process::run_script()` for cache-hit checks.

- [ ] **Step 1: Add `zip` dependency to daemon Cargo.toml**

In `crates/daemon/Cargo.toml`, add to `[dependencies]`:
```toml
zip = { version = "2", default-features = false, features = ["deflate"], optional = true }
```

Add a `[target]` section for Windows-only activation:
```toml
[target.'cfg(windows)'.dependencies]
zip = { version = "2", default-features = false, features = ["deflate"] }
```

Actually, since we are using `optional = true` with cfg, it's simpler to just add `zip` unconditionally and use `#[cfg]` in code. The crate is small. Use:
```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

- [ ] **Step 2: Update `detect_platform()` in `binary.rs`**

Replace the `detect_platform()` function in `crates/daemon/src/runner/binary.rs:18-26`:

```rust
/// Returns (os, arch) for the current platform.
pub fn detect_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "osx"
    } else if cfg!(target_os = "windows") {
        "win"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    (os, arch)
}
```

- [ ] **Step 3: Update `runner_download_url()` to handle `.zip` vs `.tar.gz`**

Replace the `runner_download_url()` function in `binary.rs:9-13`:

```rust
/// Constructs the GitHub Actions runner download URL for the given version, OS, and architecture.
pub fn runner_download_url(version: &str, os: &str, arch: &str) -> String {
    let ext = if os == "win" { "zip" } else { "tar.gz" };
    format!(
        "https://github.com/actions/runner/releases/download/v{version}/actions-runner-{os}-{arch}-{version}.{ext}"
    )
}
```

- [ ] **Step 4: Update `ensure_runner_binary()` for cross-platform extraction**

In `crates/daemon/src/runner/binary.rs`, update the `ensure_runner_binary()` function.

Add import at top of file:
```rust
use crate::platform::process::run_script;
```

Replace the `run.sh` check (line 54) with:
```rust
    let run_script_path = runner_dir.join(run_script());
```

Replace `if run_sh.exists()` (line 57 and 67) with `if run_script_path.exists()`.

Replace the archive filename construction (line 88):
```rust
    let ext = if os == "win" { "zip" } else { "tar.gz" };
    let archive_path = runner_dir.join(format!("actions-runner-{os}-{arch}-{version}.{ext}"));
```

Replace the extraction block (lines 101-108) with platform-gated extraction:

```rust
    #[cfg(unix)]
    {
        tracing::info!("Extracting runner archive to {:?}", runner_dir);
        let status = tokio::process::Command::new("tar")
            .arg("xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&runner_dir)
            .status()
            .await
            .context("Failed to run tar to extract runner archive")?;
        if !status.success() {
            anyhow::bail!("tar extraction failed with status: {}", status);
        }
    }

    #[cfg(windows)]
    {
        tracing::info!("Extracting runner zip to {:?}", runner_dir);
        let archive_path_clone = archive_path.clone();
        let runner_dir_clone = runner_dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let file = std::fs::File::open(&archive_path_clone)
                .with_context(|| format!("Failed to open archive {:?}", archive_path_clone))?;
            let mut archive = zip::ZipArchive::new(file)
                .context("Failed to read zip archive")?;
            archive.extract(&runner_dir_clone)
                .context("Failed to extract zip archive")?;
            Ok(())
        })
        .await
        .context("Zip extraction task panicked")??;
    }
```

- [ ] **Step 5: Update tests in `binary.rs`**

Update `test_detect_platform` tests to be cross-platform:

```rust
    #[test]
    fn test_detect_platform() {
        let (os, arch) = detect_platform();
        #[cfg(target_os = "macos")]
        assert_eq!(os, "osx");
        #[cfg(target_os = "windows")]
        assert_eq!(os, "win");
        #[cfg(target_os = "linux")]
        assert_eq!(os, "linux");
        assert!(arch == "arm64" || arch == "x64");
    }

    #[test]
    fn test_detect_platform_os_is_correct() {
        let (os, _arch) = detect_platform();
        if cfg!(target_os = "macos") {
            assert_eq!(os, "osx");
        } else if cfg!(target_os = "windows") {
            assert_eq!(os, "win");
        } else {
            assert_eq!(os, "linux");
        }
    }
```

Update the `test_download_url_macos_arm64` etc. to also have a Windows variant:

```rust
    #[test]
    fn test_download_url_windows_x64() {
        let url = runner_download_url("2.321.0", "win", "x64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-win-x64-2.321.0.zip"
        );
    }

    #[test]
    fn test_download_url_linux_x64() {
        let url = runner_download_url("2.321.0", "linux", "x64");
        assert_eq!(
            url,
            "https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-linux-x64-2.321.0.tar.gz"
        );
    }
```

Update the cache-hit test to use `run_script()` instead of hardcoded `"run.sh"`:

```rust
    #[tokio::test]
    async fn test_ensure_runner_binary_cache_hit_returns_early() {
        use std::fs;
        use crate::platform::process::run_script;

        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let cache_dir = tmp.path();

        let version = "2.999.0";
        let runner_dir = cache_dir.join(format!("runner-{version}"));
        fs::create_dir_all(&runner_dir).expect("failed to create runner dir");
        fs::write(runner_dir.join(run_script()), "#!/bin/bash\necho runner")
            .expect("failed to write run script");

        let run_script_path = runner_dir.join(run_script());
        assert!(run_script_path.exists(), "run script should exist in simulated cache");

        let expected_runner_dir = cache_dir.join(format!("runner-{version}"));
        assert_eq!(runner_dir, expected_runner_dir);
    }
```

- [ ] **Step 6: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All tests pass, including the new Windows URL test.

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/Cargo.toml crates/daemon/src/runner/binary.rs
git commit -m "feat: cross-platform binary download with .zip extraction for Windows (#112)"
```

---

## Task 5: Platform-aware IPC — Config and daemon server

**Files:**
- Create: `crates/daemon/src/platform/ipc.rs`
- Modify: `crates/daemon/src/platform/mod.rs`
- Modify: `crates/daemon/src/config.rs:65-66`
- Modify: `crates/daemon/src/server.rs`

This is the most complex task — adding named pipe listener support for Windows.

- [ ] **Step 1: Add ipc to platform/mod.rs**

```rust
// crates/daemon/src/platform/mod.rs
pub mod ipc;
pub mod process;
pub mod service;
pub mod shell;
```

- [ ] **Step 2: Create `platform/ipc.rs` with Windows named pipe listener**

```rust
// crates/daemon/src/platform/ipc.rs

/// The well-known pipe name for the HomeRun daemon on Windows.
#[cfg(windows)]
pub const PIPE_NAME: &str = r"\\.\pipe\homerun-daemon";

/// Check if another daemon is reachable on the IPC endpoint.
#[cfg(unix)]
pub async fn is_daemon_reachable(socket_path: &std::path::Path) -> bool {
    tokio::net::UnixStream::connect(socket_path).await.is_ok()
}

#[cfg(windows)]
pub async fn is_daemon_reachable(_socket_path: &std::path::Path) -> bool {
    // Try to connect to the named pipe
    tokio::net::windows::named_pipe::ClientOptions::new()
        .open(PIPE_NAME)
        .is_ok()
}

// ── Windows Named Pipe Listener ─────────────────────────────────────

#[cfg(windows)]
pub mod named_pipe {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::net::windows::named_pipe::{ServerOptions, NamedPipeServer};

    /// A listener that accepts connections on a Windows named pipe,
    /// compatible with axum's `serve()`.
    pub struct NamedPipeListener {
        pipe_name: String,
        current: NamedPipeServer,
    }

    impl NamedPipeListener {
        pub fn bind(pipe_name: &str) -> io::Result<Self> {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)?;
            Ok(Self {
                pipe_name: pipe_name.to_string(),
                current: server,
            })
        }

        /// Accept a new connection. Returns the connected pipe server stream.
        pub async fn accept(&mut self) -> io::Result<NamedPipeServer> {
            // Wait for a client to connect to the current pipe instance
            self.current.connect().await?;

            // Create a new pipe instance for the next client
            let new_server = ServerOptions::new().create(&self.pipe_name)?;

            // Swap: return the connected instance, store the new one
            let connected = std::mem::replace(&mut self.current, new_server);
            Ok(connected)
        }
    }

    // Implement the tokio::net::tcp::incoming pattern for axum compatibility.
    // axum 0.8 uses the `Listener` trait from `axum::serve`.
    impl axum::serve::Listener for NamedPipeListener {
        type Io = NamedPipeServer;
        type Addr = String;

        fn accept(&mut self) -> impl std::future::Future<Output = Result<(Self::Io, Self::Addr), io::Error>> + Send {
            async move {
                let stream = NamedPipeListener::accept(self).await?;
                Ok((stream, self.pipe_name.clone()))
            }
        }

        fn local_addr(&self) -> io::Result<Self::Addr> {
            Ok(self.pipe_name.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn test_pipe_name_format() {
        assert!(super::PIPE_NAME.starts_with(r"\\.\pipe\"));
    }
}
```

- [ ] **Step 3: Add `pipe_name()` to Config**

In `crates/daemon/src/config.rs`, add after `socket_path()` (line 66):

```rust
    /// Named pipe endpoint for Windows.
    #[cfg(windows)]
    pub fn pipe_name(&self) -> String {
        r"\\.\pipe\homerun-daemon".to_string()
    }
```

- [ ] **Step 4: Update `server.rs` for cross-platform IPC and shutdown**

Rewrite the `serve()` function in `crates/daemon/src/server.rs`. The router creation and state setup remain identical — only the listener setup, stale-check, and shutdown signal change.

Replace the imports (lines 1-10):
```rust
use std::sync::Arc;

use crate::api::{service as api_service, updates as api_updates};
use anyhow::Result;
use axum::{
    routing::{delete, get, patch, post},
    Json, Router,
};
use tokio::sync::RwLock;

use crate::api;
use crate::auth::AuthManager;
use crate::config::Config;
use crate::logging::DaemonLogState;
use crate::metrics::MetricsCollector;
use crate::notifications::NotificationManager;
use crate::runner::RunnerManager;
```

(Remove `use tokio::net::UnixListener;` — it's now `#[cfg]`-gated inside `serve()`.)

Replace the `serve()` function (lines 165-304) with:

```rust
pub async fn serve(config: Config, daemon_logs: DaemonLogState) -> Result<()> {
    let state = AppState::new(config, daemon_logs);

    // Restore auth token
    if let Err(e) = state.auth.try_restore().await {
        tracing::warn!("Failed to restore auth from keychain: {}", e);
    }
    if let Some(token) = state.auth.token().await {
        state.runner_manager.set_auth_token(Some(token)).await;
    }

    // Load persisted runner configs
    let need_restart = match state.runner_manager.load_from_disk().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("Failed to load runners from disk: {}", e);
            Vec::new()
        }
    };

    // Monitor reattached runners
    {
        let runners = state.runner_manager.list().await;
        for runner in &runners {
            if runner.state == crate::runner::state::RunnerState::Online {
                if let Some(pid) = runner.pid {
                    tracing::info!(
                        "Reattached to running process for {} (PID {})",
                        runner.config.name, pid
                    );
                    state.runner_manager.monitor_orphaned_process(&runner.config.id, pid);
                }
            }
        }
    }

    // Restore previously-running runners
    let restore = state.config.read().await.preferences.start_runners_on_launch;
    if restore && !need_restart.is_empty() {
        if let Some(token) = state.auth.token().await {
            tracing::info!("Restoring {} previously-running runner(s)", need_restart.len());
            for runner_id in need_restart {
                let manager = state.runner_manager.clone();
                let tok = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager
                        .update_state(&runner_id, crate::runner::state::RunnerState::Registering)
                        .await
                    {
                        tracing::error!("Failed to transition runner {}: {}", runner_id, e);
                        return;
                    }
                    if let Err(e) = manager
                        .register_and_start_from_registering(&runner_id, &tok)
                        .await
                    {
                        tracing::error!("Failed to restore runner {}: {}", runner_id, e);
                        let _ = manager
                            .update_state_with_error(
                                &runner_id,
                                crate::runner::state::RunnerState::Error,
                                Some(format!("{e:#}")),
                            )
                            .await;
                    }
                });
            }
        } else {
            tracing::warn!("Cannot restore runners: no auth token available. Sign in and restart.");
        }
    }

    state.runner_manager.start_job_context_poller();

    let app = create_router(state);

    // ── Platform-specific listener & serve ──────────────────────────

```rust
pub async fn serve(config: Config, daemon_logs: DaemonLogState) -> Result<()> {
    // Extract IPC config before moving config into state
    #[cfg(unix)]
    let socket_path = config.socket_path();
    #[cfg(windows)]
    let pipe_name = config.pipe_name();

    // ── Stale connection check ──────────────────────────────────────
    #[cfg(unix)]
    {
        if socket_path.exists() {
            match tokio::net::UnixStream::connect(&socket_path).await {
                Ok(_) => {
                    anyhow::bail!(
                        "Daemon already running (socket {} is active). Stop the existing daemon first.",
                        socket_path.display()
                    );
                }
                Err(_) => {
                    tracing::info!("Removing stale socket file: {}", socket_path.display());
                    std::fs::remove_file(&socket_path)?;
                }
            }
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    #[cfg(windows)]
    {
        if crate::platform::ipc::is_daemon_reachable(std::path::Path::new("")).await {
            anyhow::bail!(
                "Daemon already running (pipe {} is active). Stop the existing daemon first.",
                pipe_name
            );
        }
    }

    let state = AppState::new(config, daemon_logs);

    // Restore auth token
    if let Err(e) = state.auth.try_restore().await {
        tracing::warn!("Failed to restore auth from keychain: {}", e);
    }
    if let Some(token) = state.auth.token().await {
        state.runner_manager.set_auth_token(Some(token)).await;
    }

    let need_restart = match state.runner_manager.load_from_disk().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("Failed to load runners from disk: {}", e);
            Vec::new()
        }
    };

    {
        let runners = state.runner_manager.list().await;
        for runner in &runners {
            if runner.state == crate::runner::state::RunnerState::Online {
                if let Some(pid) = runner.pid {
                    tracing::info!(
                        "Reattached to running process for {} (PID {})",
                        runner.config.name, pid
                    );
                    state.runner_manager.monitor_orphaned_process(&runner.config.id, pid);
                }
            }
        }
    }

    let restore = state.config.read().await.preferences.start_runners_on_launch;
    if restore && !need_restart.is_empty() {
        if let Some(token) = state.auth.token().await {
            tracing::info!("Restoring {} previously-running runner(s)", need_restart.len());
            for runner_id in need_restart {
                let manager = state.runner_manager.clone();
                let tok = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager
                        .update_state(&runner_id, crate::runner::state::RunnerState::Registering)
                        .await
                    {
                        tracing::error!("Failed to transition runner {}: {}", runner_id, e);
                        return;
                    }
                    if let Err(e) = manager
                        .register_and_start_from_registering(&runner_id, &tok)
                        .await
                    {
                        tracing::error!("Failed to restore runner {}: {}", runner_id, e);
                        let _ = manager
                            .update_state_with_error(
                                &runner_id,
                                crate::runner::state::RunnerState::Error,
                                Some(format!("{e:#}")),
                            )
                            .await;
                    }
                });
            }
        } else {
            tracing::warn!("Cannot restore runners: no auth token available. Sign in and restart.");
        }
    }

    state.runner_manager.start_job_context_poller();
    let app = create_router(state);

    // ── Platform-specific listener ──────────────────────────────────

    #[cfg(unix)]
    {
        let listener = tokio::net::UnixListener::bind(&socket_path)?;
        tracing::info!("Listening on Unix socket: {}", socket_path.display());

        let server = axum::serve(listener, app);

        let shutdown_signal = async {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            let sigint = tokio::signal::ctrl_c();
            tokio::select! {
                _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
                _ = sigint => tracing::info!("Received SIGINT"),
            }
        };

        server.with_graceful_shutdown(shutdown_signal).await?;

        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    #[cfg(windows)]
    {
        let listener = crate::platform::ipc::named_pipe::NamedPipeListener::bind(&pipe_name)?;
        tracing::info!("Listening on named pipe: {}", pipe_name);

        let server = axum::serve(listener, app);

        let shutdown_signal = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to register Ctrl-C handler");
            tracing::info!("Received Ctrl-C");
        };

        server.with_graceful_shutdown(shutdown_signal).await?;
    }

    tracing::info!("Daemon shut down gracefully");
    Ok(())
}
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All existing server tests pass (they use `create_router` directly, not `serve()`).

- [ ] **Step 6: Verify compilation on Windows**

Run: `cargo check -p homerund`
Expected: No compilation errors. The `#[cfg(windows)]` paths should compile since we're on Windows.

Note: The `axum::serve::Listener` trait implementation for `NamedPipeListener` may need adjustment based on the exact axum 0.8 API. If `Listener` trait is not public or has different requirements, we'll need to adapt — check `axum::serve` docs. If axum doesn't expose a `Listener` trait directly, we may need to use `axum::serve::IntoMakeService` with a custom `hyper` server setup instead.

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/platform/ipc.rs crates/daemon/src/platform/mod.rs crates/daemon/src/config.rs crates/daemon/src/server.rs
git commit -m "feat: named pipe IPC for Windows daemon server (#112)"
```

---

## Task 6: Platform-aware TUI client

**Files:**
- Modify: `crates/tui/src/client.rs`
- Modify: `crates/tui/src/daemon_lifecycle.rs`
- Modify: `crates/tui/Cargo.toml`

Update the TUI client to connect via named pipes on Windows.

- [ ] **Step 1: Add tokio named pipe feature to TUI Cargo.toml (if needed)**

The TUI crate already depends on `tokio` with `full` features (workspace), which includes named pipe support on Windows. No Cargo.toml change needed.

However, verify that `hyper-util` and `tower` are available — they are (lines 23-25 of `crates/tui/Cargo.toml`).

- [ ] **Step 2: Add Windows `NamedPipeConnector` to `tui/src/client.rs`**

After the existing `UnixConnector` implementation (lines 253-281), add:

```rust
// --- Windows named pipe HTTP connector ---

#[cfg(windows)]
#[derive(Clone)]
struct NamedPipeConnector {
    pipe_name: String,
}

#[cfg(windows)]
impl tower::Service<hyper::Uri> for NamedPipeConnector {
    type Response = hyper_util::rt::TokioIo<tokio::net::windows::named_pipe::NamedPipeClient>;
    type Error = std::io::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: hyper::Uri) -> Self::Future {
        let pipe_name = self.pipe_name.clone();
        Box::pin(async move {
            let client = tokio::net::windows::named_pipe::ClientOptions::new()
                .open(&pipe_name)?;
            Ok(hyper_util::rt::TokioIo::new(client))
        })
    }
}
```

- [ ] **Step 3: Update `DaemonClient` for cross-platform support**

Gate the `UnixConnector` usage and add Windows counterparts. Update the `DaemonClient` struct and methods:

Gate the `UnixConnector` struct with `#[cfg(unix)]`:
```rust
#[cfg(unix)]
#[derive(Clone)]
struct UnixConnector {
    socket_path: PathBuf,
}
```

And its `impl tower::Service` with `#[cfg(unix)]`.

Update `DaemonClient`:

```rust
pub struct DaemonClient {
    #[cfg(unix)]
    socket_path: PathBuf,
    #[cfg(windows)]
    pipe_name: String,
}

impl DaemonClient {
    #[cfg(unix)]
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    #[cfg(windows)]
    pub fn new_pipe(pipe_name: String) -> Self {
        Self { pipe_name }
    }

    pub fn default_socket() -> Self {
        #[cfg(unix)]
        {
            let home = dirs::home_dir().expect("no home directory");
            Self::new(home.join(".homerun/daemon.sock"))
        }
        #[cfg(windows)]
        {
            Self::new_pipe(r"\\.\pipe\homerun-daemon".to_string())
        }
    }

    pub fn socket_exists(&self) -> bool {
        #[cfg(unix)]
        { self.socket_path.exists() }
        #[cfg(windows)]
        {
            // Try to open the pipe — if it succeeds, daemon is listening
            tokio::net::windows::named_pipe::ClientOptions::new()
                .open(&self.pipe_name)
                .is_ok()
        }
    }

    #[cfg(unix)]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
```

Update `request()` and `raw_request()` methods to use the right connector:

```rust
    async fn request(&self, method: &str, path: &str, body: Option<String>) -> Result<String> {
        let uri = format!("http://localhost{path}");
        let mut builder = Request::builder().method(method).uri(&uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder.body(body.unwrap_or_default())?;

        #[cfg(unix)]
        let response = {
            let connector = UnixConnector { socket_path: self.socket_path.clone() };
            let client: Client<UnixConnector, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client.request(req).await
                .context("Failed to connect to daemon — is homerund running?")?
        };

        #[cfg(windows)]
        let response = {
            let connector = NamedPipeConnector { pipe_name: self.pipe_name.clone() };
            let client: Client<NamedPipeConnector, String> =
                Client::builder(TokioExecutor::new()).build(connector);
            client.request(req).await
                .context("Failed to connect to daemon — is homerund running?")?
        };

        let status = response.status();
        let collected = http_body_util::BodyExt::collect(response.into_body())
            .await
            .context("Failed to read response body")?;
        let bytes = collected.to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();

        if !status.is_success() && status.as_u16() != 204 {
            bail!("Daemon returned {status}: {text}");
        }
        Ok(text)
    }
```

Apply the same pattern to `raw_request()`.

Update `connect_events()` for Windows:

```rust
    #[cfg(unix)]
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::UnixStream>>> {
        let stream = tokio::net::UnixStream::connect(&self.socket_path).await?;
        let uri = "ws://localhost/events";
        let (ws_stream, _response) = tokio_tungstenite::client_async(uri, stream).await?;
        let (_write, read) = ws_stream.split();
        Ok(read)
    }

    #[cfg(windows)]
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::windows::named_pipe::NamedPipeClient>>> {
        let client = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(&self.pipe_name)?;
        let uri = "ws://localhost/events";
        let (ws_stream, _response) = tokio_tungstenite::client_async(uri, client).await?;
        let (_write, read) = ws_stream.split();
        Ok(read)
    }
```

- [ ] **Step 4: Update `daemon_lifecycle.rs` for cross-platform**

In `crates/tui/src/daemon_lifecycle.rs`:

Replace `default_socket_path()` (lines 9-13):
```rust
#[cfg(unix)]
fn default_socket_path() -> std::path::PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join(".homerun/daemon.sock")
}

#[cfg(windows)]
fn default_pipe_name() -> String {
    r"\\.\pipe\homerun-daemon".to_string()
}
```

Update `is_daemon_running()` (lines 15-21):
```rust
#[cfg(unix)]
async fn is_daemon_running(socket: &std::path::Path) -> bool {
    if !socket.exists() {
        return false;
    }
    let client = DaemonClient::new(socket.to_path_buf());
    client.health().await.is_ok()
}

#[cfg(windows)]
async fn is_daemon_running() -> bool {
    let client = DaemonClient::default_socket();
    client.health().await.is_ok()
}
```

Update `start_daemon()` (lines 23-50):
```rust
pub async fn start_daemon() -> Result<()> {
    #[cfg(unix)]
    {
        let socket = default_socket_path();
        if is_daemon_running(&socket).await {
            bail!("Daemon is already running");
        }
        if socket.exists() {
            std::fs::remove_file(&socket)?;
        }
    }
    #[cfg(windows)]
    {
        if is_daemon_running().await {
            bail!("Daemon is already running");
        }
    }

    let binary = which::which("homerund")
        .context("homerund not found in PATH. Install it or add it to your PATH.")?;
    std::process::Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn homerund")?;

    let client = DaemonClient::default_socket();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if client.health().await.is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("Daemon failed to start within 5 seconds — check logs at ~/.homerun/logs/");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
```

Update `stop_daemon()` — replace launchd-specific error message (lines 62-66):
```rust
        Err(e) => {
            let msg = format!("{e}");
            #[cfg(target_os = "macos")]
            if msg.contains("launchd") || msg.contains("Uninstall the service") {
                bail!(
                    "Daemon is managed by launchd. Uninstall the service first \
                     (Settings > Startup) or run: launchctl unload ~/Library/LaunchAgents/com.homerun.daemon.plist"
                );
            }
            #[cfg(windows)]
            if msg.contains("Task Scheduler") || msg.contains("Uninstall the service") {
                bail!(
                    "Daemon is managed by Task Scheduler. Uninstall the service first \
                     (Settings > Startup) or run: schtasks /Delete /TN \"HomeRun Daemon\" /F"
                );
            }
```

Update socket cleanup in `stop_daemon()` to be Unix-only:
```rust
    #[cfg(unix)]
    let socket = default_socket_path();

    // ... existing error handling ...

    #[cfg(unix)]
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }
```

The socket-polling loop in `stop_daemon()` also needs platform awareness:
```rust
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        #[cfg(unix)]
        {
            if !socket.exists() {
                return Ok(());
            }
        }
        #[cfg(windows)]
        {
            if !DaemonClient::default_socket().health().await.is_ok() {
                return Ok(());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            #[cfg(unix)]
            if socket.exists() {
                let _ = std::fs::remove_file(&socket);
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p homerun --lib`
Expected: All TUI tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/client.rs crates/tui/src/daemon_lifecycle.rs
git commit -m "feat: Windows named pipe support for TUI client (#112)"
```

---

## Task 7: Platform-aware Desktop client

**Files:**
- Modify: `apps/desktop/src-tauri/src/client.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`

Apply the same named pipe connector pattern to the Tauri desktop client.

- [ ] **Step 1: Update desktop Cargo.toml for Windows**

In `apps/desktop/src-tauri/Cargo.toml`, gate `mac-notification-sys` to macOS only and ensure tokio is available:

Replace:
```toml
mac-notification-sys = "0.6"
```

With:
```toml
[target.'cfg(target_os = "macos")'.dependencies]
mac-notification-sys = "0.6"
```

(Move it out of the main `[dependencies]` section.)

- [ ] **Step 2: Add `NamedPipeConnector` to desktop client**

In `apps/desktop/src-tauri/src/client.rs`, gate `UnixConnector` with `#[cfg(unix)]` and add the Windows counterpart after it (same code as TUI client Task 6 Step 2):

```rust
#[cfg(windows)]
#[derive(Clone)]
struct NamedPipeConnector {
    pipe_name: String,
}

#[cfg(windows)]
impl tower::Service<hyper::Uri> for NamedPipeConnector {
    type Response = hyper_util::rt::TokioIo<tokio::net::windows::named_pipe::NamedPipeClient>;
    type Error = std::io::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: hyper::Uri) -> Self::Future {
        let pipe_name = self.pipe_name.clone();
        Box::pin(async move {
            let client = tokio::net::windows::named_pipe::ClientOptions::new()
                .open(&pipe_name)?;
            Ok(hyper_util::rt::TokioIo::new(client))
        })
    }
}
```

- [ ] **Step 3: Update `DaemonClient` in desktop client**

Apply the same cross-platform `DaemonClient` changes as in Task 6 Step 3. The desktop client's `DaemonClient` follows the same pattern. Update:

- `DaemonClient` struct to hold `#[cfg(unix)] socket_path` / `#[cfg(windows)] pipe_name`
- `default_socket()` to return platform-appropriate client
- `socket_exists()` to check pipe on Windows
- `request()` method to use the right connector
- Remove `socket_path()` accessor or gate it with `#[cfg(unix)]`

- [ ] **Step 4: Gate macOS notification imports**

In `apps/desktop/src-tauri/src/lib.rs` (or wherever `mac-notification-sys` is imported), gate it:

```rust
#[cfg(target_os = "macos")]
use mac_notification_sys::*;
```

Search for all `mac-notification-sys` / `mac_notification_sys` usages and gate them appropriately.

- [ ] **Step 5: Verify desktop compilation**

Run from `apps/desktop/src-tauri`: `cargo check`
Expected: Compiles without errors on Windows.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/client.rs apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: Windows named pipe support for desktop client (#112)"
```

---

## Task 8: Update runner/mod.rs references to run.sh/config.sh

**Files:**
- Modify: `crates/daemon/src/runner/mod.rs`

There are scattered references to `run.sh` in comments and in the copy logic that need updating.

- [ ] **Step 1: Search and update `run.sh` references in `runner/mod.rs`**

In `crates/daemon/src/runner/mod.rs`, add import:
```rust
use crate::platform::process::run_script;
```

Update line 3272 (test that creates a fake `run.sh`):
```rust
let script_path = src.path().join(crate::platform::process::run_script());
```

Update line 3283 (test that checks `run.sh` in dst):
```rust
let dst_script = dst.path().join(crate::platform::process::run_script());
```

Update comments referencing `run.sh` (lines 1325, 1350, 1409, 1797) to say `run.sh/run.cmd` or just "the runner script".

- [ ] **Step 2: Verify tests pass**

Run: `cargo test -p homerund --lib`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "refactor: use platform::process::run_script() in runner module (#112)"
```

---

## Task 9: End-to-end verification

**Files:** None (testing only)

- [ ] **Step 1: Run full daemon test suite**

Run: `cargo test -p homerund`
Expected: All tests pass.

- [ ] **Step 2: Run full TUI test suite**

Run: `cargo test -p homerun`
Expected: All tests pass.

- [ ] **Step 3: Compile-check the entire workspace**

Run: `cargo check --workspace`
Expected: Clean compilation. (Desktop app is a separate workspace — check separately if possible.)

- [ ] **Step 4: Manual smoke test — start daemon on Windows**

Run: `cargo run -p homerund`
Expected: Daemon starts and logs "Listening on named pipe: \\.\pipe\homerun-daemon"

- [ ] **Step 5: Manual smoke test — TUI connects to daemon**

In another terminal, run: `cargo run -p homerun`
Expected: TUI connects to daemon via named pipe, shows health status.

- [ ] **Step 6: Commit any test fixes**

If any tests needed adjustment:
```bash
git add -A
git commit -m "fix: test adjustments for Windows platform support (#112)"
```
