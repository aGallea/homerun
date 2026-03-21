# Contributing to HomeRun

Thank you for your interest in contributing! This document explains how to get the project running locally, coding standards, and how to submit changes.

## Table of Contents

- [Dev Environment Setup](#dev-environment-setup)
- [Running the Components](#running-the-components)
- [Running Tests](#running-tests)
- [Code Style](#code-style)
- [PR Process](#pr-process)

---

## Dev Environment Setup

### Prerequisites

| Tool                     | Version        | Install                                                 |
| ------------------------ | -------------- | ------------------------------------------------------- |
| Rust                     | stable (1.75+) | `curl https://sh.rustup.rs -sSf \| sh`                  |
| Node.js                  | 20+            | [nodejs.org](https://nodejs.org) or `brew install node` |
| Xcode Command Line Tools | latest         | `xcode-select --install`                                |

### Clone and install

```sh
git clone https://github.com/aGallea/homerun.git
cd homerun

# Install Rust toolchain (reads rust-toolchain.toml if present)
source "$HOME/.cargo/env"
rustup update stable

# Install frontend dependencies
cd apps/desktop
npm install
cd ../..
```

---

## Running the Components

### Daemon (`homerund`)

```sh
source "$HOME/.cargo/env"
cargo run -p homerund
```

The daemon listens on a Unix socket at `~/.homerun/daemon.sock`. Logs are written to `~/.homerun/logs/`.

### TUI (`homerun`)

```sh
# Full TUI (requires daemon running)
cargo run -p homerun

# CLI mode (no daemon required for some commands)
cargo run -p homerun -- --no-tui status
cargo run -p homerun -- --no-tui list
```

### Tauri Desktop App

```sh
cd apps/desktop

# Development (hot-reload frontend + Rust backend)
npm run tauri dev

# Or run frontend only (no Tauri shell)
npm run dev
```

The Tauri dev build expects `homerund` to be running. Start the daemon in a separate terminal first.

### Running All Three Together

```sh
# Terminal 1 — daemon
cargo run -p homerund

# Terminal 2 — TUI
cargo run -p homerun

# Terminal 3 — Tauri app
cd apps/desktop && npm run tauri dev
```

---

## Running Tests

### Rust (daemon + TUI)

```sh
source "$HOME/.cargo/env"

# All tests
cargo test

# Specific crate
cargo test -p homerund
cargo test -p homerun

# With output
cargo test -- --nocapture
```

### TypeScript (frontend)

```sh
cd apps/desktop

# Type check only
npx tsc --noEmit

# Build check
npm run build
```

---

## Code Style

### Rust

```sh
source "$HOME/.cargo/env"

# Format
cargo fmt

# Lint (must pass with no warnings)
cargo clippy -- -D warnings

# Audit dependencies for vulnerabilities
cargo audit
```

CI enforces both `cargo fmt --check` and `cargo clippy -- -D warnings`. PRs that fail either check will not be merged.

### TypeScript / React

```sh
cd apps/desktop

# Type check
npx tsc --noEmit

# Build (catches bundler errors)
npm run build
```

We use ESLint and Prettier (configuration in `apps/desktop`). Format your code before committing:

```sh
cd apps/desktop
npx prettier --write src/
npx eslint src/ --fix
```

### Editor Config

An `.editorconfig` is provided at the repo root. Most editors pick this up automatically (VS Code: install the EditorConfig extension).

---

## PR Process

### Conventional Commits

All commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>

[optional body]
[optional footer]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`

**Examples:**

```
feat(daemon): add runner auto-restart with exponential backoff
fix(tui): correct keybinding for log view toggle
docs: add multi-runner example to examples/
chore(deps): bump axum to 0.8.1
```

Conventional commits power the auto-generated CHANGELOG and semantic versioning. Commitlint is enforced in CI.

### Submitting a PR

1. Fork the repository and create a branch from `main`:

   ```sh
   git checkout -b feat/my-feature
   ```

2. Make your changes, following the code style guidelines above.

3. Add or update tests as appropriate.

4. Ensure everything passes locally:

   ```sh
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   cd apps/desktop && npx tsc --noEmit && npm run build
   ```

5. Push and open a PR against `main`. Fill in the PR template.

6. A maintainer will review. Address feedback and the PR will be merged once approved.

### Good First Issues

Issues labeled [`good first issue`](https://github.com/aGallea/homerun/issues?q=label%3A%22good+first+issue%22) are a great place to start.

---

## Project Structure

```
homerun/
├── crates/
│   ├── daemon/          # homerund — background daemon (Rust + Axum)
│   └── tui/             # homerun — TUI/CLI client (Rust + Ratatui)
├── apps/
│   └── desktop/         # Tauri desktop app (React + TypeScript)
├── examples/            # Setup walkthroughs and workflow templates
├── docs/                # Architecture and API documentation
└── .github/             # CI workflows and issue/PR templates
```
