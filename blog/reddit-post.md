# I built a macOS desktop app to manage GitHub Actions self-hosted runners — open source, written in Rust

**Subreddits:** r/devops, r/github, r/rust, r/selfhosted, r/homelab

---

Hey all,

I've been running self-hosted GitHub Actions runners for a while and the setup process drove me crazy. Every time: download the binary, copy a registration token from the GitHub UI, run `config.sh`, run `run.sh`, repeat N times, then figure out monitoring and restarts on your own.

So I built **HomeRun** — an open-source macOS app that handles all of that for you.

**What it does:**

- Desktop app (Tauri + React) with a guided wizard — pick a repo, set a count, click launch
- Daemon in the background manages the runner lifecycle (download, register, spawn, monitor, auto-restart)
- Real-time dashboard with live status, CPU/RAM metrics, job progress with step-by-step tracking
- GitHub Device Flow auth — no PATs, tokens stored in macOS Keychain
- Runner groups with batch operations (start/stop/restart/scale N runners at once)
- Smart repo discovery — scans your GitHub account or local workspace for repos using `runs-on: self-hosted`
- Menu bar tray icon with quick status overview
- Also has a TUI (Ratatui) and CLI mode if you prefer the terminal

**Tech stack:** Rust daemon (Axum over Unix socket), Tauri 2.0 desktop app, React + TypeScript frontend, WebSocket for real-time updates.

**Screenshots:** [see the repo README](https://github.com/aGallea/homerun) — dashboard, runner detail with job steps, repo scanner, menu bar, TUI, and the runner wizard.

**Install:**

```
brew tap aGallea/tap
brew install homerun

# Not code-signed yet, so clear the macOS quarantine flag:
xattr -cr /Applications/HomeRun.app
```

(macOS only for now — Linux/Windows support is on the roadmap: [#112](https://github.com/aGallea/homerun/issues/112))

**Roadmap** (depends on interest and time — it's a side project):

- Live log streaming ([#44](https://github.com/aGallea/homerun/issues/44))
- Docker runners ([#84](https://github.com/aGallea/homerun/issues/84))
- Kubernetes backend ([#89](https://github.com/aGallea/homerun/issues/89))
- Cross-platform — Linux & Windows ([#112](https://github.com/aGallea/homerun/issues/112))

Would love feedback, bug reports, or PRs. And if you find it useful, a star on the repo would really help with visibility — it's a solo side project so every bit of traction counts. MIT licensed.

**Repo:** <https://github.com/aGallea/homerun>
