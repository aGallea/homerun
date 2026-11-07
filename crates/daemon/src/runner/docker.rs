//! Docker-backed runner execution (`RunnerMode::Container`).
//!
//! Mirrors the shape of `runner::process` (configure/start, stop, remove) so
//! `runner::mod`'s lifecycle code — state machine, stdout line parsing,
//! ProcessHandle stop/monitor plumbing — only branches at the exact spawn
//! and monitor steps, not throughout.
//!
//! The runner binary is never baked into the image: `runner::binary`
//! downloads the Linux build into the shared cache exactly as it does for
//! native runners, `copy_dir_recursive` copies it into the runner's own
//! `work_dir` (also unchanged), and this module bind-mounts that per-runner
//! `work_dir` into the container. A container base image only needs an OS +
//! toolchain — this is what keeps a small set of first-party images (or any
//! user-supplied image) usable without runner-specific baking.

use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, LogsOptionsBuilder,
    RemoveContainerOptionsBuilder, StatsOptionsBuilder, StopContainerOptionsBuilder,
    WaitContainerOptionsBuilder,
};
use bollard::Docker;
use futures::StreamExt;
use tokio::io::{duplex, AsyncRead, AsyncWriteExt};

/// Container path the runner's per-runner work_dir is bind-mounted at.
const WORKSPACE: &str = "/workspace";

/// Bootstrap script run inside the container. Values are threaded in via
/// environment variables (not interpolated into this string) so untrusted
/// runner names/labels can never affect what the shell executes.
const BOOTSTRAP_SCRIPT: &str = r#"set -e
cd /workspace
rm -f .runner .runner_migrated .credentials .credentials_rsaparams
./config.sh --url "$HOMERUN_RUNNER_URL" --token "$HOMERUN_RUNNER_TOKEN" --name "$HOMERUN_RUNNER_NAME" --labels "$HOMERUN_RUNNER_LABELS" --unattended --replace
exec ./run.sh
"#;

/// A running (or exited) container-backed runner process.
pub struct ContainerProcess {
    pub container_id: String,
    pub stdout: Pin<Box<dyn AsyncRead + Send>>,
    pub stderr: Pin<Box<dyn AsyncRead + Send>>,
}

/// Connects to the local Docker engine (Unix socket on macOS/Linux, named
/// pipe on Windows — `bollard` picks the right transport for the host).
pub fn connect() -> Result<Docker> {
    Docker::connect_with_local_defaults()
        .context("Failed to connect to Docker. Is Docker Desktop (or the Docker daemon) running?")
}

/// Preflight check used to gate the "Container" runner mode in the UI.
pub async fn docker_status() -> Result<()> {
    let docker = connect()?;
    docker
        .ping()
        .await
        .context("Docker daemon did not respond to ping")?;
    Ok(())
}

/// Pulls `image` if not already present locally, then creates and starts a
/// container running the given runner's `config.sh`/`run.sh` against
/// `work_dir` (already populated with the cached runner binary by the
/// caller, exactly as the native path populates it).
#[allow(clippy::too_many_arguments)]
pub async fn configure_and_start_container(
    docker: &Docker,
    work_dir: &Path,
    image: &str,
    url: &str,
    token: &str,
    name: &str,
    labels: &[String],
    extra_env: &[(String, String)],
) -> Result<ContainerProcess> {
    let labels_str = labels.join(",");
    let mut env = vec![
        // The GitHub runner's config.sh/run.sh refuse to run as root unless
        // this is set. Container images very commonly run as root (the
        // first-party base image does), and root inside a container is the
        // norm — the runner's guard is meant for host installs. Set it first
        // so a user can still override via `extra_env` if their image runs as
        // a non-root user and they'd rather keep the guard active.
        "RUNNER_ALLOW_RUNASROOT=1".to_string(),
        format!("HOMERUN_RUNNER_URL={url}"),
        format!("HOMERUN_RUNNER_TOKEN={token}"),
        format!("HOMERUN_RUNNER_NAME={name}"),
        format!("HOMERUN_RUNNER_LABELS={labels_str}"),
    ];
    env.extend(extra_env.iter().map(|(k, v)| format!("{k}={v}")));

    let container_id = create_and_start(
        docker,
        work_dir,
        image,
        &format!("homerun-runner-{name}"),
        env,
        vec![BOOTSTRAP_SCRIPT.to_string()],
    )
    .await
    .with_context(|| format!("Failed to create/start container for runner '{name}'"))?;

    let (stdout, stderr) = spawn_log_demux(docker.clone(), container_id.clone());

    Ok(ContainerProcess {
        container_id,
        stdout: Box::pin(stdout),
        stderr: Box::pin(stderr),
    })
}

/// Stops the container (Docker sends SIGTERM, waits up to `timeout`, then
/// SIGKILLs — the same graceful-then-force semantics the native path
/// implements manually) and removes it.
pub async fn stop_container(docker: &Docker, container_id: &str, timeout: Duration) -> Result<()> {
    let options = StopContainerOptionsBuilder::new()
        .t(timeout.as_secs() as i32)
        .build();
    // Ignore "already stopped"/"not found" — stopping is best-effort during shutdown.
    let _ = docker.stop_container(container_id, Some(options)).await;
    remove_container(docker, container_id).await
}

/// Force-removes a container by id or name. Best-effort: ignores
/// "not found"/"already removed". Used both when stopping a runner and when a
/// container exits on its own, so exited containers don't accumulate.
pub async fn remove_container(docker: &Docker, container_id: &str) -> Result<()> {
    let options = RemoveContainerOptionsBuilder::new().force(true).build();
    let _ = docker.remove_container(container_id, Some(options)).await;
    Ok(())
}

/// Runs `config.sh remove --token <token>` in a short-lived helper
/// container against the same bind-mounted `work_dir`, then removes the
/// helper container. A non-zero config-script exit is returned to the caller:
/// local deletion must not silently leave a stale runner registered on GitHub.
pub async fn deregister(docker: &Docker, work_dir: &Path, image: &str, token: &str) -> Result<()> {
    // config.sh remove hits the same root guard as config.sh/run.sh.
    let env = vec![
        "RUNNER_ALLOW_RUNASROOT=1".to_string(),
        format!("HOMERUN_RUNNER_TOKEN={token}"),
    ];
    let cmd = vec![
        "cd /workspace && exec ./config.sh remove --token \"$HOMERUN_RUNNER_TOKEN\"".to_string(),
    ];
    let name = format!("homerun-deregister-{}", uuid::Uuid::new_v4());

    let container_id = create_and_start(docker, work_dir, image, &name, env, cmd)
        .await
        .context("Failed to start runner deregistration container")?;
    let wait_result = wait_container(docker, &container_id).await;
    let cleanup_result = remove_container(docker, &container_id).await;
    let exit_code = wait_result.context("Failed while waiting for runner deregistration")?;
    cleanup_result?;
    if exit_code != 0 {
        anyhow::bail!("Runner deregistration container exited with code {exit_code}");
    }
    Ok(())
}

/// Pulls `image` if needed, creates a container bind-mounting `work_dir` at
/// `/workspace` with a `/bin/bash -c <cmd>` entrypoint, and starts it.
/// Shared by the long-lived runner container and the one-shot deregister
/// helper.
async fn create_and_start(
    docker: &Docker,
    work_dir: &Path,
    image: &str,
    container_name: &str,
    env: Vec<String>,
    cmd: Vec<String>,
) -> Result<String> {
    pull_image(docker, image).await?;

    // Best-effort cleanup of a leftover container from a previous daemon
    // session with the same deterministic name (mirrors what
    // `kill_orphaned_processes` does for the native process backend).
    remove_container(docker, container_name).await?;

    let bind = format!("{}:{}", work_dir.to_string_lossy(), WORKSPACE);
    let body = ContainerCreateBody {
        image: Some(image.to_string()),
        working_dir: Some(WORKSPACE.to_string()),
        entrypoint: Some(vec!["/bin/bash".to_string(), "-c".to_string()]),
        cmd: Some(cmd),
        env: Some(env),
        host_config: Some(HostConfig {
            binds: Some(vec![bind]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = CreateContainerOptionsBuilder::new()
        .name(container_name)
        .build();

    let created = docker
        .create_container(Some(options), body)
        .await
        .with_context(|| format!("Failed to create container '{container_name}'"))?;

    docker
        .start_container(&created.id, None)
        .await
        .with_context(|| format!("Failed to start container '{container_name}'"))?;

    Ok(created.id)
}

/// CPU/memory usage for a single container, shaped to match
/// `metrics::RunnerMetrics` so the API layer can merge native and
/// container-backed runners into one list.
pub struct ContainerStats {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

/// Docker's CPU-percent formula: the fraction of total system CPU time the
/// container consumed over the sample interval, scaled to the number of cores.
/// `(cpu_delta / system_delta) * online_cpus * 100` — one fully-busy core = 100%.
/// Uses saturating subtraction so a counter reset can't produce a negative
/// delta, and returns 0 when no system time elapsed (avoids NaN/inf).
fn compute_cpu_percent(
    cpu_total: u64,
    precpu_total: u64,
    system_total: u64,
    presystem_total: u64,
    online_cpus: u32,
) -> f64 {
    let cpu_delta = cpu_total.saturating_sub(precpu_total) as f64;
    let system_delta = system_total.saturating_sub(presystem_total) as f64;
    let online = online_cpus.max(1) as f64;
    if system_delta > 0.0 {
        (cpu_delta / system_delta) * online * 100.0
    } else {
        0.0
    }
}

/// Current CPU/memory usage for a container, using the same CPU-percent formula
/// as `docker stats`: `(cpu_delta / system_delta) * online_cpus * 100`.
///
/// Streams two stat frames instead of a single one-shot read. A one-shot read
/// leaves `precpu_stats` zeroed, so its delta spans the container's whole
/// lifetime — a lifetime average, not current usage. The second streamed
/// frame's `precpu` is the first frame, giving a real (~1s) interval. Callers
/// fetch containers' stats concurrently, so the ~1s wait is paid once overall.
pub async fn container_stats(docker: &Docker, container_id: &str) -> Result<ContainerStats> {
    let options = StatsOptionsBuilder::new().stream(true).build();
    let mut stream = docker.stats(container_id, Some(options));

    // Discard the first frame (zeroed precpu); keep the second (valid precpu).
    let _first = stream
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("no stats returned for container {container_id}"))?
        .with_context(|| format!("Failed to fetch stats for container {container_id}"))?;
    let response = stream
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("stats stream ended early for container {container_id}"))?
        .with_context(|| format!("Failed to fetch stats for container {container_id}"))?;

    let memory_bytes = response
        .memory_stats
        .as_ref()
        .and_then(|m| m.usage)
        .unwrap_or(0);

    let cpu_percent = response
        .cpu_stats
        .as_ref()
        .zip(response.precpu_stats.as_ref())
        .map(|(cpu, precpu)| {
            let total = |s: &bollard::models::ContainerCpuStats| {
                s.cpu_usage
                    .as_ref()
                    .and_then(|u| u.total_usage)
                    .unwrap_or(0)
            };
            compute_cpu_percent(
                total(cpu),
                total(precpu),
                cpu.system_cpu_usage.unwrap_or(0),
                precpu.system_cpu_usage.unwrap_or(0),
                cpu.online_cpus.unwrap_or(1),
            )
        })
        .unwrap_or(0.0);

    Ok(ContainerStats {
        cpu_percent,
        memory_bytes,
    })
}

/// Waits for the container to exit and returns its exit code.
pub async fn wait_container(docker: &Docker, container_id: &str) -> Result<i64> {
    let options = WaitContainerOptionsBuilder::new()
        .condition("not-running")
        .build();
    let mut stream = docker.wait_container(container_id, Some(options));
    match stream.next().await {
        Some(Ok(response)) => Ok(response.status_code),
        Some(Err(e)) => {
            // DockerContainerWaitError still carries the real exit code.
            if let bollard::errors::Error::DockerContainerWaitError { code, .. } = e {
                Ok(code)
            } else {
                Err(e).context("Failed waiting for container to exit")
            }
        }
        None => Ok(0),
    }
}

async fn pull_image(docker: &Docker, image: &str) -> Result<()> {
    // Skip the pull if the image already exists locally (covers locally
    // built/tagged custom images that don't exist in any registry).
    if docker.inspect_image(image).await.is_ok() {
        return Ok(());
    }

    let options = CreateImageOptionsBuilder::new().from_image(image).build();
    let mut stream = docker.create_image(Some(options), None, None);
    while let Some(next) = stream.next().await {
        next.with_context(|| format!("Failed to pull image '{image}'"))?;
    }
    Ok(())
}

/// Demultiplexes Docker's combined stdout/stderr log stream into two
/// independent `AsyncRead` handles, so the caller can treat them exactly
/// like `ChildStdout`/`ChildStderr` (`BufReader::new(..).lines()`).
fn spawn_log_demux(
    docker: Docker,
    container_id: String,
) -> (impl AsyncRead + Send, impl AsyncRead + Send) {
    let (stdout_reader, mut stdout_writer) = duplex(64 * 1024);
    let (stderr_reader, mut stderr_writer) = duplex(64 * 1024);

    tokio::spawn(async move {
        let options = LogsOptionsBuilder::new()
            .follow(true)
            .stdout(true)
            .stderr(true)
            .build();
        let mut stream = docker.logs(&container_id, Some(options));
        while let Some(next) = stream.next().await {
            match next {
                Ok(LogOutput::StdOut { message }) => {
                    if stdout_writer.write_all(&message).await.is_err() {
                        break;
                    }
                }
                Ok(LogOutput::StdErr { message }) => {
                    if stderr_writer.write_all(&message).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Container {} log stream ended: {e}", container_id);
                    break;
                }
            }
        }
    });

    (stdout_reader, stderr_reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Docker` handle that constructs successfully (points at a real
    /// file, so `connect_with_unix`'s existence check passes) but fails
    /// every actual request — a regular file isn't a listening socket.
    /// Lets these tests assert "fails gracefully" without depending on a
    /// live Docker daemon being present on the test host.
    #[cfg(unix)]
    fn broken_docker_client() -> Docker {
        let path = std::env::temp_dir().join(format!(
            "homerun-test-not-a-socket-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, b"").unwrap();
        Docker::connect_with_unix(
            &format!("unix://{}", path.display()),
            5,
            bollard::API_DEFAULT_VERSION,
        )
        .expect("constructing the client itself should succeed")
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_container_stats_fails_gracefully_without_daemon() {
        let docker = broken_docker_client();
        let result = container_stats(&docker, "nonexistent-container").await;
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_wait_container_fails_gracefully_without_daemon() {
        let docker = broken_docker_client();
        let result = wait_container(&docker, "nonexistent-container").await;
        assert!(result.is_err());
    }

    /// `remove_container` is best-effort cleanup — it must never error even
    /// when Docker is unreachable or the container is already gone, so the
    /// natural-exit cleanup path never fails the monitor task.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_remove_container_is_best_effort_without_daemon() {
        let docker = broken_docker_client();
        let result = remove_container(&docker, "nonexistent-container").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_compute_cpu_percent_one_full_core() {
        // One fully-busy core: the container used one core's worth of time
        // while the system advanced across all 4 cores (cpu_delta = interval,
        // system_delta = 4 * interval) → 100%.
        let pct = compute_cpu_percent(1_000, 0, 4_000, 0, 4);
        assert!((pct - 100.0).abs() < 1e-9, "got {pct}");
    }

    #[test]
    fn test_compute_cpu_percent_half_core_single_cpu() {
        // Half a core on a 1-CPU system: cpu_delta=50, system_delta=100 → 50%.
        let pct = compute_cpu_percent(150, 100, 200, 100, 1);
        assert!((pct - 50.0).abs() < 1e-9, "got {pct}");
    }

    #[test]
    fn test_compute_cpu_percent_zero_system_delta_is_zero() {
        // No system time elapsed (e.g. identical frames) → 0, not NaN/inf.
        let pct = compute_cpu_percent(1_000, 500, 5_000, 5_000, 4);
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn test_compute_cpu_percent_counter_reset_saturates_to_zero() {
        // If the cumulative counters appear to go backwards (reset), the
        // saturating subtraction keeps the delta at 0 rather than underflowing.
        let pct = compute_cpu_percent(100, 200, 1_000, 2_000, 2);
        assert_eq!(pct, 0.0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_configure_and_start_container_fails_without_daemon() {
        let docker = broken_docker_client();
        let dir = tempfile::tempdir().unwrap();
        let result = configure_and_start_container(
            &docker,
            dir.path(),
            "ghcr.io/example/does-not-matter:latest",
            "https://github.com/test/repo",
            "fake-token",
            "test-runner",
            &["self-hosted".to_string()],
            &[],
        )
        .await;
        assert!(result.is_err());
    }

    /// `deregister` must surface Docker failures so callers retain the local
    /// runner and can retry instead of silently leaving a stale GitHub runner.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_deregister_fails_without_daemon() {
        let docker = broken_docker_client();
        let dir = tempfile::tempdir().unwrap();
        let result = deregister(
            &docker,
            dir.path(),
            "ghcr.io/example/does-not-matter:latest",
            "fake-token",
        )
        .await;
        assert!(result.is_err());
    }
}
