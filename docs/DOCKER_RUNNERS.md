# Docker Runners

HomeRun can run a self-hosted runner inside a Docker container instead of as a native process on your machine. This gives you process isolation and an easy way to pin the exact toolchain a runner sees, without changing anything about how HomeRun talks to GitHub.

## Table of Contents

- [Requirements](#requirements)
- [Creating a container runner](#creating-a-container-runner)
- [Base images vs. custom images](#base-images-vs-custom-images)
- [Configuration reference](#configuration-reference)
- [How it works](#how-it-works)
- [Constraint: the runner needs glibc](#constraint-the-runner-needs-glibc)
- [Troubleshooting](#troubleshooting)
- [What's not supported yet](#whats-not-supported-yet)

## Requirements

- **Docker Desktop** (macOS/Windows) or the **Docker daemon** (Linux), installed and running. HomeRun pings it before offering "Container" as a mode — if it's unreachable, that option is disabled in the wizard with an explanation.
- The daemon (`homerund`) needs permission to reach the Docker socket (`/var/run/docker.sock` on macOS/Linux, the named pipe on Windows) — the same account that runs Docker Desktop / is in the `docker` group.
- Outbound internet access to pull images (GHCR for the default base image, or wherever your custom image lives) the first time a given image/tag is used. Already-pulled images are reused.
- On Apple Silicon / ARM hosts: pick an image with an `arm64` build, or expect QEMU emulation overhead for `amd64`-only images.
- No GitHub-side requirement beyond what native runners already need (repo admin access to register a runner).

## Creating a container runner

**Desktop app:** New Runner → pick a repository → on the Configure step, choose **Container** as the Mode (grayed out if Docker isn't reachable) → an **Image** field appears, pre-filled with HomeRun's base image (`ghcr.io/agallea/homerun-runner:ubuntu-24.04`) — replace it with your own image if you want → Next → Launch. From there it behaves like any other runner in the dashboard (start/stop/restart/delete, logs, job history).

**Direct API call** (useful for scripting, or before the CLI/TUI gain a creation flow — see below): the daemon exposes a Unix socket at `~/.homerun/daemon.sock` you can hit directly once you're authenticated in the app/TUI at least once:

```sh
curl --unix-socket ~/.homerun/daemon.sock \
  -X POST http://localhost/runners \
  -H "Content-Type: application/json" \
  -d '{
    "repo_full_name": "your-org/your-repo",
    "mode": "container",
    "container": { "image": "ghcr.io/agallea/homerun-runner:ubuntu-24.04" }
  }'
```

**TUI / `homerun --no-tui`:** currently read-only for container runners — you can list, monitor, and act on them (start/stop/restart/delete) the same as any runner, but the interactive/CLI _creation_ flow doesn't yet expose the Container mode or image picker. Create container runners from the desktop app or the API for now.

## Base images vs. custom images

HomeRun publishes one first-party base image — Ubuntu 24.04 with common CI tooling (git, curl, build-essential, node, python3) — to `ghcr.io/agallea/homerun-runner`. Its `Dockerfile` lives at [`docker/runner-base/Dockerfile`](../docker/runner-base/Dockerfile).

You can also point a runner at **any image you supply** — your own registry ref, a locally built/tagged image, or another CI vendor's image. There's no daemon-side allowlist; the image just needs to meet the constraint below. A minimal custom image is just:

```dockerfile
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash ca-certificates libicu74 libssl3 libkrb5-3 zlib1g \
    <+ whatever your build needs> \
 && rm -rf /var/lib/apt/lists/*
```

You don't need to install the runner itself, `git`, or anything runner-specific — see [How it works](#how-it-works).

## Configuration reference

Fields on a container runner's config (`container` object in the API; only `image` is currently exposed in the desktop wizard):

| Field       | Type                    | Required | Notes                                                                                                  |
| ----------- | ----------------------- | -------- | ------------------------------------------------------------------------------------------------------ |
| `image`     | string                  | yes      | Any pullable registry ref, or a locally built/tagged image name.                                       |
| `extra_env` | array of `[key, value]` | no       | Extra environment variables injected into the container. API-only for now — not yet in the desktop UI. |

Not yet configurable (planned, see [roadmap](../README.md#roadmap)): per-runner CPU/memory limits, ephemeral (per-job) lifecycle.

## How it works

A container runner uses the same registration/lifecycle flow as a native runner — the daemon still gets a registration token from GitHub, still runs `config.sh` then `run.sh`, and still streams logs and job events the same way. The only thing that changes is _where_ those scripts execute.

The runner binary itself is **never baked into the image**. HomeRun downloads the Linux build of the [official runner](https://github.com/actions/runner) once (cached alongside the native runner cache), copies it into the runner's own work directory, and bind-mounts that directory into the container at `/workspace`. This means:

- A base image only needs to provide an OS and a toolchain — nothing runner-specific.
- Any image you already use for CI can work as a runner image, as long as it satisfies the constraint below.

```
Host                                    Container
┌──────────────────┐                    ┌──────────────────┐
│ ~/.homerun/       │  bind mount        │                  │
│  runners/{id}/  ──┼───────────────────▶│  /workspace       │
│  (config.sh,       │  /workspace       │  (config.sh,      │
│   run.sh, _work/)  │                    │   run.sh, _work/) │
└──────────────────┘                    └──────────────────┘
```

Containers are long-lived (like native `App`/`Service` runners), not recreated per job — see [What's not supported yet](#whats-not-supported-yet).

## Constraint: the runner needs glibc

The GitHub Actions runner is a .NET application and needs a glibc-based Linux userland plus a handful of runtime libraries (`libicu`, `libssl`, `libkrb5`, `zlib`) — the same dependencies [`actions/runner`'s own `installdependencies.sh`](https://github.com/actions/runner/blob/main/src/Misc/layoutbin/installdependencies.sh) installs. **Alpine (musl) images will not work** unless you use the separate Alpine-specific runner build, which HomeRun does not currently support. Debian/Ubuntu-based images are the safe default.

If an image is missing a runtime dependency, `config.sh`/`run.sh` will fail to start inside the container — the runner surfaces this as an `Error` state with the container's stderr attached, rather than hanging silently in `Registering`.

## Troubleshooting

<details>
<summary><strong>"Container" mode is grayed out in the wizard</strong></summary>

Docker isn't reachable from the daemon. Start Docker Desktop (or the Docker daemon on Linux) and reopen the wizard — the preflight check (`GET /system/docker-status`) re-runs each time.

</details>

<details>
<summary><strong>Runner stuck in "Registering" or goes straight to "Error"</strong></summary>

Check the runner's error message / logs in the runner detail view — it includes the container's stderr. Common causes:

- The image is missing a runtime dependency (see the glibc constraint above) — `config.sh`/`run.sh` fails to exec.
- The image doesn't have `bash` (the container's entrypoint is `/bin/bash -c ...`).
- The image couldn't be pulled — check the daemon logs for the pull error (bad tag, private registry needs auth Docker isn't configured for, etc.).

</details>

<details>
<summary><strong>Image pull fails for a private registry</strong></summary>

HomeRun pulls images using the Docker daemon's own credential store — log in with `docker login <registry>` on the host first (Docker Desktop's usual auth flow), the same as you would for any other `docker run` against that registry.

</details>

<details>
<summary><strong>Slow performance on Apple Silicon</strong></summary>

If the image only has an `amd64` build, Docker Desktop runs it under QEMU emulation, which is noticeably slower. Prefer an image with a native `arm64` build (HomeRun's own base image is multi-arch).

</details>

## What's not supported yet

- **Ephemeral (per-job) containers.** Today's container runners are long-lived, like native `App`/`Service` runners — they persist across jobs rather than being torn down and recreated per job.
- **Kubernetes.** Running runners as pods in a cluster is a separate, larger feature — see the [roadmap](../README.md#roadmap).
- **Per-runner CPU/memory limits** and **`extra_env`** in the creation UI (both exist in the underlying config already and are reachable via the API).
- **Creating** container runners from the TUI/CLI (viewing and managing them works today).

## Labels and job routing

Container runners are created with the labels `self-hosted` and `docker` by
default. GitHub also auto-adds the container's real `self-hosted`, `Linux`, and
architecture (`ARM64`/`X64`) labels. Target container runners from a workflow
with the `docker` marker:

```yaml
jobs:
  build:
    runs-on: [self-hosted, docker]
```

Purpose-built images add their own toolchain label so you can route by toolchain
— e.g. the Rust image below carries `rust`:

```yaml
jobs:
  test:
    runs-on: [self-hosted, rust]
```

## Build your own runner image

The runner binary is bind-mounted at start time, so an image only needs an OS,
`bash`, glibc, and whatever toolchain your jobs use. The first-party **Rust**
image is a worked example — extend the base image and add a toolchain:

```dockerfile
# docker/runner-rust/Dockerfile
FROM ghcr.io/agallea/homerun-runner:ubuntu-24.04

# Build deps for crates with native components (e.g. openssl-sys via reqwest):
# pkg-config + the OpenSSL headers the base image's runtime libssl3 lacks.
USER root
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
USER ubuntu

# The base image runs as the non-root `ubuntu` user, so rustup installs into
# /home/ubuntu/.cargo. `--profile minimal` omits rustfmt/clippy, but real Rust
# CI needs them (and llvm-tools-preview for coverage), so add them explicitly.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
        -c rustfmt -c clippy -c llvm-tools-preview
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"
```

Build and tag it, then create a runner from it:

```bash
docker build -t ghcr.io/agallea/homerun-runner:rust docker/runner-rust/
```

In the New Runner wizard, pick **Container** mode and the **Rust** preset (or
enter a **Custom** image). The runner registers with the `rust` label, and
`runs-on: [self-hosted, rust]` jobs land on it.

To run HomeRun's own CI you keep at least one **`rust`** runner and one base
**`docker`** runner online — see `.github/workflows/ci.yml` for how jobs are
routed.
