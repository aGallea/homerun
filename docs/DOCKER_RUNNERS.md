# Docker Runners

HomeRun can run a self-hosted runner inside a Docker container instead of as a native process on your machine. This gives you process isolation and an easy way to pin the exact toolchain a runner sees, without changing anything about how HomeRun talks to GitHub.

## How it works

A container runner uses the same registration/lifecycle flow as a native runner — the daemon still gets a registration token from GitHub, still runs `config.sh` then `run.sh`, and still streams logs and job events the same way. The only thing that changes is *where* those scripts execute.

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

## Base images vs. custom images

HomeRun publishes one first-party base image — Ubuntu 24.04 with common CI tooling (git, curl, build-essential, node, python3) — to `ghcr.io/agallea/homerun-runner`. Its `Dockerfile` lives at [`docker/runner-base/Dockerfile`](../docker/runner-base/Dockerfile).

You can also point a runner at **any image you supply** — your own registry ref, a locally built/tagged image, or another CI vendor's image. There's no daemon-side allowlist; the image just needs to meet the constraint below.

## Constraint: the runner needs glibc

The GitHub Actions runner is a .NET application and needs a glibc-based Linux userland plus a handful of runtime libraries (`libicu`, `libssl`, `libkrb5`, `zlib`) — the same dependencies [`actions/runner`'s own `installdependencies.sh`](https://github.com/actions/runner/blob/main/src/Misc/layoutbin/installdependencies.sh) installs. **Alpine (musl) images will not work** unless you use the separate Alpine-specific runner build, which HomeRun does not currently support. Debian/Ubuntu-based images are the safe default.

If an image is missing a runtime dependency, `config.sh`/`run.sh` will fail to start inside the container — the runner surfaces this as an `Error` state with the container's stderr attached, rather than hanging silently in `Registering`.

## Prerequisites

- Docker Desktop (macOS/Windows) or the Docker daemon (Linux) running and reachable. HomeRun checks this before offering the "Container" mode in the runner creation wizard.
- On Apple Silicon / ARM hosts, make sure the image you choose has an `arm64` build, or expect emulation overhead.

## What's not supported yet

- **Ephemeral (per-job) containers.** Today's container runners are long-lived, like native `App`/`Service` runners — they persist across jobs rather than being torn down and recreated per job.
- **Kubernetes.** Running runners as pods in a cluster is a separate, larger feature — see the [roadmap](../README.md#roadmap).
- **Per-runner CPU/memory limits** in the creation UI (the underlying config has room to grow this later).
