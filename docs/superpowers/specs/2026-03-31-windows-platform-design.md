# Windows Platform Support — Design Spec

**Issue:** #112 (partial — Windows only)
**Date:** 2026-03-31

## Overview

Extend HomeRun's daemon, TUI client, and desktop client to run on Windows. The current codebase is macOS-only with platform-specific code scattered across several modules. This design introduces a `platform/` module that isolates all OS-specific logic behind `#[cfg]`-gated submodules with consistent public APIs.

## Approach

**Approach B: Platform module with `#[cfg]`-selected implementations.**

Create `crates/daemon/src/platform/` with submodules per concern (`ipc`, `process`, `service`, `shell`). Each submodule contains both Unix and Windows implementations using `#[cfg(unix)]` / `#[cfg(windows)]` blocks. Existing code calls into `platform::*` instead of directly using Unix APIs. No traits — just free functions with the same signatures on both platforms.

## Platform-Specific Touchpoints

| #   | Component        | Current File(s)                                        | macOS-Specific Code                                                         |
| --- | ---------------- | ------------------------------------------------------ | --------------------------------------------------------------------------- |
| 1   | IPC (server)     | `daemon/src/server.rs`                                 | `UnixListener`, `UnixStream`, stale socket cleanup, SIGTERM handler         |
| 2   | IPC (clients)    | `tui/src/client.rs`, `desktop/src-tauri/src/client.rs` | `UnixConnector`, `UnixStream`, WebSocket over Unix socket                   |
| 3   | Process mgmt     | `daemon/src/runner/process.rs`                         | `pgrep`, `libc::kill`/`SIGTERM`/`SIGKILL`, `setsid()`, `run.sh`/`config.sh` |
| 4   | Binary download  | `daemon/src/runner/binary.rs`                          | Hardcoded `"osx"`, `tar xzf` extraction, `run.sh` existence check           |
| 5   | Auto-start       | `daemon/src/launchd.rs`                                | launchd plist, `launchctl` commands                                         |
| 6   | Service API      | `daemon/src/api/service.rs`                            | Direct `crate::launchd::*` calls                                            |
| 7   | Daemon lifecycle | `tui/src/daemon_lifecycle.rs`                          | `daemon.sock` path, launchd error messages                                  |
| 8   | Config           | `daemon/src/config.rs`                                 | `socket_path()` returns `.sock`                                             |
| 9   | Shell PATH       | `daemon/src/runner/process.rs`                         | `$SHELL -l -c "echo $PATH"`                                                 |

## Design Sections

### 1. Platform Module Structure

```
crates/daemon/src/platform/
├── mod.rs          # Re-exports, cfg gates
├── ipc.rs          # Unix socket (unix) / Named pipe (windows) listener + client connector
├── process.rs      # Process discovery, signaling, spawning
├── service.rs      # launchd (macOS) / schtasks (Windows)
└── shell.rs        # Shell PATH resolution
```

`mod.rs` re-exports all public items from submodules. Each submodule contains platform-specific code with `#[cfg(unix)]` / `#[cfg(windows)]` blocks. No traits — just free functions with the same signatures on both platforms.

### 2. IPC Transport

**Daemon side (`platform::ipc`):**

- Unix: `tokio::net::UnixListener` at `~/.homerun/daemon.sock` (existing behavior)
- Windows: `tokio::net::windows::named_pipe::ServerOptions` at `\\.\pipe\homerun-daemon`

A custom `NamedPipeListener` struct that implements an accept loop, yielding connected `NamedPipeServer` instances wrapped as `tokio::io::AsyncRead + AsyncWrite`. This integrates with axum's `serve()` via the same pattern used for `UnixListener`.

**Client side (TUI + Desktop):**

The `UnixConnector` tower service gets a `#[cfg(windows)]` counterpart `NamedPipeConnector` that opens `\\.\pipe\homerun-daemon` via `ClientOptions::new().open()`. hyper just needs an `AsyncRead + AsyncWrite` stream — it doesn't care about the transport.

**Config change:**

`Config::socket_path()` renamed/extended to `Config::ipc_endpoint()`:

- Unix: returns `PathBuf` for `~/.homerun/daemon.sock`
- Windows: returns `String` for `\\.\pipe\homerun-daemon`

On Unix, `socket_path()` continues to return `PathBuf` for the socket file. On Windows, `pipe_name()` returns the pipe name string `\\.\pipe\homerun-daemon`. Both are `#[cfg]`-gated methods on `Config`. The `serve()` function and client code use `#[cfg]` to call the appropriate method.

**Stale connection check:**

- Unix: try `UnixStream::connect()`, remove stale socket file if unreachable
- Windows: not needed — named pipes are kernel objects that vanish when the server process dies

**Shutdown signal:**

- Unix: SIGTERM + Ctrl-C (current)
- Windows: `tokio::signal::ctrl_c()` only (no SIGTERM on Windows)

### 3. Process Management

**`platform::process` public API:**

```rust
pub async fn find_runner_pids(runner_dir: &Path) -> Vec<u32>
pub async fn find_runner_pid(runner_dir: &Path) -> Option<u32>
pub async fn kill_orphaned_processes(runner_dir: &Path)
pub fn configure_process_group(cmd: &mut tokio::process::Command)
pub fn runner_script(name: &str) -> String  // "run" -> "run.sh" or "run.cmd"
```

**Finding runner processes:**

- Unix: `pgrep -f {dir_str}` (existing)
- Windows: `sysinfo` crate (already a dependency) to enumerate processes, filter by command line containing the runner directory path

**Killing processes:**

- Unix: `libc::kill(-pid, SIGTERM)` then `SIGKILL` after timeout (existing)
- Windows: `taskkill /T /F /PID {pid}` — `/T` kills the process tree (equivalent to Unix process group kill)

**Process groups on spawn:**

- Unix: `pre_exec(|| { setsid(); })` (existing)
- Windows: `cmd.creation_flags(CREATE_NEW_PROCESS_GROUP)` via `std::os::windows::process::CommandExt`

**Runner scripts:**

- Unix: `config.sh`, `run.sh`
- Windows: `config.cmd`, `run.cmd` (GitHub Actions runner ships both)

### 4. Shell PATH Resolution

**`platform::shell` public API:**

```rust
pub fn resolve_shell_path() -> Option<String>
```

- Unix: `$SHELL -l -c "echo $PATH"` (existing logic, moved here)
- Windows: returns `None` — Windows resolves PATH from the system environment automatically. Callers already handle `None` (they skip the PATH override).

### 5. Auto-Start Service

**`platform::service` public API:**

```rust
pub fn install_daemon_service(daemon_path: &Path) -> Result<()>
pub fn uninstall_daemon_service() -> Result<()>
pub fn is_daemon_installed() -> bool
```

**macOS:** launchd plist at `~/Library/LaunchAgents/com.homerun.daemon.plist` + `launchctl load/unload` (existing logic, moved here)

**Windows:** Task Scheduler via `schtasks.exe`:

- **Install:** `schtasks /Create /SC ONLOGON /TN "HomeRun Daemon" /TR "\"{daemon_path}\"" /RL HIGHEST /F`
- **Uninstall:** `schtasks /Delete /TN "HomeRun Daemon" /F`
- **Status:** `schtasks /Query /TN "HomeRun Daemon"` — exit code 0 means installed

Task name constant: `"HomeRun Daemon"`

### 6. Binary Download & Extraction

**`detect_platform()` update:**

```rust
pub fn detect_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") { "osx" }
        else if cfg!(target_os = "windows") { "win" }
        else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    (os, arch)
}
```

**Archive format:**

- Unix: `.tar.gz` — extracted with `tar xzf` (existing)
- Windows: `.zip` — extracted with the `zip` crate (pure Rust, no shell dependency)

**Download URL:** `runner_download_url()` already takes `os` as a parameter, so the URL format handles itself. The only difference is the file extension.

**Runner existence check:**

- Unix: check for `run.sh`
- Windows: check for `run.cmd`

Uses `platform::process::runner_script("run")` to get the correct filename.

### 7. Client Updates

Both `crates/tui/src/client.rs` and `apps/desktop/src-tauri/src/client.rs` have identical `UnixConnector` + `DaemonClient` structures. Changes:

- Add `#[cfg(windows)]` `NamedPipeConnector` alongside `UnixConnector`
- `DaemonClient::default_socket()` becomes platform-aware
- `DaemonClient::socket_exists()` on Windows checks pipe availability via a connect attempt
- TUI's `connect_events()` WebSocket: connect over named pipe instead of Unix stream on Windows

### 8. Daemon Lifecycle (TUI)

`tui/src/daemon_lifecycle.rs` changes:

- `default_socket_path()` → `default_ipc_endpoint()` using platform module
- `is_daemon_running()`: on Windows, check pipe connectivity instead of socket file existence
- `stop_daemon()`: platform-aware error messages (launchd on macOS, Task Scheduler on Windows)

## New Dependencies

| Crate | Purpose                                | Platform                      |
| ----- | -------------------------------------- | ----------------------------- |
| `zip` | Extract Windows runner `.zip` archives | Windows only (`#[cfg]` gated) |

No new dependencies for IPC (tokio has built-in named pipe support) or process management (`sysinfo` already present).

## Out of Scope

- Linux-specific implementations (Unix code path already covers Linux for IPC, process, shell)
- Linux-specific auto-start (`systemd`) — future work
- Desktop app (Tauri) platform-specific notifications — Tauri handles this natively
- `.msi` installer packaging — follow-up
- CI/CD for Windows builds — follow-up
- `keyring` crate migration — file-based token storage works cross-platform already
- Mobile platforms, BSD variants

## Testing Strategy

- All existing macOS tests must continue to pass (Unix code paths unchanged)
- New `#[cfg(windows)]` unit tests for each `platform::*` submodule
- Integration tests: daemon startup via named pipe, client connectivity
- `detect_platform()` tests updated to expect `"win"` on Windows
