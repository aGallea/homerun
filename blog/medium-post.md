# I Was Tired of Babysitting GitHub Actions Runners — So I Built HomeRun

## Self-hosted runners shouldn't require a 15-step ritual. Here's how I replaced shell scripts with a single command.

![Hero image: HomeRun logo or a split screen showing a messy terminal full of manual setup commands on the left vs. a clean HomeRun TUI/dashboard on the right]
`[IMAGE: hero-banner.png — Hero image showing HomeRun in action]`

---

If you've ever set up a GitHub Actions self-hosted runner, you know the drill:

1. Go to repo Settings → Actions → Runners → New self-hosted runner
2. Copy a download URL
3. Paste it into a terminal
4. Extract the tarball
5. Copy a registration token
6. Run `config.sh` with the right flags
7. Run `run.sh`
8. Pray it doesn't die overnight

Now do that for 4 runners. Across 6 repos. And monitor them. And restart the ones that crash.

I got tired of this. So I built **HomeRun** — a tool that turns all of the above into:

```sh
homerund                                          # start the daemon
homerun --no-tui login                            # authenticate via browser
homerun --no-tui add ci-runner --repo me/app --count 4  # done.
```

Three commands. Four runners. Zero copy-pasting.

---

## What Is HomeRun?

HomeRun is an open-source macOS tool for managing GitHub Actions self-hosted runners. It's a daemon that handles the entire runner lifecycle — downloading binaries, registering with GitHub, spawning processes, monitoring health, streaming logs, and auto-restarting on failure.

It comes with three interfaces:

- **A desktop app** (Tauri + React) for visual management
- **A terminal UI** (Ratatui) for keyboard-driven power users
- **A CLI mode** for scripting and automation

`[IMAGE: screenshot-tui.png — Screenshot of the HomeRun TUI showing runners across multiple repos with status, CPU/RAM usage, and job info]`

---

## The Problem with Self-Hosted Runners

GitHub Actions is excellent. Self-hosted runners are powerful — they let you run CI on your own hardware, access private networks, use specific architectures, and avoid per-minute billing.

But the **management experience** is stuck in 2019:

| What you need to do | GitHub's solution |
|---|---|
| Set up a runner | Copy-paste 6 shell commands from the UI |
| Monitor runner health | Check the Settings page manually |
| See live logs | SSH into the machine and `tail -f` |
| Restart a crashed runner | Hope you set up a systemd service |
| Scale to multiple runners | Repeat everything N times |
| Track which runner ran which job | Good luck |

There's no unified dashboard, no real-time observability, and no simple way to scale.

---

## How HomeRun Fixes This

### 1. One-Command Setup

HomeRun caches the official GitHub Actions runner binary locally. When you create a new runner, it pulls from cache, registers with GitHub's API, and spawns the process — all automatically.

```sh
homerun --no-tui add my-runner --repo owner/repo --count 4 --labels ci,fast
```

That's it. Four runners, labeled for job routing, up and running.

### 2. Browser-Based Authentication

No more generating Personal Access Tokens and pasting them around. HomeRun uses GitHub's Device Flow — it opens your browser, you authorize, and the token is stored securely in your macOS Keychain.

```sh
homerun --no-tui login
# Opens browser → authorize → done
```

(PAT-based auth is still available if you prefer it.)

### 3. Real-Time Dashboard

Whether you use the desktop app or the TUI, you get a single pane of glass across all your runners and repos:

- **Live status** — Online, Busy, Offline, Error
- **Current job** — Which workflow and job name is running right now
- **CPU & RAM** — Per-runner resource usage, updated in real time via WebSocket
- **Job history** — Completed/failed counters, step-level log inspection
- **Log streaming** — Tail runner output live, no SSH required

`[IMAGE: screenshot-dashboard.png — Desktop app showing the runner dashboard with multiple runners, their status, resource metrics, and active jobs]`

### 4. Auto-Restart & Self-Healing

Runners crash. It happens. HomeRun detects failures and automatically restarts runners with exponential backoff (up to 3 retries). For even more resilience, you can run runners as background services via macOS `launchd`:

```sh
homerun --no-tui add my-runner --repo owner/repo --mode service
# Survives reboots, managed by launchd
```

### 5. Smart Repo Discovery

Not sure which of your repos use self-hosted runners? HomeRun can scan for you:

```sh
# Scan local workspace for repos with `runs-on: self-hosted`
homerun --no-tui scan ~/workspace

# Or scan your GitHub account remotely
homerun --no-tui scan --remote
```

### 6. Custom Labels for Job Routing

Tag runners with labels and route specific jobs to specific machines:

```sh
homerun --no-tui add unit-runner --repo owner/repo --count 2 --labels self-hosted,unit
homerun --no-tui add e2e-runner  --repo owner/repo --count 2 --labels self-hosted,e2e
```

Then in your workflow:

```yaml
jobs:
  unit-tests:
    runs-on: [self-hosted, unit]
    steps:
      - uses: actions/checkout@v4
      - run: cargo test

  e2e-tests:
    runs-on: [self-hosted, e2e]
    steps:
      - uses: actions/checkout@v4
      - run: npm run test:e2e
```

---

## Architecture — Built for Reliability

HomeRun is written entirely in **Rust**, designed as a lightweight daemon that runs 24/7 without hogging resources.

```
┌──────────────┐   ┌─────────┐
│  Desktop App │   │   TUI   │     ← thin clients
└──────┬───────┘   └────┬────┘
       └────────┬────────┘
                │  Unix socket (REST + SSE + WebSocket)
       ┌────────┴────────┐
       │    homerund      │     ← daemon @ ~/.homerun/daemon.sock
       └────────┬────────┘
                │  spawns & monitors
      ┌─────────┼─────────┐
      │         │         │
   ┌──┴──┐  ┌──┴──┐  ┌──┴──┐
   │Run 1│  │Run 2│  │Run N│  ← native GitHub Actions runners
   └─────┘  └─────┘  └─────┘
```

**Why this matters:**

- **Unix socket** — No open ports, no network exposure. Communication stays local.
- **SSE for logs** — Real-time log streaming without polling overhead.
- **WebSocket for events** — Instant state updates pushed to all connected clients.
- **Native child processes** — Runners aren't containerized. Full access to host tools, hardware, and file system.
- **Secure token storage** — Credentials live in macOS Keychain, not in plaintext config files.

---

## Before & After

Here's what managing 4 runners for a repo looks like:

### Before (manual)

```sh
# Download
mkdir runner1 && cd runner1
curl -o actions-runner.tar.gz -L https://github.com/actions/runner/releases/download/v2.XXX/...
tar xzf actions-runner.tar.gz

# Register (copy token from GitHub UI)
./config.sh --url https://github.com/owner/repo --token AXXXXXXXXXXXX --name runner1

# Start
./run.sh &

# Repeat 3 more times for runner2, runner3, runner4...
# Then figure out monitoring, restarts, log access...
```

**~60 manual steps for 4 runners. No monitoring. No auto-restart.**

### After (HomeRun)

```sh
homerund
homerun --no-tui login
homerun --no-tui add ci-runner --repo owner/repo --count 4
```

**3 commands. Full monitoring. Auto-restart. Live logs. Job tracking.**

`[IMAGE: before-after.png — Side-by-side comparison: cluttered terminal with manual steps vs. clean HomeRun TUI with 4 healthy runners]`

---

## Who Is This For?

- **DevOps & Platform engineers** who manage runner fleets and want a unified dashboard instead of SSH sessions
- **Teams running private CI** who need self-hosted runners but don't want the operational overhead
- **Open source maintainers** who run tests on specific hardware (Apple Silicon, GPUs, etc.)
- **Cost-conscious teams** who want to reduce spending on GitHub's hosted runners
- **macOS developers** who need native CI capabilities with macOS Keychain integration and launchd support

---

## Getting Started

HomeRun is open source under the MIT license.

```sh
# Clone and build
git clone https://github.com/aGallea/homerun.git
cd homerun

# Start the daemon
cargo run --bin homerund

# Launch the TUI
cargo run --bin homerun

# Or use CLI mode
cargo run --bin homerun -- --no-tui login
cargo run --bin homerun -- --no-tui add my-runner --repo owner/repo
```

`[IMAGE: screenshot-getting-started.png — Terminal showing the quick start flow: daemon startup, login, and runner creation]`

Check out the [GitHub repo](https://github.com/aGallea/homerun) for full documentation, architecture details, and contribution guidelines.

---

## What's Next

HomeRun is actively developed, with regular releases and a growing feature set. Here's what's on the roadmap:

- **Linux support** — Extending beyond macOS to cover Linux-based runner hosts
- **Organization-level runners** — Manage runners at the GitHub org level, not just per-repo
- **Pre-built binaries & Homebrew** — `brew install homerun` is the goal
- **Runner groups & policies** — Advanced fleet management for larger teams

---

## Try It, Break It, Improve It

HomeRun started as a personal itch-scratch project and grew into something I use every day. If you manage self-hosted GitHub Actions runners — or you've been avoiding self-hosted because of the setup pain — give it a try.

**Star the repo** if you find it useful. **Open an issue** if something breaks. **Send a PR** if you want to help.

→ [**github.com/aGallea/homerun**](https://github.com/aGallea/homerun)

`[IMAGE: screenshot-star-repo.png — GitHub repo page showing the star button, or a call-to-action graphic]`

---

*HomeRun is open source (MIT license) and built with Rust, Tauri, React, and Ratatui. Contributions welcome.*
