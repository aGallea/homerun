# Basic Setup: Your First Self-Hosted Runner

This walkthrough gets you from zero to a running GitHub Actions self-hosted runner in under 5 minutes.

## Prerequisites

- HomeRun installed (see [README](../README.md#quick-start))
- A GitHub account
- A repository where you want to run workflows locally

---

## Step 1: Start the Daemon

The daemon is the background process that manages your runners. Start it once and leave it running.

```sh
homerund
```

Or, to auto-start on login:

```sh
# Enable auto-start (installs a launchd plist)
homerun --no-tui daemon autostart enable
```

---

## Step 2: Authenticate with GitHub

### Option A: OAuth (recommended)

```sh
homerun --no-tui login
```

This opens your browser to GitHub's OAuth consent screen. Approve access and HomeRun stores your token securely in the macOS Keychain.

### Option B: Personal Access Token

```sh
homerun --no-tui login --token ghp_your_token_here
```

Required scopes:

- Personal repos: `repo`
- Organization repos: `repo` + `manage_runners:org`

---

## Step 3: Add a Runner

```sh
homerun --no-tui add my-runner --repo owner/my-repo
```

HomeRun will:

1. Download the GitHub Actions runner binary (cached for future use)
2. Register the runner with your repo via the GitHub API
3. Start the runner process

You should see output like:

```
Creating runner my-runner for owner/my-repo...
Downloading runner binary v2.319.1... done (cached)
Registering with GitHub... done
Starting runner process... done

Runner my-runner is Online
Labels: self-hosted, macOS, ARM64
Mode: app-managed
```

---

## Step 4: Verify It's Working

Check runner status:

```sh
homerun --no-tui status
```

```
NAME         REPO              STATUS   MODE          CPU    LABELS
my-runner    owner/my-repo     Online   app-managed   0.1%   self-hosted,macOS,ARM64
```

You can also verify on GitHub: go to your repo → Settings → Actions → Runners.

---

## Step 5: Use the Runner in a Workflow

Create or update a workflow file in your repo:

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: self-hosted # <-- this targets your HomeRun runner
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: cargo test
```

Push the file and trigger a workflow run. HomeRun will pick it up automatically.

---

## Optional: Run as a Background Service

By default, the runner is "app-managed" — it runs as a child of the daemon and stops if you quit HomeRun. To make it persist across reboots:

```sh
homerun --no-tui add my-runner --repo owner/my-repo --mode service
```

Or switch an existing runner:

```sh
homerun --no-tui set-mode my-runner service
```

Service mode installs a launchd plist for the runner. It starts automatically on login, independent of the daemon.

---

## Managing Your Runner

```sh
# Stop a runner (graceful — waits for current job to finish)
homerun --no-tui stop my-runner

# Start it again
homerun --no-tui start my-runner

# Restart
homerun --no-tui restart my-runner

# Delete (stops, deregisters from GitHub, removes local files)
homerun --no-tui remove my-runner
```

---

## Using the TUI Instead

All of the above is also available in the interactive TUI:

```sh
homerun
```

Use `a` to add a runner, `↑↓` to navigate, `s` to start/stop, `l` to view logs, `?` for help.

---

## Next Steps

- [Multi-runner setup](multi-runner.md) — run multiple runners for parallelism
- [Self-hosted workflow templates](workflow-templates/self-hosted.yml) — ready-to-use workflow files
