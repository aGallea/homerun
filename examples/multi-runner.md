# Multi-Runner Setup

Run multiple GitHub Actions runners in parallel to dramatically reduce CI wait times. HomeRun makes it easy to spin up a fleet of runners for one or many repositories.

## Why Multiple Runners?

GitHub Actions jobs run one at a time per runner. With multiple runners:
- Parallel jobs in a single workflow actually run in parallel
- Multiple PRs can run CI simultaneously
- Different job types can be isolated to different runners with custom labels

---

## Quick: Add Multiple Runners at Once

```sh
# Add 4 runners for a repo (auto-named: my-runner-1 through my-runner-4)
homerun --no-tui add my-runner --repo owner/my-repo --count 4
```

Check the result:

```sh
homerun --no-tui list
```

```
NAME           REPO             STATUS   MODE          LABELS
my-runner-1    owner/my-repo    Online   app-managed   self-hosted,macOS,ARM64
my-runner-2    owner/my-repo    Online   app-managed   self-hosted,macOS,ARM64
my-runner-3    owner/my-repo    Online   app-managed   self-hosted,macOS,ARM64
my-runner-4    owner/my-repo    Online   app-managed   self-hosted,macOS,ARM64
```

---

## Runners with Custom Labels

Use labels to route specific jobs to specific runners. This is useful for separating fast unit tests from slow integration tests, or running jobs that require specific tools.

```sh
# Runners for unit tests (lightweight)
homerun --no-tui add unit-runner --repo owner/my-repo --count 2 --labels self-hosted,macOS,unit

# Runners for integration/E2E tests (heavier)
homerun --no-tui add e2e-runner --repo owner/my-repo --count 2 --labels self-hosted,macOS,e2e
```

Target them in workflows:

```yaml
jobs:
  unit-tests:
    runs-on: [self-hosted, unit]
    steps:
      - run: cargo test --lib

  e2e-tests:
    runs-on: [self-hosted, e2e]
    steps:
      - run: cargo test --test '*'
```

---

## Runners Across Multiple Repos

```sh
# Repo 1
homerun --no-tui add ci-runner --repo owner/repo-a --count 2 --mode service

# Repo 2
homerun --no-tui add ci-runner --repo owner/repo-b --count 2 --mode service

# Repo 3
homerun --no-tui add ci-runner --repo owner/repo-c --count 1 --mode service
```

All runners appear in the same TUI and dashboard view, grouped by repo.

---

## Service Mode for Persistent Runners

For long-lived multi-runner setups, use service mode so runners survive reboots and don't depend on the daemon staying alive:

```sh
homerun --no-tui add ci-runner --repo owner/my-repo --count 4 --mode service
```

Each runner gets its own launchd plist and starts automatically on login.

---

## Smart Repo Discovery

Not sure which repos need self-hosted runners? HomeRun can scan for you:

```sh
# Scan your local workspace
homerun --no-tui scan ~/workspace

# Scan your GitHub repos via API
homerun --no-tui scan --remote

# Both at once
homerun --no-tui scan ~/workspace --remote
```

Output example:

```
Found 3 repos using runs-on: self-hosted:

  owner/repo-a    2 workflows, 0 active runners
  owner/repo-b    1 workflow,  0 active runners
  owner/repo-c    3 workflows, 1 active runner

Run `homerun --no-tui add` to set up runners for any of these.
```

---

## Using the TUI for Fleet Management

The TUI is well-suited for managing many runners at once:

```sh
homerun
```

Key operations:
- `↑↓` — navigate runner list
- `a` — add new runner (wizard)
- `d` — delete selected runner (with confirmation)
- `s` — start/stop toggle
- `r` — restart
- `l` — view live logs
- `4` — switch to Monitoring tab (aggregate resource view)

---

## Workflow: Parallel Matrix Build

A common use case for multiple runners — running a build matrix in parallel:

```yaml
jobs:
  test:
    runs-on: [self-hosted, macOS]
    strategy:
      matrix:
        rust: [stable, beta, nightly]
        features: [default, all-features]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      - run: cargo test ${{ matrix.features == 'all-features' && '--all-features' || '' }}
```

With 4+ runners, all 6 matrix combinations run simultaneously.

---

## Resource Considerations

Each GitHub Actions runner is lightweight when idle (< 50 MB RAM). Under load, resource usage depends entirely on what the workflows do.

Monitor aggregate usage in HomeRun:

```sh
homerun --no-tui status --verbose
```

Or use the Monitoring tab in the TUI / desktop app for per-runner CPU/RAM graphs.

---

## Next Steps

- [Workflow templates](workflow-templates/self-hosted.yml) — ready-to-use workflow configurations
- [Basic setup](basic-setup.md) — single runner walkthrough
