# Changelog

All notable changes to HomeRun will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is auto-generated from [Conventional Commits](https://www.conventionalcommits.org/).

---

## [0.1.0] — 2026-03-21

Initial release of HomeRun.

### Added

#### Daemon (`homerund`)

- Rust + Axum daemon exposing REST/SSE/WebSocket API over Unix socket at `~/.homerun/daemon.sock`
- GitHub OAuth flow with temporary localhost callback listener
- Personal Access Token (PAT) authentication as fallback
- Token storage in macOS Keychain via `security-framework`
- Runner lifecycle management: create, register, start, stop, restart, delete
- App-managed runner mode (daemon child process)
- Background service runner mode (macOS launchd plist)
- Runner state machine: Creating → Registering → Online ⇄ Busy → Offline → Deleting
- Auto-restart on crash: up to 3 attempts with 10s backoff
- GitHub runner binary download and caching at `~/.homerun/cache/`
- Per-runner isolated working directories at `~/.homerun/runners/<name>/`
- Real-time log streaming via Server-Sent Events (SSE)
- CPU/RAM/disk metrics collection via `sysinfo`
- In-memory metrics ring buffer (last 24h per runner)
- WebSocket `/events` endpoint for real-time status updates
- Config stored at `~/.homerun/config.toml`
- Structured logging to `~/.homerun/logs/`

#### TUI / CLI (`homerun`)

- Ratatui-based terminal UI with split-pane layout (runner list + detail)
- Tab bar: Runners, Repos, Workflows, Monitoring
- Full keyboard navigation: `↑↓`, `Enter`, `a`, `d`, `s`, `r`, `l`, `e`, `1-4`, `q`, `?`
- Live log view per runner
- Plain CLI mode via `homerun --no-tui <command>`
- CLI commands: `list`, `add`, `remove`, `status`, `scan`, `login`
- `homerun scan <path>` — local workspace scan for `runs-on: self-hosted`
- `homerun scan --remote` — GitHub API scan
- Clap-based argument parsing

#### Desktop App (Tauri)

- Tauri 2.0 desktop app for macOS (ARM64 + Intel)
- React + TypeScript frontend
- Dashboard with runner stats cards and runners table
- Repositories view with runner counts and quick-add
- Runners view with filtering and bulk actions
- Monitoring view with CPU/RAM/disk graphs
- Workflow Runs view with status across all repos
- New Runner wizard: pick repo → configure → launch
- Runner detail view: live logs, resource graphs, controls
- Smart repo discovery: local workspace scan + GitHub API scan
- Actions menu: start, stop, restart, delete (with confirmation)
- React Router v7 for navigation

#### Infrastructure

- Rust workspace with `resolver = "2"`
- Shared workspace dependencies and version management
- MIT license

[0.1.0]: https://github.com/aGallea/homerun/releases/tag/v0.1.0
