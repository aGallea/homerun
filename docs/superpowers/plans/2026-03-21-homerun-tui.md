# HomeRun TUI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `homerun` TUI binary — a Ratatui-based terminal UI that connects to the running daemon over its Unix socket and provides a keyboard-driven interface for managing GitHub Actions self-hosted runners.

**Architecture:** A new `tui` crate in the workspace. The binary connects to the daemon at `~/.homerun/daemon.sock` via HTTP (using `hyper` + `hyper-util` over Unix socket) and WebSocket. The TUI uses Ratatui with crossterm for rendering, driven by an async event loop that merges terminal input, tick events, and daemon WebSocket messages.

**Deferred to later plans:**
- OAuth flow (`homerun login` browser-based) — Plan 5
- Workflows tab — Plan 5 (requires `GET /repos/:id/workflows` endpoint)
- Add-runner interactive wizard — Plan 5

**Tech Stack:** Rust, Tokio, Ratatui, crossterm, hyper + hyper-util (Unix socket HTTP client), tokio-tungstenite (WebSocket client), serde/serde_json, clap (CLI args)

**Spec:** `docs/superpowers/specs/2026-03-21-self-runner-design.md` (TUI section)

**Implementation notes:**
- The daemon must be running for the TUI to work. If the socket is missing, show a clear error: "Daemon not running. Start it with: homerund"
- For HTTP over Unix socket, use `hyper-util` with a custom Unix connector — `reqwest` does not support Unix sockets natively. Alternatively, check if `hyperlocal` crate is compatible with hyper 1.x; if not, write a minimal connector.
- For WebSocket over Unix socket, `tokio-tungstenite` supports custom connectors via `client_async` on any `AsyncRead + AsyncWrite` stream (i.e., `tokio::net::UnixStream`).
- Ratatui 0.29+ API — verify widget constructors at implementation time.
- The TUI is a thin client. All state lives in the daemon. The TUI polls or subscribes and renders what the daemon reports.

---

## File Structure

```
homerun/
├── Cargo.toml                          # Workspace root (add tui crate)
├── crates/
│   ├── daemon/                         # Existing daemon crate
│   └── tui/
│       ├── Cargo.toml                  # TUI crate dependencies
│       └── src/
│           ├── main.rs                 # Entry point: parse args, launch TUI or CLI mode
│           ├── lib.rs                  # Re-exports for testing
│           ├── client.rs              # DaemonClient: HTTP + WebSocket over Unix socket
│           ├── app.rs                 # App state struct, update logic
│           ├── event.rs               # Event loop: crossterm + tick + WebSocket merge
│           ├── ui/
│           │   ├── mod.rs             # Top-level draw function, layout
│           │   ├── tabs.rs            # Tab bar widget
│           │   ├── runners.rs         # Runners tab: list + detail split pane
│           │   ├── repos.rs           # Repos tab: repo list
│           │   ├── monitoring.rs      # Monitoring tab: system metrics
│           │   └── status_bar.rs      # Bottom status bar with keybindings
│           └── cli.rs                 # Plain CLI mode (--no-tui)
```

---

### Task 1: TUI Crate Scaffold

**Files:**
- Modify: `Cargo.toml` (workspace root — add `crates/tui` to members)
- Create: `crates/tui/Cargo.toml`
- Create: `crates/tui/src/main.rs`
- Create: `crates/tui/src/lib.rs`

- [ ] **Step 1: Add `crates/tui` to workspace members**

In the root `Cargo.toml`, add `"crates/tui"` to the members list:

```toml
[workspace]
resolver = "2"
members = ["crates/daemon", "crates/tui"]
```

Also add shared dependencies that both crates will use:

```toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: Create TUI crate Cargo.toml**

```toml
[package]
name = "homerun"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "homerun"
path = "src/main.rs"

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
chrono.workspace = true
ratatui = "0.29"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
hyper = { version = "1", features = ["client", "http1"] }
hyper-util = { version = "0.1", features = ["tokio", "client-legacy"] }
http-body-util = "0.1"
tower = "0.5"
tokio-tungstenite = "0.26"
futures = "0.3"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Create minimal main.rs**

```rust
use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "homerun", about = "HomeRun — GitHub Actions self-hosted runner manager")]
struct Cli {
    /// Disable TUI, use plain CLI output
    #[arg(long)]
    no_tui: bool,

    /// Subcommand for CLI mode
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// List all runners
    List,
    /// Show runner and system status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        println!("CLI mode — not yet implemented");
    } else {
        println!("TUI mode — not yet implemented");
    }

    Ok(())
}
```

- [ ] **Step 4: Create lib.rs**

```rust
pub mod client;
pub mod app;
pub mod event;
pub mod ui;
pub mod cli;
```

- [ ] **Step 5: Create empty module stubs so it compiles**

Create the following files with minimal content so `cargo check` passes:

`crates/tui/src/client.rs`:
```rust
// DaemonClient — HTTP + WebSocket over Unix socket
```

`crates/tui/src/app.rs`:
```rust
// App state and update logic
```

`crates/tui/src/event.rs`:
```rust
// Event loop: crossterm + tick + WebSocket
```

`crates/tui/src/ui/mod.rs`:
```rust
pub mod tabs;
pub mod runners;
pub mod repos;
pub mod monitoring;
pub mod status_bar;
```

Create empty files for each UI submodule (`tabs.rs`, `runners.rs`, `repos.rs`, `monitoring.rs`, `status_bar.rs`) as blank files.

`crates/tui/src/cli.rs`:
```rust
// Plain CLI mode (--no-tui)
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p homerun`
Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/tui/ Cargo.toml
git commit -m "chore: scaffold TUI crate with clap CLI skeleton"
```

---

### Task 2: DaemonClient (HTTP over Unix Socket)

**Files:**
- Create: `crates/tui/src/client.rs`
- Test: inline `#[cfg(test)]` module (tests use the daemon's router directly via `tower::ServiceExt` to avoid needing a real socket)

- [ ] **Step 1: Write failing test for DaemonClient**

In `crates/tui/src/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_runners_response() {
        let json = r#"[
            {
                "config": {
                    "id": "abc-123",
                    "name": "gifted-runner-1",
                    "repo_owner": "aGallea",
                    "repo_name": "gifted",
                    "labels": ["self-hosted", "macOS"],
                    "mode": "app",
                    "work_dir": "/tmp/runners/abc-123"
                },
                "state": "online",
                "pid": null,
                "uptime_secs": null,
                "jobs_completed": 0,
                "jobs_failed": 0
            }
        ]"#;
        let runners: Vec<RunnerInfo> = serde_json::from_str(json).unwrap();
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].config.name, "gifted-runner-1");
        assert_eq!(runners[0].state, "online");
    }

    #[tokio::test]
    async fn test_parse_auth_status() {
        let json = r#"{"authenticated": false, "user": null}"#;
        let status: AuthStatus = serde_json::from_str(json).unwrap();
        assert!(!status.authenticated);
        assert!(status.user.is_none());
    }

    #[tokio::test]
    async fn test_parse_metrics_response() {
        let json = r#"{
            "system": {
                "cpu_percent": 12.5,
                "memory_used_bytes": 8000000000,
                "memory_total_bytes": 16000000000,
                "disk_used_bytes": 100000000000,
                "disk_total_bytes": 500000000000
            },
            "runners": []
        }"#;
        let metrics: MetricsResponse = serde_json::from_str(json).unwrap();
        assert!((metrics.system.cpu_percent - 12.5).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerun client::tests`
Expected: FAIL — types not defined

- [ ] **Step 3: Implement client types and DaemonClient**

```rust
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use hyper::body::Incoming;
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use http_body_util::BodyExt;

// --- Response types (mirror daemon's API responses) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub id: String,
    pub name: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub labels: Vec<String>,
    pub mode: String,
    pub work_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub config: RunnerConfig,
    pub state: String,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub jobs_completed: u32,
    pub jobs_failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user: Option<GitHubUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerMetrics {
    pub runner_id: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub system: SystemMetrics,
    pub runners: Vec<RunnerMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub id: u64,
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub private: bool,
    pub html_url: String,
    pub is_org: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunnerRequest {
    pub repo_full_name: String,
    pub name: Option<String>,
    pub labels: Option<Vec<String>>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerEvent {
    pub runner_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

// --- Unix socket HTTP connector ---

/// A tower connector that dials a Unix socket instead of TCP.
#[derive(Clone)]
struct UnixConnector {
    socket_path: PathBuf,
}

impl tower::Service<hyper::Uri> for UnixConnector {
    type Response = hyper_util::rt::TokioIo<tokio::net::UnixStream>;
    type Error = std::io::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: hyper::Uri) -> Self::Future {
        let path = self.socket_path.clone();
        Box::pin(async move {
            let stream = tokio::net::UnixStream::connect(path).await?;
            Ok(hyper_util::rt::TokioIo::new(stream))
        })
    }
}

// --- DaemonClient ---

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn default_socket() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        Self::new(home.join(".homerun/daemon.sock"))
    }

    /// Check if the daemon socket exists.
    pub fn socket_exists(&self) -> bool {
        self.socket_path.exists()
    }

    /// Return the socket path (for WebSocket connections).
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn request(&self, method: &str, path: &str, body: Option<String>) -> Result<String> {
        let connector = UnixConnector {
            socket_path: self.socket_path.clone(),
        };
        let client: Client<UnixConnector, String> =
            Client::builder(TokioExecutor::new()).build(connector);

        // hyper requires a valid URI — the host is ignored for Unix sockets.
        let uri = format!("http://localhost{path}");
        let mut builder = Request::builder().method(method).uri(&uri);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        let req = builder.body(body.unwrap_or_default())?;

        let response = client
            .request(req)
            .await
            .context("Failed to connect to daemon — is homerund running?")?;

        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .context("Failed to read response body")?
            .to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();

        if !status.is_success() && status.as_u16() != 204 {
            bail!("Daemon returned {status}: {text}");
        }
        Ok(text)
    }

    // --- API methods ---

    pub async fn health(&self) -> Result<()> {
        self.request("GET", "/health", None).await?;
        Ok(())
    }

    pub async fn auth_status(&self) -> Result<AuthStatus> {
        let body = self.request("GET", "/auth/status", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn list_runners(&self) -> Result<Vec<RunnerInfo>> {
        let body = self.request("GET", "/runners", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_runner(&self, id: &str) -> Result<RunnerInfo> {
        let body = self.request("GET", &format!("/runners/{id}"), None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn create_runner(&self, req: &CreateRunnerRequest) -> Result<RunnerInfo> {
        let body = self
            .request("POST", "/runners", Some(serde_json::to_string(req)?))
            .await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn delete_runner(&self, id: &str) -> Result<()> {
        self.request("DELETE", &format!("/runners/{id}"), None)
            .await?;
        Ok(())
    }

    pub async fn start_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/start"), None)
            .await?;
        Ok(())
    }

    pub async fn stop_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/stop"), None)
            .await?;
        Ok(())
    }

    pub async fn restart_runner(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/runners/{id}/restart"), None)
            .await?;
        Ok(())
    }

    pub async fn list_repos(&self) -> Result<Vec<RepoInfo>> {
        let body = self.request("GET", "/repos", None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_metrics(&self) -> Result<MetricsResponse> {
        let body = self.request("GET", "/metrics", None).await?;
        Ok(serde_json::from_str(&body)?)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p homerun client::tests`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/client.rs
git commit -m "feat(tui): add DaemonClient with Unix socket HTTP transport"
```

---

### Task 3: App Struct + Event Loop Skeleton

**Files:**
- Create: `crates/tui/src/app.rs`
- Create: `crates/tui/src/event.rs`

- [ ] **Step 1: Write failing test for App**

In `crates/tui/src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_default_state() {
        let app = App::new();
        assert_eq!(app.active_tab, Tab::Runners);
        assert_eq!(app.selected_runner_index, 0);
        assert!(!app.should_quit);
        assert!(app.runners.is_empty());
    }

    #[test]
    fn test_tab_cycling() {
        let mut app = App::new();
        assert_eq!(app.active_tab, Tab::Runners);
        app.active_tab = Tab::Repos;
        assert_eq!(app.active_tab, Tab::Repos);
        app.active_tab = Tab::Monitoring;
        assert_eq!(app.active_tab, Tab::Monitoring);
    }

    #[test]
    fn test_runner_selection_bounds() {
        let mut app = App::new();
        // With no runners, selection stays at 0
        app.select_next_runner();
        assert_eq!(app.selected_runner_index, 0);
        app.select_prev_runner();
        assert_eq!(app.selected_runner_index, 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerun app::tests`
Expected: FAIL — `App` not defined

- [ ] **Step 3: Implement App struct**

```rust
use crate::client::{AuthStatus, MetricsResponse, RunnerInfo, RepoInfo, SystemMetrics};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Runners,
    Repos,
    Monitoring,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Runners, Tab::Repos, Tab::Monitoring]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Runners => "Runners",
            Tab::Repos => "Repos",
            Tab::Monitoring => "Monitoring",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Runners => 0,
            Tab::Repos => 1,
            Tab::Monitoring => 2,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        match i {
            0 => Some(Tab::Runners),
            1 => Some(Tab::Repos),
            2 => Some(Tab::Monitoring),
            _ => None,
        }
    }
}

pub struct App {
    pub active_tab: Tab,
    pub should_quit: bool,
    pub runners: Vec<RunnerInfo>,
    pub selected_runner_index: usize,
    pub repos: Vec<RepoInfo>,
    pub selected_repo_index: usize,
    pub auth_status: Option<AuthStatus>,
    pub metrics: Option<MetricsResponse>,
    pub show_help: bool,
    pub status_message: Option<String>,
    pub daemon_connected: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Runners,
            should_quit: false,
            runners: Vec::new(),
            selected_runner_index: 0,
            repos: Vec::new(),
            selected_repo_index: 0,
            auth_status: None,
            metrics: None,
            show_help: false,
            status_message: None,
            daemon_connected: false,
        }
    }

    pub fn select_next_runner(&mut self) {
        if !self.runners.is_empty() {
            self.selected_runner_index =
                (self.selected_runner_index + 1).min(self.runners.len() - 1);
        }
    }

    pub fn select_prev_runner(&mut self) {
        self.selected_runner_index = self.selected_runner_index.saturating_sub(1);
    }

    pub fn selected_runner(&self) -> Option<&RunnerInfo> {
        self.runners.get(self.selected_runner_index)
    }

    pub fn select_next_repo(&mut self) {
        if !self.repos.is_empty() {
            self.selected_repo_index =
                (self.selected_repo_index + 1).min(self.repos.len() - 1);
        }
    }

    pub fn select_prev_repo(&mut self) {
        self.selected_repo_index = self.selected_repo_index.saturating_sub(1);
    }
}
```

- [ ] **Step 4: Implement event loop in event.rs**

```rust
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use tokio::sync::mpsc;

/// Events the TUI reacts to.
pub enum AppEvent {
    /// A key press from the terminal.
    Key(KeyEvent),
    /// Periodic tick for polling daemon state.
    Tick,
    /// A runner event from the daemon WebSocket.
    DaemonEvent(String),
}

/// Spawns a background task that sends AppEvents into a channel.
pub fn start_event_loop(tick_rate: Duration) -> Result<mpsc::UnboundedReceiver<AppEvent>> {
    let (tx, rx) = mpsc::unbounded_channel();

    let key_tx = tx.clone();
    // Crossterm event polling (blocking, so run in a blocking task)
    tokio::task::spawn_blocking(move || loop {
        if event::poll(tick_rate).unwrap_or(false) {
            if let Ok(CrosstermEvent::Key(key)) = event::read() {
                if key_tx.send(AppEvent::Key(key)).is_err() {
                    break;
                }
            }
        }
    });

    let tick_tx = tx.clone();
    // Tick timer
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    Ok(rx)
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p homerun app::tests`
Expected: 3 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/event.rs
git commit -m "feat(tui): add App state struct and event loop skeleton"
```

---

### Task 4: Runners Tab (List + Detail Split Pane)

**Files:**
- Create: `crates/tui/src/ui/mod.rs`
- Create: `crates/tui/src/ui/tabs.rs`
- Create: `crates/tui/src/ui/runners.rs`
- Create: `crates/tui/src/ui/status_bar.rs`

- [ ] **Step 1: Implement the top-level draw function**

In `crates/tui/src/ui/mod.rs`:

```rust
pub mod tabs;
pub mod runners;
pub mod repos;
pub mod monitoring;
pub mod status_bar;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Tab bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    tabs::draw_tabs(f, app, chunks[0]);

    match app.active_tab {
        crate::app::Tab::Runners => runners::draw_runners(f, app, chunks[1]),
        crate::app::Tab::Repos => repos::draw_repos(f, app, chunks[1]),
        crate::app::Tab::Monitoring => monitoring::draw_monitoring(f, app, chunks[1]),
    }

    status_bar::draw_status_bar(f, app, chunks[2]);

    if app.show_help {
        draw_help_popup(f);
    }
}

fn draw_help_popup(f: &mut Frame) {
    use ratatui::layout::Rect;
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    use ratatui::style::{Color, Style};

    let area = f.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let help_text = "\
  Keybindings

  Up/Down    Navigate list
  1-3        Switch tabs
  s          Start/Stop runner
  r          Restart runner
  d          Delete runner (confirm)
  l          View logs
  e          Edit labels
  a          Add runner
  ?          Toggle this help
  q          Quit";

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(Style::default().fg(Color::White)),
        popup_area,
    );
}
```

- [ ] **Step 2: Implement tab bar**

In `crates/tui/src/ui/tabs.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Tabs};

use crate::app::{App, Tab};

pub fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            Line::from(Span::styled(
                format!(" [{}] {} ", i + 1, tab.title()),
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" HomeRun "))
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}
```

- [ ] **Step 3: Implement runners tab with split pane**

In `crates/tui/src/ui/runners.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::App;
use crate::client::RunnerInfo;

pub fn draw_runners(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_runner_list(f, app, chunks[0]);
    draw_runner_detail(f, app, chunks[1]);
}

fn state_color(state: &str) -> Color {
    match state {
        "online" => Color::Green,
        "busy" => Color::Yellow,
        "offline" => Color::Gray,
        "error" => Color::Red,
        "creating" | "registering" => Color::Cyan,
        "stopping" | "deleting" => Color::Magenta,
        _ => Color::White,
    }
}

fn draw_runner_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .runners
        .iter()
        .map(|r| {
            let status_color = state_color(&r.state);
            let line = Line::from(vec![
                Span::styled("● ", Style::default().fg(status_color)),
                Span::raw(&r.config.name),
                Span::styled(
                    format!(" ({})", r.state),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Runners ({}) ", app.runners.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !app.runners.is_empty() {
        list_state.select(Some(app.selected_runner_index));
    }

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_runner_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.selected_runner() {
        Some(runner) => format_runner_detail(runner, app),
        None => " No runner selected.\n\n Press 'a' to add a new runner.".to_string(),
    };

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(" Detail "));

    f.render_widget(paragraph, area);
}

fn format_runner_detail(runner: &RunnerInfo, app: &App) -> String {
    let repo = format!("{}/{}", runner.config.repo_owner, runner.config.repo_name);
    let labels = runner.config.labels.join(", ");
    let uptime = runner
        .uptime_secs
        .map(|s| format_duration(s))
        .unwrap_or_else(|| "—".to_string());

    let mut lines = format!(
        " Name:    {}\n\
         \n\
         \ State:   {}\n\
         \ Repo:    {}\n\
         \ Mode:    {}\n\
         \ Labels:  {}\n\
         \ Uptime:  {}\n\
         \ Jobs:    {} completed, {} failed\n",
        runner.config.name,
        runner.state,
        repo,
        runner.config.mode,
        labels,
        uptime,
        runner.jobs_completed,
        runner.jobs_failed,
    );

    // Show per-runner metrics if available
    if let Some(ref metrics) = app.metrics {
        if let Some(rm) = metrics.runners.iter().find(|m| m.runner_id == runner.config.id) {
            lines.push_str(&format!(
                "\n CPU:     {:.1}%\n Memory:  {}\n",
                rm.cpu_percent,
                format_bytes(rm.memory_bytes),
            ));
        }
    }

    lines.push_str("\n [s] start/stop  [r] restart  [d] delete  [e] edit  [l] logs");
    lines
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(90), "1m");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_state_color_mapping() {
        assert_eq!(state_color("online"), Color::Green);
        assert_eq!(state_color("error"), Color::Red);
        assert_eq!(state_color("busy"), Color::Yellow);
    }
}
```

- [ ] **Step 4: Implement status bar**

In `crates/tui/src/ui/status_bar.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let online = app.runners.iter().filter(|r| r.state == "online").count();
    let busy = app.runners.iter().filter(|r| r.state == "busy").count();
    let total = app.runners.len();

    let status = if let Some(ref msg) = app.status_message {
        Span::styled(msg.as_str(), Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };

    let connection = if app.daemon_connected {
        Span::styled("Connected", Style::default().fg(Color::Green))
    } else {
        Span::styled("Disconnected", Style::default().fg(Color::Red))
    };

    let line = Line::from(vec![
        Span::raw(" "),
        connection,
        Span::raw("  |  "),
        Span::styled(
            format!("{total} runners ({online} online, {busy} busy)"),
            Style::default().fg(Color::White),
        ),
        Span::raw("  |  "),
        status,
        Span::raw("  "),
        Span::styled(
            "q:quit  ?:help",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p homerun`
Expected: All tests pass (app, client, runners UI helpers)

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/ui/
git commit -m "feat(tui): add Runners tab with split pane list+detail UI"
```

---

### Task 5: Keyboard Handling (Navigation + Actions)

**Files:**
- Modify: `crates/tui/src/main.rs`
- Create keyboard handler logic (integrated into main loop)

- [ ] **Step 1: Write failing test for key handling**

In `crates/tui/src/app.rs`, add to the test module:

```rust
    #[test]
    fn test_handle_key_quit() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_key_tab_switch() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('2'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Repos);
        app.handle_key(KeyCode::Char('3'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Monitoring);
        app.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Runners);
    }

    #[test]
    fn test_handle_key_help_toggle() {
        let mut app = App::new();
        assert!(!app.show_help);
        app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(app.show_help);
        app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut app = App::new();
        // Populate fake runners
        app.runners = vec![
            make_test_runner("r1", "online"),
            make_test_runner("r2", "busy"),
            make_test_runner("r3", "offline"),
        ];
        assert_eq!(app.selected_runner_index, 0);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 1);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 2);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 2); // stays at end
        app.handle_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 1);
    }
```

Add a test helper:

```rust
    fn make_test_runner(id: &str, state: &str) -> crate::client::RunnerInfo {
        crate::client::RunnerInfo {
            config: crate::client::RunnerConfig {
                id: id.to_string(),
                name: format!("runner-{id}"),
                repo_owner: "test".to_string(),
                repo_name: "repo".to_string(),
                labels: vec!["self-hosted".to_string()],
                mode: "app".to_string(),
                work_dir: std::path::PathBuf::from("/tmp"),
            },
            state: state.to_string(),
            pid: None,
            uptime_secs: None,
            jobs_completed: 0,
            jobs_failed: 0,
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p homerun app::tests`
Expected: FAIL — `handle_key` not defined

- [ ] **Step 3: Implement handle_key on App**

Add to `crates/tui/src/app.rs`:

```rust
use crossterm::event::{KeyCode, KeyModifiers};

/// Actions that require async daemon calls — returned from handle_key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    StartRunner(String),
    StopRunner(String),
    RestartRunner(String),
    DeleteRunner(String),
    RefreshRunners,
    RefreshRepos,
    RefreshMetrics,
}

impl App {
    /// Handle a key event. Returns an optional Action requiring a daemon call.
    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Option<Action> {
        // Help overlay captures all keys except ? and Esc
        if self.show_help {
            match code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return None;
        }

        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                None
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                None
            }
            KeyCode::Char('1') => {
                self.active_tab = Tab::Runners;
                None
            }
            KeyCode::Char('2') => {
                self.active_tab = Tab::Repos;
                None
            }
            KeyCode::Char('3') => {
                self.active_tab = Tab::Monitoring;
                None
            }
            KeyCode::Down => {
                match self.active_tab {
                    Tab::Runners => self.select_next_runner(),
                    Tab::Repos => self.select_next_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Up => {
                match self.active_tab {
                    Tab::Runners => self.select_prev_runner(),
                    Tab::Repos => self.select_prev_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Char('s') => {
                if let Some(runner) = self.selected_runner() {
                    let id = runner.config.id.clone();
                    let action = if runner.state == "online" || runner.state == "busy" {
                        Action::StopRunner(id)
                    } else {
                        Action::StartRunner(id)
                    };
                    return Some(action);
                }
                None
            }
            KeyCode::Char('r') => {
                if let Some(runner) = self.selected_runner() {
                    return Some(Action::RestartRunner(runner.config.id.clone()));
                }
                None
            }
            KeyCode::Char('d') => {
                if let Some(runner) = self.selected_runner() {
                    return Some(Action::DeleteRunner(runner.config.id.clone()));
                }
                None
            }
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerun app::tests`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): add keyboard handling with actions for runner management"
```

---

### Task 6: Main Loop + Real-Time Updates (WebSocket)

**Files:**
- Modify: `crates/tui/src/main.rs`
- Modify: `crates/tui/src/event.rs`
- Modify: `crates/tui/src/client.rs`

- [ ] **Step 1: Add WebSocket connection to DaemonClient**

In `crates/tui/src/client.rs`, add:

```rust
use tokio_tungstenite::tungstenite::Message as WsMessage;
use futures::stream::SplitStream;
use futures::StreamExt;
use tokio_tungstenite::WebSocketStream;

impl DaemonClient {
    /// Connect to the daemon's WebSocket endpoint for real-time events.
    /// Returns a stream of RunnerEvent messages.
    pub async fn connect_events(
        &self,
    ) -> Result<SplitStream<WebSocketStream<tokio::net::UnixStream>>> {
        let stream = tokio::net::UnixStream::connect(&self.socket_path).await?;

        let uri = "ws://localhost/events";
        let (ws_stream, _response) =
            tokio_tungstenite::client_async(uri, stream).await?;

        let (_write, read) = ws_stream.split();
        Ok(read)
    }
}
```

- [ ] **Step 2: Integrate WebSocket into event loop**

Update `crates/tui/src/event.rs` to accept an optional WebSocket stream:

```rust
use futures::StreamExt;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Start WebSocket event forwarding (optional — if daemon is reachable).
pub fn start_ws_forwarding(
    tx: mpsc::UnboundedSender<AppEvent>,
    mut ws_read: futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<tokio::net::UnixStream>,
    >,
) {
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_read.next().await {
            if let WsMessage::Text(text) = msg {
                if tx.send(AppEvent::DaemonEvent(text.to_string())).is_err() {
                    break;
                }
            }
        }
    });
}
```

- [ ] **Step 3: Wire up the main loop in main.rs**

```rust
use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::KeyEventKind,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use homerun::app::{Action, App};
use homerun::client::DaemonClient;
use homerun::event::{start_event_loop, start_ws_forwarding, AppEvent};
use homerun::ui;

#[derive(Parser)]
#[command(name = "homerun", about = "HomeRun — GitHub Actions self-hosted runner manager")]
struct Cli {
    #[arg(long)]
    no_tui: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    List,
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_tui || cli.command.is_some() {
        return homerun::cli::run(cli.command).await;
    }

    run_tui().await
}

async fn run_tui() -> Result<()> {
    let client = DaemonClient::default_socket();
    let mut app = App::new();

    // Check daemon connectivity
    match client.health().await {
        Ok(_) => app.daemon_connected = true,
        Err(_) => {
            eprintln!(
                "Cannot connect to HomeRun daemon.\n\
                 Make sure homerund is running:\n\n  \
                 homerund\n"
            );
            std::process::exit(1);
        }
    }

    // Initial data load
    if let Ok(runners) = client.list_runners().await {
        app.runners = runners;
    }
    if let Ok(auth) = client.auth_status().await {
        app.auth_status = Some(auth);
    }
    if let Ok(metrics) = client.get_metrics().await {
        app.metrics = Some(metrics);
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start event loop
    let tick_rate = Duration::from_millis(1000);
    let mut events = start_event_loop(tick_rate)?;

    // Try to connect WebSocket for real-time updates
    if let Ok(ws_read) = client.connect_events().await {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        // We need access to the event sender — refactor: pass tx from start_event_loop
        // For now, the tick-based polling will handle updates.
        let _ = ws_read; // TODO: wire into event sender in next step
    }

    // Main loop
    let mut poll_counter = 0u32;
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Some(event) = events.recv().await {
            match event {
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if let Some(action) = app.handle_key(key.code, key.modifiers) {
                        handle_action(&client, &mut app, action).await;
                    }
                }
                AppEvent::Tick => {
                    poll_counter += 1;
                    // Refresh runners every tick, metrics every 5 ticks
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                    }
                    if poll_counter % 5 == 0 {
                        if let Ok(metrics) = client.get_metrics().await {
                            app.metrics = Some(metrics);
                        }
                    }
                }
                AppEvent::DaemonEvent(_json) => {
                    // Real-time event received — refresh runner list
                    if let Ok(runners) = client.list_runners().await {
                        app.runners = runners;
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_action(client: &DaemonClient, app: &mut App, action: Action) {
    let result = match &action {
        Action::StartRunner(id) => client.start_runner(id).await,
        Action::StopRunner(id) => client.stop_runner(id).await,
        Action::RestartRunner(id) => client.restart_runner(id).await,
        Action::DeleteRunner(id) => client.delete_runner(id).await,
        Action::RefreshRunners | Action::RefreshRepos | Action::RefreshMetrics => Ok(()),
    };

    match result {
        Ok(_) => {
            app.status_message = Some(format!("{:?} succeeded", action));
            // Refresh runners after any action
            if let Ok(runners) = client.list_runners().await {
                app.runners = runners;
                // Clamp selection index
                if app.selected_runner_index >= app.runners.len() && !app.runners.is_empty() {
                    app.selected_runner_index = app.runners.len() - 1;
                }
            }
        }
        Err(e) => {
            app.status_message = Some(format!("Error: {e}"));
        }
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p homerun`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/main.rs crates/tui/src/event.rs crates/tui/src/client.rs
git commit -m "feat(tui): wire up main TUI loop with polling and WebSocket skeleton"
```

---

### Task 7: Repos Tab

**Files:**
- Create: `crates/tui/src/ui/repos.rs`
- Modify: `crates/tui/src/main.rs` (fetch repos on tab switch)

- [ ] **Step 1: Implement repos tab UI**

In `crates/tui/src/ui/repos.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::app::App;

pub fn draw_repos(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_repo_list(f, app, chunks[0]);
    draw_repo_detail(f, app, chunks[1]);
}

fn draw_repo_list(f: &mut Frame, app: &App, area: Rect) {
    if !app.auth_status.as_ref().is_some_and(|a| a.authenticated) {
        let msg = Paragraph::new(" Not authenticated.\n\n Run: homerun login --token <PAT>")
            .block(Block::default().borders(Borders::ALL).title(" Repos "));
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|r| {
            let visibility = if r.private { "private" } else { "public" };
            let org_marker = if r.is_org { " [org]" } else { "" };
            let line = Line::from(vec![
                Span::raw(&r.full_name),
                Span::styled(
                    format!(" ({visibility}{org_marker})"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Repos ({}) ", app.repos.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !app.repos.is_empty() {
        list_state.select(Some(app.selected_repo_index));
    }

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_repo_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.repos.get(app.selected_repo_index) {
        Some(repo) => {
            let runner_count = app
                .runners
                .iter()
                .filter(|r| {
                    r.config.repo_owner == repo.owner && r.config.repo_name == repo.name
                })
                .count();
            format!(
                " Repository: {}\n\
                 \n\
                 \ Owner:      {}\n\
                 \ Visibility: {}\n\
                 \ URL:        {}\n\
                 \ Runners:    {}\n\
                 \n\
                 \ Press 'a' to add a runner for this repo.",
                repo.full_name,
                repo.owner,
                if repo.private { "Private" } else { "Public" },
                repo.html_url,
                runner_count,
            )
        }
        None => " No repos loaded.\n\n Authenticate first, then repos will appear.".to_string(),
    };

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(" Detail "));

    f.render_widget(paragraph, area);
}
```

- [ ] **Step 2: Load repos on tab switch**

In `crates/tui/src/app.rs`, update the `handle_key` match for tab `'2'`:

```rust
            KeyCode::Char('2') => {
                self.active_tab = Tab::Repos;
                if self.repos.is_empty() {
                    Some(Action::RefreshRepos)
                } else {
                    None
                }
            }
```

In `main.rs` `handle_action`, add handling for `RefreshRepos`:

```rust
        Action::RefreshRepos => {
            if let Ok(repos) = client.list_repos().await {
                app.repos = repos;
            }
            Ok(())
        }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p homerun`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/ui/repos.rs crates/tui/src/app.rs crates/tui/src/main.rs
git commit -m "feat(tui): add Repos tab with list and detail pane"
```

---

### Task 8: Monitoring Tab (System Metrics Display)

**Files:**
- Create: `crates/tui/src/ui/monitoring.rs`

- [ ] **Step 1: Implement monitoring tab**

In `crates/tui/src/ui/monitoring.rs`:

```rust
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::App;

pub fn draw_monitoring(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // CPU gauge
            Constraint::Length(5),  // Memory gauge
            Constraint::Length(5),  // Disk gauge
            Constraint::Min(0),    // Per-runner table
        ])
        .split(area);

    match &app.metrics {
        Some(metrics) => {
            draw_cpu_gauge(f, &metrics.system, chunks[0]);
            draw_memory_gauge(f, &metrics.system, chunks[1]);
            draw_disk_gauge(f, &metrics.system, chunks[2]);
            draw_runner_metrics(f, app, chunks[3]);
        }
        None => {
            let msg = Paragraph::new(" Loading metrics...")
                .block(Block::default().borders(Borders::ALL).title(" Monitoring "));
            f.render_widget(msg, area);
        }
    }
}

fn draw_cpu_gauge(f: &mut Frame, sys: &crate::client::SystemMetrics, area: Rect) {
    let ratio = (sys.cpu_percent / 100.0).clamp(0.0, 1.0);
    let color = gauge_color(ratio);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" CPU "))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(format!("{:.1}%", sys.cpu_percent));
    f.render_widget(gauge, area);
}

fn draw_memory_gauge(f: &mut Frame, sys: &crate::client::SystemMetrics, area: Rect) {
    let ratio = if sys.memory_total_bytes > 0 {
        (sys.memory_used_bytes as f64 / sys.memory_total_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color = gauge_color(ratio);
    let used = format_bytes(sys.memory_used_bytes);
    let total = format_bytes(sys.memory_total_bytes);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Memory "))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(format!("{used} / {total}"));
    f.render_widget(gauge, area);
}

fn draw_disk_gauge(f: &mut Frame, sys: &crate::client::SystemMetrics, area: Rect) {
    let ratio = if sys.disk_total_bytes > 0 {
        (sys.disk_used_bytes as f64 / sys.disk_total_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color = gauge_color(ratio);
    let used = format_bytes(sys.disk_used_bytes);
    let total = format_bytes(sys.disk_total_bytes);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Disk "))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(format!("{used} / {total}"));
    f.render_widget(gauge, area);
}

fn draw_runner_metrics(f: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = match &app.metrics {
        Some(m) if !m.runners.is_empty() => {
            let mut lines = vec![Line::from(Span::styled(
                " Runner                       CPU      Memory",
                Style::default().fg(Color::DarkGray),
            ))];
            for rm in &m.runners {
                let name = app
                    .runners
                    .iter()
                    .find(|r| r.config.id == rm.runner_id)
                    .map(|r| r.config.name.as_str())
                    .unwrap_or(&rm.runner_id);
                lines.push(Line::from(format!(
                    " {:<30}{:>5.1}%   {}",
                    name,
                    rm.cpu_percent,
                    format_bytes(rm.memory_bytes),
                )));
            }
            lines
        }
        _ => vec![Line::from(" No active runner processes.")],
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Per-Runner "));
    f.render_widget(paragraph, area);
}

fn gauge_color(ratio: f64) -> Color {
    if ratio > 0.9 {
        Color::Red
    } else if ratio > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_color_thresholds() {
        assert_eq!(gauge_color(0.5), Color::Green);
        assert_eq!(gauge_color(0.8), Color::Yellow);
        assert_eq!(gauge_color(0.95), Color::Red);
    }
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

Run: `cargo test -p homerun`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add crates/tui/src/ui/monitoring.rs
git commit -m "feat(tui): add Monitoring tab with CPU/memory/disk gauges"
```

---

### Task 9: Plain CLI Mode (--no-tui)

**Files:**
- Create: `crates/tui/src/cli.rs`

- [ ] **Step 1: Write failing test for CLI output formatting**

In `crates/tui/src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_runner_row() {
        let runner = crate::client::RunnerInfo {
            config: crate::client::RunnerConfig {
                id: "abc".to_string(),
                name: "gifted-runner-1".to_string(),
                repo_owner: "aGallea".to_string(),
                repo_name: "gifted".to_string(),
                labels: vec!["self-hosted".to_string(), "macOS".to_string()],
                mode: "app".to_string(),
                work_dir: std::path::PathBuf::from("/tmp"),
            },
            state: "online".to_string(),
            pid: Some(1234),
            uptime_secs: Some(3600),
            jobs_completed: 5,
            jobs_failed: 1,
        };
        let row = format_runner_row(&runner);
        assert!(row.contains("gifted-runner-1"));
        assert!(row.contains("online"));
        assert!(row.contains("aGallea/gifted"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p homerun cli::tests`
Expected: FAIL

- [ ] **Step 3: Implement CLI mode**

```rust
use anyhow::Result;
use crate::client::DaemonClient;

/// Format a single runner as a table row.
pub fn format_runner_row(runner: &crate::client::RunnerInfo) -> String {
    format!(
        "{:<25} {:<10} {:<25} {:<8} {}",
        runner.config.name,
        runner.state,
        format!("{}/{}", runner.config.repo_owner, runner.config.repo_name),
        runner.config.mode,
        runner.config.labels.join(", "),
    )
}

fn print_runner_table(runners: &[crate::client::RunnerInfo]) {
    println!(
        "{:<25} {:<10} {:<25} {:<8} {}",
        "NAME", "STATE", "REPO", "MODE", "LABELS"
    );
    println!("{}", "-".repeat(80));
    for runner in runners {
        println!("{}", format_runner_row(runner));
    }
}

pub async fn run(command: Option<super::Commands>) -> Result<()> {
    let client = DaemonClient::default_socket();

    if !client.socket_exists() {
        eprintln!("Daemon not running. Start it with: homerund");
        std::process::exit(1);
    }

    match command {
        Some(super::Commands::List) => {
            let runners = client.list_runners().await?;
            if runners.is_empty() {
                println!("No runners configured.");
            } else {
                print_runner_table(&runners);
            }
        }
        Some(super::Commands::Status) => {
            let auth = client.auth_status().await?;
            let runners = client.list_runners().await?;
            let metrics = client.get_metrics().await?;

            println!("HomeRun Status");
            println!("{}", "=".repeat(40));
            match auth.user {
                Some(user) => println!("User:      {} (authenticated)", user.login),
                None => println!("User:      not authenticated"),
            }
            println!("Runners:   {}", runners.len());
            let online = runners.iter().filter(|r| r.state == "online").count();
            let busy = runners.iter().filter(|r| r.state == "busy").count();
            println!("  Online:  {online}");
            println!("  Busy:    {busy}");
            println!("CPU:       {:.1}%", metrics.system.cpu_percent);
            println!(
                "Memory:    {:.1} GB / {:.1} GB",
                metrics.system.memory_used_bytes as f64 / 1_073_741_824.0,
                metrics.system.memory_total_bytes as f64 / 1_073_741_824.0,
            );
        }
        None => {
            // --no-tui with no command defaults to status
            let runners = client.list_runners().await?;
            if runners.is_empty() {
                println!("No runners configured. Use `homerun --no-tui list` or launch the TUI with `homerun`.");
            } else {
                print_runner_table(&runners);
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p homerun cli::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/cli.rs crates/tui/src/main.rs
git commit -m "feat(tui): add plain CLI mode with list and status commands"
```

---

### Task 10: Integration Test + Cleanup

**Files:**
- Create: `crates/tui/tests/integration.rs`
- Cleanup: ensure `cargo clippy` and `cargo test` pass across workspace

- [ ] **Step 1: Write integration test**

Create `crates/tui/tests/integration.rs`:

```rust
//! Integration tests for the TUI client.
//!
//! These tests start a real daemon on a temporary Unix socket and exercise
//! the DaemonClient against it.

use std::path::PathBuf;

use tokio::net::UnixListener;

/// Start an in-process daemon and return its socket path.
async fn start_test_daemon() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let socket_path = dir.into_path().join("daemon.sock");

    let config = homerund::config::Config::with_base_dir(
        socket_path.parent().unwrap().join(".homerun"),
    );
    config.ensure_dirs().unwrap();

    let state = homerund::server::AppState::new(config);
    let app = homerund::server::create_router(state);

    let listener = UnixListener::bind(&socket_path).unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to bind
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    socket_path
}

#[tokio::test]
async fn test_client_health_check() {
    let socket = start_test_daemon().await;
    let client = homerun::client::DaemonClient::new(socket);
    client.health().await.unwrap();
}

#[tokio::test]
async fn test_client_runner_lifecycle() {
    let socket = start_test_daemon().await;
    let client = homerun::client::DaemonClient::new(socket);

    // Initially empty
    let runners = client.list_runners().await.unwrap();
    assert!(runners.is_empty());

    // Create a runner
    let req = homerun::client::CreateRunnerRequest {
        repo_full_name: "aGallea/gifted".to_string(),
        name: Some("test-runner".to_string()),
        labels: None,
        mode: None,
    };
    let runner = client.create_runner(&req).await.unwrap();
    assert_eq!(runner.config.name, "test-runner");

    // List should have 1
    let runners = client.list_runners().await.unwrap();
    assert_eq!(runners.len(), 1);

    // Get by ID
    let fetched = client.get_runner(&runner.config.id).await.unwrap();
    assert_eq!(fetched.config.name, "test-runner");

    // Delete
    client.delete_runner(&runner.config.id).await.unwrap();
    let runners = client.list_runners().await.unwrap();
    assert!(runners.is_empty());
}

#[tokio::test]
async fn test_client_metrics() {
    let socket = start_test_daemon().await;
    let client = homerun::client::DaemonClient::new(socket);
    let metrics = client.get_metrics().await.unwrap();
    assert!(metrics.system.memory_total_bytes > 0);
}
```

- [ ] **Step 2: Add homerund as a dev-dependency of the tui crate**

In `crates/tui/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
homerund = { path = "../daemon" }
axum = "0.8"
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test -p homerun --test integration`
Expected: 3 tests PASS

- [ ] **Step 4: Run clippy and fix any warnings**

Run: `cargo clippy -p homerun -- -D warnings`
Fix any warnings.

- [ ] **Step 5: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/tui/tests/ crates/tui/Cargo.toml
git commit -m "test(tui): add integration tests for DaemonClient against real daemon"
```
