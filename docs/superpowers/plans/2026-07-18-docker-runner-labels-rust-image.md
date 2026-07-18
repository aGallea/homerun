# Docker Runner: Labels, Rust Image, and UI Chip — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give container runners correct routable labels, ship a first-party Rust runner image, make Docker runners visible in the UI, and dogfood it all by routing HomeRun's own CI to container runners.

**Architecture:** Small additive changes across four surfaces — daemon label defaults (Rust), a new derived Docker image, React UI (wizard preset + badge), and workflow `runs-on` edits. No daemon API contract changes; presets are frontend sugar. Container runners are identified by a stable `docker` label; per-image toolchain labels (e.g. `rust`) route jobs.

**Tech Stack:** Rust (axum daemon), React 19 + TypeScript + Vite (Tauri desktop), Docker/buildx, GitHub Actions.

## Global Constraints

- Conventional Commits required (`feat`, `fix`, `docs`, `chore`, ...). Pre-commit hook enforces.
- Rust: `cargo fmt` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean; 4-space indent, 100 col.
- TypeScript: Prettier (double quotes, semicolons, trailing commas, 2-space, 100 col); strict (`noUnusedLocals`/`noUnusedParameters`); `npx tsc --noEmit` clean.
- Base runner image is `ghcr.io/agallea/homerun-runner:ubuntu-24.04`; GHCR names are lowercase (`agallea`).
- Every commit message ends with:

  ```
  Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG
  ```

- The pre-commit hook reformats files (prettier) and may abort the first commit; re-stage and re-commit.

---

### Task 0: Commit the pending container fixes (prerequisite)

The working tree already contains verified, uncommitted fixes from earlier
debugging that the rest of this plan builds on (the Rust image in Task 2 depends
on the **non-root base image**). Commit them first so the branch is clean.

**Files (already modified, not yet committed):**

- `crates/daemon/src/runner/docker.rs` — sets `RUNNER_ALLOW_RUNASROOT=1` in the container env + deregister helper
- `docker/runner-base/Dockerfile` — runs as non-root `USER ubuntu`, owns `/workspace`
- `apps/desktop/src/utils/openExternal.ts` — new helper routing links through the shell plugin
- `apps/desktop/src/components/NewRunnerWizard.tsx` — "Docker Runners" link uses `openExternal`

- [ ] **Step 1: Review what's staged**

Run: `git status --short && git diff --stat`
Expected: the four files above appear as modified/untracked.

- [ ] **Step 2: Verify the daemon still builds clean**

Run: `cargo fmt --check -p homerund && cargo clippy -p homerund --all-targets -- -D warnings`
Expected: no output / exits 0.

- [ ] **Step 3: Verify the frontend type-checks**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no output / exits 0.

- [ ] **Step 4: Commit**

```bash
git add crates/daemon/src/runner/docker.rs docker/runner-base/Dockerfile \
        apps/desktop/src/utils/openExternal.ts apps/desktop/src/components/NewRunnerWizard.tsx
git commit -m "fix(runner): run container runners as non-root and open links via shell

Container images commonly run as root; GitHub's config.sh refuses that, so set
RUNNER_ALLOW_RUNASROOT and run the base image as the non-root ubuntu user.
Route the wizard's external link through the shell plugin (WKWebView drops
target=_blank).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

Expected: commit succeeds (re-stage + re-run if prettier reformats).

---

### Task 1: Daemon — container-aware default labels

**Files:**

- Modify: `crates/daemon/src/runner/mod.rs` (add `default_container_labels()` near `default_runner_labels()` at line ~127; change label resolution in `create()` at line ~1013)
- Test: `crates/daemon/src/runner/mod.rs` (tests module, near existing `create` tests)

**Interfaces:**

- Produces: `fn default_container_labels() -> Vec<String>` returning `vec!["self-hosted", "docker"]`.
- Consumes: existing `create(&self, repo_full_name, name, labels, mode, group_id, container)`; `RunnerMode` (already imported), `types::ContainerConfig`.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/daemon/src/runner/mod.rs`:

```rust
#[tokio::test]
async fn test_container_mode_empty_labels_default_to_docker() {
    let manager = create_test_manager();
    let runner = manager
        .create(
            "owner/repo",
            None,
            None,
            Some(RunnerMode::Container),
            None,
            Some(types::ContainerConfig {
                image: "img:latest".to_string(),
                extra_env: vec![],
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        runner.config.labels,
        vec!["self-hosted".to_string(), "docker".to_string()]
    );
}

#[tokio::test]
async fn test_container_mode_user_labels_preserved() {
    let manager = create_test_manager();
    let runner = manager
        .create(
            "owner/repo",
            None,
            Some(vec!["self-hosted".to_string(), "rust".to_string()]),
            Some(RunnerMode::Container),
            None,
            Some(types::ContainerConfig {
                image: "img:latest".to_string(),
                extra_env: vec![],
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        runner.config.labels,
        vec!["self-hosted".to_string(), "rust".to_string()]
    );
}

#[tokio::test]
async fn test_non_container_mode_does_not_get_docker_label() {
    let manager = create_test_manager();
    let runner = manager
        .create("owner/repo", None, None, Some(RunnerMode::App), None, None)
        .await
        .unwrap();
    assert!(runner.config.labels.contains(&"self-hosted".to_string()));
    assert!(!runner.config.labels.contains(&"docker".to_string()));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerund test_container_mode_empty_labels_default_to_docker test_container_mode_user_labels_preserved test_non_container_mode_does_not_get_docker_label`
Expected: FAIL — `test_container_mode_empty_labels_default_to_docker` asserts `["self-hosted","docker"]` but current code produces host labels (`self-hosted, macOS/Linux, arch`).

- [ ] **Step 3: Add `default_container_labels()`**

Immediately after `default_runner_labels()` (ends ~line 145) in `crates/daemon/src/runner/mod.rs`:

```rust
/// Default labels for a container-mode runner. Unlike `default_runner_labels`,
/// these are independent of the daemon's host OS — the runner is a Linux
/// container regardless of host. GitHub's `config.sh` auto-adds the real
/// `self-hosted`/OS/arch labels on top; `docker` is the stable marker workflows
/// use to target container runners (`runs-on: [self-hosted, docker]`).
fn default_container_labels() -> Vec<String> {
    vec!["self-hosted".to_string(), "docker".to_string()]
}
```

- [ ] **Step 4: Use it in `create()`**

In `create()`, replace the existing `resolved_labels` block (lines ~1013-1024):

```rust
        let resolved_labels = if let Some(user_labels) = labels {
            if user_labels.is_empty() {
                // No labels provided — use platform defaults
                default_runner_labels()
            } else {
                // User explicitly chose labels — use as-is
                user_labels
            }
        } else {
            // None — use platform defaults
            default_runner_labels()
        };
```

with:

```rust
        // Container runners are Linux regardless of host, and need a stable
        // `docker` marker for routing; native runners keep host platform labels.
        let platform_defaults = if matches!(mode.as_ref(), Some(RunnerMode::Container)) {
            default_container_labels()
        } else {
            default_runner_labels()
        };
        let resolved_labels = match labels {
            Some(user_labels) if !user_labels.is_empty() => user_labels,
            _ => platform_defaults,
        };
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p homerund test_container_mode_empty_labels_default_to_docker test_container_mode_user_labels_preserved test_non_container_mode_does_not_get_docker_label`
Expected: PASS (3 passed).

- [ ] **Step 6: Lint + full daemon tests**

Run: `cargo fmt -p homerund && cargo clippy -p homerund --all-targets --all-features -- -D warnings && cargo test -p homerund`
Expected: clippy clean; all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/daemon/src/runner/mod.rs
git commit -m "feat(runner): container runners default to self-hosted,docker labels

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

### Task 2: First-party Rust runner image + publish

**Files:**

- Create: `docker/runner-rust/Dockerfile`
- Modify: `.github/workflows/docker-runner-image.yml` (add a second build-push step)

**Interfaces:**

- Produces: image `ghcr.io/agallea/homerun-runner:rust` with `cargo`/`rustc` on PATH as the non-root `ubuntu` user.
- Consumes: base image `ghcr.io/agallea/homerun-runner:ubuntu-24.04` (non-root, from Task 0).

- [ ] **Step 1: Create the Rust image Dockerfile**

Create `docker/runner-rust/Dockerfile`:

```dockerfile
# HomeRun Rust runner image — a worked example of a purpose-built runner image.
#
# Extends the base image (OS + CI tooling + non-root `ubuntu` user) with a Rust
# toolchain so jobs labeled `rust` can run cargo without a per-job install. Use
# this same FROM-the-base pattern to build your own toolchain images.
FROM ghcr.io/agallea/homerun-runner:ubuntu-24.04

# The base image ends with `USER ubuntu`, so this RUN executes as the non-root
# ubuntu user and rustup installs into /home/ubuntu/.cargo.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"
```

- [ ] **Step 2: Build it locally and verify the toolchain**

Run:

```bash
docker build -t ghcr.io/agallea/homerun-runner:rust docker/runner-rust/
docker run --rm ghcr.io/agallea/homerun-runner:rust bash -lc 'id; cargo --version; rustc --version'
```

Expected: `uid=1000(ubuntu)`, a `cargo x.y.z` line, and a `rustc x.y.z` line.
(Requires the base image built locally — see Task 0. If missing: `docker build -t ghcr.io/agallea/homerun-runner:ubuntu-24.04 docker/runner-base/`.)

- [ ] **Step 3: Add the publish step to the workflow**

In `.github/workflows/docker-runner-image.yml`, after the existing `Build and push` step (base image), append a second step (same indentation, inside `steps:`):

```yaml
- name: Build and push (rust)
  uses: docker/build-push-action@v6
  with:
    context: docker/runner-rust
    platforms: linux/amd64,linux/arm64
    push: true
    tags: |
      ghcr.io/${{ github.repository_owner }}/homerun-runner:rust
      ghcr.io/${{ github.repository_owner }}/homerun-runner:rust-${{ github.ref_name }}
```

Also update the job `name:` for clarity — change `Build and push base runner image` to `Build and push runner images`.

Note: the base step runs first and pushes `:ubuntu-24.04`; the rust step's
`FROM` then resolves against that just-pushed image.

- [ ] **Step 4: Validate the workflow YAML**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/docker-runner-image.yml'))" && echo OK`
Expected: `OK`.

- [ ] **Step 5: Commit**

```bash
git add docker/runner-rust/Dockerfile .github/workflows/docker-runner-image.yml
git commit -m "feat(docker): add first-party Rust runner image and publish it

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

### Task 3: Docker chip in the UI

**Files:**

- Create: `apps/desktop/src/components/DockerBadge.tsx`
- Create: `apps/desktop/src/components/DockerBadge.test.tsx`
- Modify: `apps/desktop/src/components/RunnerTable.tsx` (import + render, near `SvcBadge` usage at lines ~385/388)
- Modify: `apps/desktop/src/pages/RunnerDetail.tsx` (import + render in header row at line ~416)

**Interfaces:**

- Produces: `export function DockerBadge(): JSX.Element` — a labeled pill (whale icon + "Docker").

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/components/DockerBadge.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DockerBadge } from "./DockerBadge";

describe("DockerBadge", () => {
  it("renders the Docker label", () => {
    render(<DockerBadge />);
    expect(screen.getByText("Docker")).toBeInTheDocument();
  });

  it("has a descriptive title", () => {
    render(<DockerBadge />);
    expect(screen.getByTitle("Runs in a Docker container")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/desktop && npx vitest run src/components/DockerBadge.test.tsx`
Expected: FAIL — cannot resolve `./DockerBadge`.

- [ ] **Step 3: Create the component**

Create `apps/desktop/src/components/DockerBadge.tsx`:

```tsx
/** Labeled pill marking a container-backed (Docker) runner. */
export function DockerBadge() {
  return (
    <span
      title="Runs in a Docker container"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 4,
        padding: "1px 8px",
        fontSize: 11,
        fontWeight: 600,
        color: "var(--accent-blue)",
        background: "rgba(59, 130, 246, 0.1)",
        border: "1px solid var(--accent-blue)",
        borderRadius: 999,
        whiteSpace: "nowrap",
      }}
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M13.98 11.08h2.12a.19.19 0 0 0 .19-.19V9a.19.19 0 0 0-.19-.19h-2.12a.19.19 0 0 0-.18.19v1.9c0 .1.08.18.18.18m-2.95-5.43h2.12a.19.19 0 0 0 .18-.19V3.57a.19.19 0 0 0-.18-.18h-2.12a.19.19 0 0 0-.19.18v1.89c0 .1.09.19.19.19m0 2.71h2.12a.19.19 0 0 0 .18-.18V6.29a.19.19 0 0 0-.18-.19h-2.12a.19.19 0 0 0-.19.19v1.89c0 .1.09.18.19.18m-2.93 0h2.12a.19.19 0 0 0 .18-.18V6.29a.19.19 0 0 0-.18-.19H8.1a.19.19 0 0 0-.19.19v1.89c0 .1.08.18.19.18m-2.96 0h2.12a.19.19 0 0 0 .18-.18V6.29a.19.19 0 0 0-.18-.19H5.14a.19.19 0 0 0-.19.19v1.89c0 .1.08.18.19.18m5.89 2.72h2.12a.19.19 0 0 0 .18-.19V9a.19.19 0 0 0-.18-.19h-2.12a.19.19 0 0 0-.19.19v1.9c0 .1.09.18.19.18m-2.93 0h2.12a.19.19 0 0 0 .18-.19V9a.19.19 0 0 0-.18-.19H8.1a.19.19 0 0 0-.19.19v1.9c0 .1.08.18.19.18m-2.96 0h2.12a.19.19 0 0 0 .18-.19V9a.19.19 0 0 0-.19-.19H5.14a.19.19 0 0 0-.19.19v1.9c0 .1.08.18.19.18m17.5-1.4c-.06-.05-.67-.51-1.95-.51-.34 0-.68.03-1.01.09-.25-1.7-1.65-2.53-1.72-2.57l-.34-.2-.23.33c-.28.44-.49.92-.61 1.43-.23.97-.09 1.88.4 2.66-.6.33-1.55.41-1.74.42H.75a.75.75 0 0 0-.75.75 11.4 11.4 0 0 0 .69 4.06c.55 1.43 1.36 2.48 2.41 3.12 1.18.72 3.1 1.14 5.28 1.14.98 0 1.96-.09 2.93-.27a12.2 12.2 0 0 0 3.82-1.39c.98-.56 1.86-1.29 2.61-2.13 1.25-1.42 2-3 2.55-4.4h.22c1.37 0 2.22-.55 2.68-1.01.31-.29.55-.65.71-1.05l.1-.29z" />
      </svg>
      Docker
    </span>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/desktop && npx vitest run src/components/DockerBadge.test.tsx`
Expected: PASS (2 passed).

- [ ] **Step 5: Render it in the runner list**

In `apps/desktop/src/components/RunnerTable.tsx`:

Add the import near the top (with the other component imports):

```tsx
import { DockerBadge } from "./DockerBadge";
```

At line ~385, change:

```tsx
{
  runner.config.mode === "service" && <SvcBadge />;
}
```

to:

```tsx
{
  runner.config.mode === "service" && <SvcBadge />;
}
{
  runner.config.mode === "container" && <DockerBadge />;
}
```

At line ~388, change:

```tsx
{
  !indented && runner.config.mode === "service" && <SvcBadge />;
}
```

to:

```tsx
{
  !indented && runner.config.mode === "service" && <SvcBadge />;
}
{
  !indented && runner.config.mode === "container" && <DockerBadge />;
}
```

- [ ] **Step 6: Render it in the runner detail header**

In `apps/desktop/src/pages/RunnerDetail.tsx`:

Add the import near the top (with other component imports):

```tsx
import { DockerBadge } from "../components/DockerBadge";
```

In the header row at line ~416-417, change:

```tsx
        <div className="flex items-center gap-16">
          <StatusPill state={state} currentJob={current_job} />
```

to:

```tsx
        <div className="flex items-center gap-16">
          {config.mode === "container" && <DockerBadge />}
          <StatusPill state={state} currentJob={current_job} />
```

- [ ] **Step 7: Type-check + tests**

Run: `cd apps/desktop && npx tsc --noEmit && npx vitest run src/components/DockerBadge.test.tsx`
Expected: tsc clean; tests pass.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/components/DockerBadge.tsx apps/desktop/src/components/DockerBadge.test.tsx \
        apps/desktop/src/components/RunnerTable.tsx apps/desktop/src/pages/RunnerDetail.tsx
git commit -m "feat(desktop): show a Docker pill on container-backed runners

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

### Task 4: Wizard image preset selector

**Files:**

- Modify: `apps/desktop/src/components/NewRunnerWizard.tsx`
- Test: `apps/desktop/src/components/NewRunnerWizard.test.tsx` (add one test)

**Interfaces:**

- Consumes: existing `containerImage`/`setContainerImage`, `labelsInput`/`setLabelsInput`, `mode`, and `StepConfigure`.
- Produces: a `preset` state (`"base" | "rust" | "custom"`) and preset buttons inside the container block.

- [ ] **Step 1: Write the failing test**

Add to `apps/desktop/src/components/NewRunnerWizard.test.tsx` inside the `describe("NewRunnerWizard", ...)` block:

```tsx
it("Rust preset fills the rust image and rust label", async () => {
  const { props } = await renderWizard();
  await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

  fireEvent.click(screen.getByText("org/frontend"));
  await waitFor(() => expect(screen.getByText("Container")).toBeInTheDocument());
  fireEvent.click(screen.getByText("Container"));

  fireEvent.click(screen.getByRole("button", { name: "Rust" }));
  const imageInput = screen.getByLabelText("Image") as HTMLInputElement;
  expect(imageInput.value).toBe("ghcr.io/agallea/homerun-runner:rust");

  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

  await waitFor(() => expect(props.onCreate).toHaveBeenCalledTimes(1));
  expect(props.onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      mode: "container",
      container: { image: "ghcr.io/agallea/homerun-runner:rust" },
      labels: expect.arrayContaining(["rust"]),
    }),
  );
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/desktop && npx vitest run src/components/NewRunnerWizard.test.tsx -t "Rust preset"`
Expected: FAIL — no button named "Rust".

- [ ] **Step 3: Add preset constants and state**

In `apps/desktop/src/components/NewRunnerWizard.tsx`, near the existing
`DEFAULT_CONTAINER_IMAGE` constant (line ~14):

```tsx
const DEFAULT_CONTAINER_IMAGE = "ghcr.io/agallea/homerun-runner:ubuntu-24.04";
const RUST_CONTAINER_IMAGE = "ghcr.io/agallea/homerun-runner:rust";
type ContainerPreset = "base" | "rust" | "custom";
```

In the component body, near the other `useState` hooks (with `containerImage`):

```tsx
const [preset, setPreset] = useState<ContainerPreset>("base");
```

Add a preset-apply helper and an effect that applies the current preset whenever
Container mode is entered (place after the existing hooks, before the JSX):

```tsx
const applyPreset = (p: ContainerPreset) => {
  setPreset(p);
  if (p === "base") {
    setContainerImage(DEFAULT_CONTAINER_IMAGE);
    setLabelsInput("self-hosted, docker");
  } else if (p === "rust") {
    setContainerImage(RUST_CONTAINER_IMAGE);
    setLabelsInput("self-hosted, docker, rust");
  }
  // "custom": leave the current image/labels for the user to edit.
};

useEffect(() => {
  if (mode === "container") applyPreset(preset);
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, [mode]);
```

- [ ] **Step 4: Thread preset into `StepConfigure` and render the selector**

In the `StepConfigure` render (where `mode`, `containerImage` etc. are passed, line ~236), add:

```tsx
preset = { preset };
onPreset = { applyPreset };
```

In `StepConfigureProps` (line ~403 area) add:

```tsx
  preset: ContainerPreset;
  onPreset: (p: ContainerPreset) => void;
```

Destructure them in the `StepConfigure` signature (line ~418 area): add `preset,` and `onPreset,`.

Inside the existing `{mode === "container" && (` block (line ~577), immediately
before the `<label ... htmlFor="runner-image">Image</label>` form-group, insert:

```tsx
{
  mode === "container" && (
    <div className="form-group">
      <label className="form-label">Image preset</label>
      <div style={{ display: "flex", gap: 8 }}>
        {(["base", "rust", "custom"] as const).map((p) => (
          <button
            key={p}
            type="button"
            onClick={() => onPreset(p)}
            style={{
              flex: 1,
              padding: "8px 10px",
              fontSize: 12,
              fontWeight: 600,
              textTransform: "capitalize",
              background: preset === p ? "rgba(59, 130, 246, 0.08)" : "var(--bg-tertiary)",
              border: `1.5px solid ${preset === p ? "var(--accent-blue)" : "var(--border)"}`,
              borderRadius: 8,
              color: "var(--text-primary)",
              cursor: "pointer",
            }}
          >
            {p}
          </button>
        ))}
      </div>
    </div>
  );
}
```

(Leave the existing Image input form-group as the next sibling.)

- [ ] **Step 5: Ensure `useEffect` is imported**

The file already imports `{ useState, useMemo, useEffect }` from `"react"` (added earlier). Confirm `useEffect` is in the import; if not, add it.

- [ ] **Step 6: Run the wizard tests**

Run: `cd apps/desktop && npx vitest run src/components/NewRunnerWizard.test.tsx`
Expected: PASS — the new "Rust preset" test and all pre-existing tests (including "selects Container mode and sends the image", which still sees the default base image).

- [ ] **Step 7: Type-check**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: no output / exits 0.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/components/NewRunnerWizard.tsx apps/desktop/src/components/NewRunnerWizard.test.tsx
git commit -m "feat(desktop): add Base/Rust/Custom image presets to the runner wizard

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

### Task 5: Route CI/automation workflows to container runners

**Files:**

- Modify: `.github/workflows/ci.yml` (4 jobs)
- Modify: `.github/workflows/coverage-badge.yml` (1 job)
- Modify: `.github/workflows/release-please.yml` (1 job)

`release-build.yml` is intentionally NOT changed.

- [ ] **Step 1: Edit `ci.yml`**

- Line ~27 (`pre-commit`): `runs-on: self-hosted` → `runs-on: [self-hosted, rust]`
- Line ~95 (`react`): `runs-on: self-hosted` → `runs-on: [self-hosted, docker]`
- Line ~123 (`rust`): `runs-on: self-hosted` → `runs-on: [self-hosted, rust]`
- Line ~188 (`typescript`): `runs-on: self-hosted` → `runs-on: [self-hosted, docker]`

- [ ] **Step 2: Edit `coverage-badge.yml`**

- Line ~13 (`badge`): `runs-on: self-hosted` → `runs-on: [self-hosted, rust]`

- [ ] **Step 3: Edit `release-please.yml`**

- Line ~13 (`release-please`): `runs-on: self-hosted` → `runs-on: [self-hosted, rust]`

- [ ] **Step 4: Verify no bare `self-hosted` remains in these three files, and YAML is valid**

Run:

```bash
grep -rn "runs-on: self-hosted$" .github/workflows/ci.yml .github/workflows/coverage-badge.yml .github/workflows/release-please.yml; echo "exit=$?"
for f in ci coverage-badge release-please; do python3 -c "import yaml;yaml.safe_load(open('.github/workflows/$f.yml'))"; done && echo "YAML OK"
```

Expected: grep prints nothing and `exit=1` (no matches); then `YAML OK`. `release-build.yml` untouched.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/coverage-badge.yml .github/workflows/release-please.yml
git commit -m "ci: route self-hosted jobs to rust/docker container runners

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

### Task 6: Documentation — labels + Rust image BYO reference

**Files:**

- Modify: `docs/DOCKER_RUNNERS.md`

- [ ] **Step 1: Append the label-routing and Rust-image sections**

Add the following sections to `docs/DOCKER_RUNNERS.md` (place after the existing
usage/requirements content; keep existing headings intact):

````markdown
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

# The base image runs as the non-root `ubuntu` user, so rustup installs into
# /home/ubuntu/.cargo.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal
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
````

- [ ] **Step 2: Verify it renders (no broken code fences)**

Run: `npx --yes markdownlint-cli docs/DOCKER_RUNNERS.md || true`
Expected: no fatal errors (warnings acceptable; match repo's existing style).

- [ ] **Step 3: Commit**

```bash
git add docs/DOCKER_RUNNERS.md
git commit -m "docs(docker-runners): document label routing and BYO Rust image

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01SU8ACjwANSEWMHUBPvoaBG"
```

---

## Final verification (after all tasks)

- [ ] `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
- [ ] `cd apps/desktop && npx tsc --noEmit && npx vitest run && npx prettier --check src/`
- [ ] Live dogfood (manual, needs the branch daemon running):
  - Build `ghcr.io/agallea/homerun-runner:rust` locally (Task 2).
  - Create one runner with the **Rust** preset and one with the **Base** preset.
  - Confirm both reach Online and carry the expected labels (`gh api repos/aGallea/homerun/actions/runners`), with `docker`/`rust` present and no `macOS`.
  - Open a PR (or re-run CI) and confirm the `rust` jobs land on the rust runner and the node-only jobs on the base runner.
