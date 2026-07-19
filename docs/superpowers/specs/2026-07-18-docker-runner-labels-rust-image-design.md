# Docker Runner: Labels, Rust Image, and UI Affordance — Design

**Date:** 2026-07-18
**Status:** Approved (pending spec review)
**Related:** PR #131 (`feat(runner): add Docker container mode for self-hosted runners`)

## Problem

The Docker container runner mode (PR #131) works, but three rough edges surfaced
while testing it:

1. **Wrong labels.** Container runners inherit the _host's_ default labels
   (`self-hosted, macOS, ARM64` on a Mac) even though the runner is a Linux
   container. GitHub then auto-adds the real `Linux`/`ARM64` on top, leaving a
   misleading `macOS` label and — worse — no stable way to route a job
   _specifically_ to a container runner.
2. **No toolchain story.** The first-party base image ships an OS + generic CI
   tooling but no language toolchain, so a repo whose `self-hosted` jobs expect
   e.g. Rust can't actually run on a container runner without installing the
   toolchain per-job. There's no worked example of a purpose-built image.
3. **Invisible backend.** Nothing in the UI distinguishes a container-backed
   runner from a native one at a glance.

## Goals

- Container runners get correct, routable default labels.
- Ship a first-party Rust runner image and document it as the reference example
  other users copy to build their own toolchain images.
- Make "this runner runs in Docker" obvious in the UI.

## Non-Goals

- Migrating `release-build.yml` (macOS/Windows release builds) to GitHub-hosted
  cloud runners, or adding a Linux release. That is a separate release-infra
  project with its own spec. This spec leaves `release-build.yml` untouched.
- Auto-deriving labels from arbitrary image tags.

## Design

### 1. Container-aware default labels (`crates/daemon/src/runner/mod.rs`)

Add:

```rust
fn default_container_labels() -> Vec<String> {
    vec!["self-hosted".to_string(), "docker".to_string()]
}
```

In `create()`, when the caller supplies **no** labels **and** `mode ==
RunnerMode::Container`, use `default_container_labels()` instead of the
host-based `default_runner_labels()`. GitHub's `config.sh` still auto-adds the
container's real `self-hosted`/`Linux`/arch labels, so the final set is e.g.
`[self-hosted, docker, Linux, ARM64]`.

- The `docker` marker is the stable, host-OS-independent label a workflow uses
  to target container runners: `runs-on: [self-hosted, docker]`.
- User-supplied labels are still respected verbatim (unchanged behavior).

### 2. Wizard image preset + label prefill (`apps/desktop/src/components/NewRunnerWizard.tsx`)

When mode is `container`, present a small **preset selector**:

| Preset | Image                                         | Prefilled labels            |
| ------ | --------------------------------------------- | --------------------------- |
| Base   | `ghcr.io/agallea/homerun-runner:ubuntu-24.04` | `self-hosted, docker`       |
| Rust   | `ghcr.io/agallea/homerun-runner:rust`         | `self-hosted, docker, rust` |
| Custom | (free text, current behavior)                 | `self-hosted, docker`       |

Selecting a preset fills the image field and the labels field; both remain
editable. This makes `runs-on: [self-hosted, rust]` work out of the box for the
Rust preset. Presets are frontend-only sugar — the daemon contract is unchanged
(it still receives `container.image` + `labels`).

### 3. First-party Rust image (`docker/runner-rust/Dockerfile` + publish workflow)

```dockerfile
FROM ghcr.io/agallea/homerun-runner:ubuntu-24.04
# Inherits the non-root `ubuntu` user from the base image, so rustup installs
# into /home/ubuntu/.cargo.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"
```

Add a second build+push to `.github/workflows/docker-runner-image.yml` so CI
publishes `ghcr.io/agallea/homerun-runner:rust` alongside the base image.

### 4. Docker chip in the UI (`apps/desktop/src/components/RunnerTable.tsx`, runner detail header)

Add a `DockerBadge()` component: a compact **pill** with the Docker **whale**
icon + the text **"Docker"**, rendered when `runner.config.mode ===
"container"`.

- **Runner list:** shown inline on the runner row (same location the existing
  `SvcBadge()` icon appears for service runners), as a labeled pill.
- **Runner detail header:** shown as the same labeled pill where there's room to
  read boldly.

Styling follows existing badge conventions (CSS variables, matches
`status-badge`/`SvcBadge` idiom). Whale icon is an inline SVG (no external
asset).

### 5. Docs — BYO reference example (`docs/DOCKER_RUNNERS.md`)

Expand the docs with:

- The label model: `docker` marker + per-image toolchain labels, and how
  `runs-on` matches them.
- The Rust image presented as a **worked "build your own runner image"
  example**: the `runner-rust/Dockerfile`, how to build/tag it locally, creating
  a runner from it (with the `rust` label), and a `runs-on: [self-hosted, rust]`
  workflow snippet.

### 6. Dogfood: route CI/automation jobs to container runners

Rewire the self-hosted jobs in the CI and automation workflows to the new label
scheme so HomeRun's own CI runs on container runners. `release-build.yml` is
intentionally left untouched (its cloud-runner migration is a separate spec).

| Workflow · job                        | Needs                      | `runs-on`               |
| ------------------------------------- | -------------------------- | ----------------------- |
| `ci.yml` · pre-commit                 | Rust + Node + Python       | `[self-hosted, rust]`   |
| `ci.yml` · react (test + coverage)    | Node                       | `[self-hosted, docker]` |
| `ci.yml` · rust (fmt/clippy/test/cov) | Rust                       | `[self-hosted, rust]`   |
| `ci.yml` · typescript (tsc + build)   | Node                       | `[self-hosted, docker]` |
| `coverage-badge.yml` · badge          | Rust + Node                | `[self-hosted, rust]`   |
| `release-please.yml` · release-please | Rust (`generate-lockfile`) | `[self-hosted, rust]`   |

The `rust` image inherits Node/Python from the base image, so combined Rust+Node
jobs run on a single `rust` runner. **Operational consequence:** running full CI
now requires at least one `rust` and one base (`docker`) container runner online.

## Acceptance Criteria

- Creating a container runner with empty labels yields GitHub labels including
  `docker` and **not** `macOS` (verified against the GitHub runners API).
- Every self-hosted job in `ci.yml`, `coverage-badge.yml`, and
  `release-please.yml` uses `[self-hosted, rust]` or `[self-hosted, docker]`;
  `release-build.yml` is unchanged.
- Wizard Rust preset creates a runner whose image is `…:rust` and whose labels
  include `rust`.
- `docker/runner-rust/Dockerfile` builds; `rustc`/`cargo` are on PATH as the
  `ubuntu` user; a runner created from it registers and reaches Online.
- Container runners show the whale "Docker" pill in both the list and detail
  views; native runners do not.
- `cargo fmt --check`, `cargo clippy -D warnings`, `tsc --noEmit`, and existing
  test suites pass.

## Testing

- **Daemon:** unit test for `default_container_labels()` selection in `create()`
  (empty labels + container mode → `[self-hosted, docker]`; non-empty preserved;
  non-container mode unchanged).
- **Wizard:** component test that the Rust preset sets image + labels.
- **Image:** local build + live runner registration (manual, macOS/Docker
  Desktop), same flow already validated for the base image.
- **Chip:** component test that `DockerBadge` renders for `mode==="container"`
  and is absent otherwise.
- **CI routing:** validated by a live PR run once a `rust` and a `docker`
  container runner are online — each job lands on a matching runner and passes
  (manual, since it depends on live runners).
