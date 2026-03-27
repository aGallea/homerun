# Compact Mini-View & Menu Bar Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an always-on-top mini window and a macOS menu bar tray icon with rich dropdown for at-a-glance runner monitoring.

**Architecture:** Tauri 2 system tray (`tray-icon` feature) for the menu bar icon with dynamic state-based icons. Two additional Tauri webview windows created programmatically: a transparent always-on-top mini window (`/mini` route) and a borderless tray dropdown panel (`/tray` route). The `tauri-plugin-positioner` positions the dropdown relative to the tray icon. Mini window position persists to a local JSON file in the app data directory.

**Tech Stack:** Tauri 2 (Rust), React 19, TypeScript, `tauri-plugin-positioner`

---

## File Map

### New Files

| File                                    | Responsibility                                                  |
| --------------------------------------- | --------------------------------------------------------------- |
| `apps/desktop/src/pages/MiniView.tsx`   | Mini always-on-top window React component                       |
| `apps/desktop/src/pages/TrayPanel.tsx`  | Tray dropdown panel React component                             |
| `apps/desktop/src/hooks/useTrayIcon.ts` | Computes aggregate runner state + calls tray icon update        |
| `apps/desktop/src-tauri/src/tray.rs`    | Tray icon initialization, event handling, icon state updates    |
| `apps/desktop/src-tauri/src/window.rs`  | Mini window + tray panel creation, toggle, position persistence |
| `apps/desktop/src-tauri/icons/tray/`    | Resized tray icon PNGs (22x22 @2x)                              |

### Modified Files

| File                                               | Changes                                                                                                                                               |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/desktop/src-tauri/Cargo.toml`                | Add `tray-icon`, `image-png` features + `tauri-plugin-positioner`                                                                                     |
| `apps/desktop/src-tauri/src/lib.rs`                | Register positioner plugin, tray init, new commands, `mod tray; mod window;`                                                                          |
| `apps/desktop/src-tauri/capabilities/default.json` | Add `mini` and `tray-panel` windows + positioner permissions                                                                                          |
| `apps/desktop/src/App.tsx`                         | Add `/mini` and `/tray` routes outside Layout                                                                                                         |
| `apps/desktop/src/api/commands.ts`                 | Add `updateTrayIcon`, `toggleMiniWindow`, `showMainWindow`, `saveMiniPosition`, `getMiniPosition`, `toggleTrayPanel`, `stopDaemonFromTray`, `quitApp` |
| `apps/desktop/src/api/types.ts`                    | Add `TrayIconState` type                                                                                                                              |
| `apps/desktop/src/index.css`                       | Add mini-view and tray-panel styles                                                                                                                   |

---

## Task 1: Add Tauri Dependencies and Features

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`

- [ ] **Step 1: Update Cargo.toml with tray-icon feature and positioner plugin**

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-shell = "2"
tauri-plugin-opener = "2"
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
hyper = { version = "1", features = ["client", "http1"] }
hyper-util = { version = "0.1", features = ["tokio", "client-legacy", "http1"] }
http-body-util = "0.1"
tower = "0.5"
dirs = "5"
```

- [ ] **Step 2: Update capabilities to include new windows and permissions**

Replace `apps/desktop/src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for all app windows",
  "windows": ["main", "mini", "tray-panel"],
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-set-focus",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-position",
    "core:window:allow-set-always-on-top",
    "core:window:allow-start-dragging",
    "opener:default",
    "shell:allow-open",
    "shell:allow-execute",
    "shell:allow-spawn",
    "positioner:default"
  ]
}
```

- [ ] **Step 3: Verify it compiles**

Run from `apps/desktop/src-tauri/`:

```bash
cd apps/desktop && npm run tauri build -- --debug 2>&1 | head -30
```

If there's a compilation error about unresolved imports, that's expected — we haven't registered the positioner plugin yet. The goal is to verify dependencies resolve. Alternatively, run:

```bash
cd apps/desktop/src-tauri && cargo check 2>&1 | tail -20
```

Expected: compiles (possibly with unused import warnings). If `tauri-plugin-positioner` fails to resolve, check that the version matches Tauri 2.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/capabilities/default.json
git commit -m "feat(desktop): add tray-icon and positioner dependencies"
```

---

## Task 2: Prepare Tray Icon Assets

**Files:**

- Create: `apps/desktop/src-tauri/icons/tray/` directory
- Source: `assets/homerun_idle.png`, `homerun_active.png`, `homerun_error.png`, `homerun_offline.png`

macOS menu bar icons should be 44x44px (22pt @2x). The source PNGs are much larger and need to be resized.

- [ ] **Step 1: Create tray icon directory and resize icons**

```bash
mkdir -p apps/desktop/src-tauri/icons/tray
for state in idle active error offline; do
  sips -z 44 44 "assets/homerun_${state}.png" --out "apps/desktop/src-tauri/icons/tray/${state}.png" 2>/dev/null
done
```

Verify the output:

```bash
file apps/desktop/src-tauri/icons/tray/*.png
```

Expected: four 44x44 PNG files.

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src-tauri/icons/tray/
git commit -m "feat(desktop): add resized tray icon assets"
```

---

## Task 3: Initialize System Tray with Static Icon

**Files:**

- Create: `apps/desktop/src-tauri/src/tray.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create tray.rs with initialization function**

Create `apps/desktop/src-tauri/src/tray.rs`:

```rust
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

const TRAY_ID: &str = "homerun-tray";

pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = Image::from_path(
        app.path()
            .resource_dir()
            .unwrap_or_default()
            .join("icons/tray/idle.png"),
    )
    .or_else(|_| Image::from_bytes(include_bytes!("../icons/tray/idle.png")))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("HomeRun")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                crate::window::toggle_tray_panel_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

pub fn update_icon(app: &AppHandle, state: &str) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "Tray icon not found".to_string())?;

    let icon_bytes: &[u8] = match state {
        "active" => include_bytes!("../icons/tray/active.png"),
        "error" => include_bytes!("../icons/tray/error.png"),
        "offline" => include_bytes!("../icons/tray/offline.png"),
        _ => include_bytes!("../icons/tray/idle.png"),
    };

    let icon = Image::from_bytes(icon_bytes).map_err(|e| e.to_string())?;
    tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;

    let tooltip = match state {
        "active" => "HomeRun — runners active",
        "error" => "HomeRun — runner error",
        "offline" => "HomeRun — daemon offline",
        _ => "HomeRun — idle",
    };
    tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 2: Create a stub window.rs (needed by tray.rs)**

Create `apps/desktop/src-tauri/src/window.rs`:

```rust
use tauri::AppHandle;

pub fn toggle_tray_panel_window(_app: &AppHandle) {
    // Stub — implemented in Task 6
}
```

- [ ] **Step 3: Update lib.rs to register tray module and positioner plugin**

Add module declarations at the top of `apps/desktop/src-tauri/src/lib.rs`:

```rust
mod client;
mod commands;
mod tray;
mod window;
```

Add the positioner plugin registration in the builder chain (after `tauri_plugin_opener`):

```rust
.plugin(tauri_plugin_positioner::init())
```

Add tray initialization at the end of the `setup` closure, before `Ok(())`:

```rust
            // -- Initialize system tray --
            if let Err(e) = tray::init(app) {
                eprintln!("Failed to initialize tray: {e}");
            }
```

- [ ] **Step 4: Verify it compiles**

```bash
cd apps/desktop/src-tauri && cargo check
```

Expected: compiles cleanly (possibly warnings about unused `window` module).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/tray.rs apps/desktop/src-tauri/src/window.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): initialize system tray with idle icon"
```

---

## Task 4: Add Tray Icon State Update Command

**Files:**

- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (register command)
- Modify: `apps/desktop/src/api/commands.ts`
- Modify: `apps/desktop/src/api/types.ts`

- [ ] **Step 1: Add the Rust command in commands.rs**

Add at the bottom of `apps/desktop/src-tauri/src/commands.rs`:

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn update_tray_icon(
    app_handle: tauri::AppHandle,
    state: String,
) -> Result<(), String> {
    crate::tray::update_icon(&app_handle, &state)
}
```

- [ ] **Step 2: Register the command in lib.rs**

Add `commands::update_tray_icon` to the `invoke_handler` list in `lib.rs`:

```rust
commands::delete_history_entry,
commands::update_tray_icon,
```

- [ ] **Step 3: Add TypeScript type for tray icon state**

Add to `apps/desktop/src/api/types.ts`:

```typescript
export type TrayIconState = "idle" | "active" | "error" | "offline";
```

- [ ] **Step 4: Add frontend command wrapper**

Add to the `api` object in `apps/desktop/src/api/commands.ts`:

```typescript
// Tray
updateTrayIcon: (state: TrayIconState) =>
  invoke<void>("update_tray_icon", { state }),
```

Add `TrayIconState` to the import from `"./types"`.

- [ ] **Step 5: Verify TypeScript compiles**

```bash
cd apps/desktop && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src/api/commands.ts apps/desktop/src/api/types.ts
git commit -m "feat(desktop): add update_tray_icon command"
```

---

## Task 5: Add Mini Window and Main Window Tauri Commands

**Files:**

- Modify: `apps/desktop/src-tauri/src/window.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/api/commands.ts`
- Modify: `apps/desktop/src/api/types.ts`

- [ ] **Step 1: Implement window.rs with mini window management**

Replace `apps/desktop/src-tauri/src/window.rs`:

```rust
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_positioner::{Position, WindowExt};

const MINI_LABEL: &str = "mini";
const TRAY_PANEL_LABEL: &str = "tray-panel";
const MINI_WIDTH: f64 = 280.0;
const MINI_HEIGHT: f64 = 200.0;
const TRAY_PANEL_WIDTH: f64 = 300.0;
const TRAY_PANEL_HEIGHT: f64 = 420.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniPosition {
    pub x: f64,
    pub y: f64,
}

/// Toggle the mini always-on-top window. Creates it on first call.
pub fn toggle_mini_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MINI_LABEL) {
        if win.is_visible().unwrap_or(false) {
            win.hide().map_err(|e| e.to_string())?;
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    // Create mini window
    let url = WebviewUrl::App("/mini".into());
    let builder = WebviewWindowBuilder::new(app, MINI_LABEL, url)
        .title("HomeRun Mini")
        .inner_size(MINI_WIDTH, MINI_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true);

    let win = builder.build().map_err(|e| e.to_string())?;

    // Restore saved position or default to top-right
    if let Some(pos) = load_mini_position(app) {
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            pos.x, pos.y,
        )));
    } else if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let x = (screen.width as f64 / scale) - MINI_WIDTH - 20.0;
        let y = 40.0;
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
    }

    Ok(())
}

/// Show and focus the main window, hide the mini window.
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(mini) = app.get_webview_window(MINI_LABEL) {
        let _ = mini.hide();
    }
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.unminimize().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Toggle the tray dropdown panel. Creates it on first call.
pub fn toggle_tray_panel_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(TRAY_PANEL_LABEL) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.as_ref().move_window(Position::TrayBottomCenter);
        }
        return;
    }

    let url = WebviewUrl::App("/tray".into());
    let builder = WebviewWindowBuilder::new(app, TRAY_PANEL_LABEL, url)
        .title("HomeRun Tray")
        .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .focused(true)
        .visible(true);

    if let Ok(win) = builder.build() {
        let _ = win.as_ref().move_window(Position::TrayBottomCenter);

        // Hide on blur
        let app_handle = app.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if let Some(panel) = app_handle.get_webview_window(TRAY_PANEL_LABEL) {
                    let _ = panel.hide();
                }
            }
        });
    }
}

/// Save mini window position to local app data.
pub fn save_mini_pos(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    let path = mini_position_path(app)?;
    let pos = MiniPosition { x, y };
    let json = serde_json::to_string(&pos).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load mini window position from local app data.
pub fn load_mini_position(app: &AppHandle) -> Option<MiniPosition> {
    let path = mini_position_path(app).ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn mini_position_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("mini_position.json"))
}
```

- [ ] **Step 2: Add Tauri commands in commands.rs**

Add at the bottom of `commands.rs`:

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_mini_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::toggle_mini_window(&app_handle)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn show_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::show_main_window(&app_handle)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_mini_position(
    app_handle: tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<(), String> {
    crate::window::save_mini_pos(&app_handle, x, y)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_mini_position(
    app_handle: tauri::AppHandle,
) -> Result<Option<(f64, f64)>, String> {
    Ok(crate::window::load_mini_position(&app_handle).map(|p| (p.x, p.y)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn quit_app(app_handle: tauri::AppHandle) -> Result<(), String> {
    app_handle.exit(0);
    Ok(())
}
```

- [ ] **Step 3: Register new commands in lib.rs**

Add to the `invoke_handler` list:

```rust
commands::update_tray_icon,
commands::toggle_mini_window,
commands::show_main_window,
commands::save_mini_position,
commands::get_mini_position,
commands::quit_app,
```

- [ ] **Step 4: Add frontend command wrappers**

Add to the `api` object in `apps/desktop/src/api/commands.ts`:

```typescript
// Window management
toggleMiniWindow: () => invoke<void>("toggle_mini_window"),
showMainWindow: () => invoke<void>("show_main_window"),
saveMiniPosition: (x: number, y: number) => invoke<void>("save_mini_position", { x, y }),
getMiniPosition: () => invoke<[number, number] | null>("get_mini_position"),
quitApp: () => invoke<void>("quit_app"),
```

- [ ] **Step 5: Verify compilation**

```bash
cd apps/desktop/src-tauri && cargo check && cd .. && npx tsc --noEmit
```

Expected: both compile cleanly.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/window.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src/api/commands.ts
git commit -m "feat(desktop): add mini window and main window toggle commands"
```

---

## Task 6: Add Frontend Routes for Mini and Tray Views

**Files:**

- Modify: `apps/desktop/src/App.tsx`

The `/mini` and `/tray` routes must be **outside** the `<Layout>` wrapper — they don't need the sidebar, daemon banner, or full app chrome.

- [ ] **Step 1: Update App.tsx with new routes**

Replace `apps/desktop/src/App.tsx`:

```tsx
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { AuthProvider } from "./hooks/useAuth";
import { Layout } from "./components/Layout";
import { Dashboard } from "./pages/Dashboard";
import { Repositories } from "./pages/Repositories";
import { RunnerDetail } from "./pages/RunnerDetail";
import { Settings } from "./pages/Settings";
import { Daemon } from "./pages/Daemon";
import { MiniView } from "./pages/MiniView";
import { TrayPanel } from "./pages/TrayPanel";

function App() {
  return (
    <AuthProvider>
      <BrowserRouter>
        <Routes>
          {/* Standalone windows — no Layout wrapper */}
          <Route path="/mini" element={<MiniView />} />
          <Route path="/tray" element={<TrayPanel />} />

          {/* Main app with sidebar layout */}
          <Route element={<Layout />}>
            <Route index element={<Navigate to="/dashboard" replace />} />
            <Route path="/dashboard" element={<Dashboard />} />
            <Route path="/repositories" element={<Repositories />} />
            <Route path="/runners/:id" element={<RunnerDetail />} />
            <Route path="/daemon" element={<Daemon />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </AuthProvider>
  );
}

export default App;
```

- [ ] **Step 2: Create stub MiniView page**

Create `apps/desktop/src/pages/MiniView.tsx`:

```tsx
export function MiniView() {
  return <div className="mini-view">Mini View — loading...</div>;
}
```

- [ ] **Step 3: Create stub TrayPanel page**

Create `apps/desktop/src/pages/TrayPanel.tsx`:

```tsx
export function TrayPanel() {
  return <div className="tray-panel">Tray Panel — loading...</div>;
}
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
cd apps/desktop && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/App.tsx apps/desktop/src/pages/MiniView.tsx apps/desktop/src/pages/TrayPanel.tsx
git commit -m "feat(desktop): add /mini and /tray routes with stub components"
```

---

## Task 7: Build MiniView Component

**Files:**

- Modify: `apps/desktop/src/pages/MiniView.tsx`
- Modify: `apps/desktop/src/index.css`

- [ ] **Step 1: Implement the MiniView component**

Replace `apps/desktop/src/pages/MiniView.tsx`:

```tsx
import { useEffect, useRef } from "react";
import { useRunners } from "../hooks/useRunners";
import { api } from "../api/commands";
import type { RunnerInfo, RunnerState } from "../api/types";

function formatElapsed(jobStartedAt: string | null | undefined): string {
  if (!jobStartedAt) return "";
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return "";
  const secs = Math.floor((Date.now() - started) / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}m ${rem.toString().padStart(2, "0")}s`;
}

function jobProgress(
  jobStartedAt: string | null | undefined,
  estimatedDuration: number | null | undefined,
): number | null {
  if (!jobStartedAt || !estimatedDuration || estimatedDuration <= 0) return null;
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return null;
  const elapsed = (Date.now() - started) / 1000;
  return Math.min(elapsed / estimatedDuration, 1);
}

function countByState(runners: RunnerInfo[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const r of runners) {
    const key = r.state === "busy" ? "busy" : r.state === "offline" ? "offline" : "online";
    counts[key] = (counts[key] || 0) + 1;
  }
  return counts;
}

export function MiniView() {
  const { runners } = useRunners();
  const positionSaved = useRef(false);

  const busy = runners
    .filter((r) => r.state === "busy")
    .sort((a, b) => {
      const aTime = a.job_started_at ? new Date(a.job_started_at).getTime() : -Infinity;
      const bTime = b.job_started_at ? new Date(b.job_started_at).getTime() : -Infinity;
      return bTime - aTime;
    });

  const counts = countByState(runners);
  const daemonOk = runners.length > 0 || !document.hidden;

  // Save position on window move (debounced)
  useEffect(() => {
    async function onMove() {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const pos = await win.outerPosition();
      const scale = (await win.scaleFactor()) || 1;
      await api.saveMiniPosition(pos.x / scale, pos.y / scale);
    }

    const handler = () => {
      if (positionSaved.current) return;
      positionSaved.current = true;
      setTimeout(() => {
        positionSaved.current = false;
        onMove().catch(() => {});
      }, 500);
    };

    window.addEventListener("mouseup", handler);
    return () => window.removeEventListener("mouseup", handler);
  }, []);

  return (
    <div className="mini-view" data-tauri-drag-region>
      <div className="mini-header" data-tauri-drag-region>
        <div className="mini-header-left" data-tauri-drag-region>
          <span className={`mini-health-dot ${daemonOk ? "online" : "offline"}`} />
          <span className="mini-label">HOMERUN</span>
        </div>
        <div className="mini-header-right" data-tauri-drag-region>
          {(counts.online || 0) > 0 && (
            <span className="mini-count online">{counts.online} online</span>
          )}
          {(counts.busy || 0) > 0 && <span className="mini-count busy">{counts.busy} busy</span>}
          {(counts.offline || 0) > 0 && (
            <span className="mini-count offline">{counts.offline} off</span>
          )}
        </div>
      </div>

      {busy.map((runner) => {
        const pct = jobProgress(runner.job_started_at, runner.estimated_job_duration_secs);
        return (
          <div key={runner.config.id} className="mini-runner-card">
            <div className="mini-runner-top">
              <span className="mini-runner-name">{runner.config.name}</span>
              <span className="mini-runner-time">{formatElapsed(runner.job_started_at)}</span>
            </div>
            <div className="mini-runner-job">{runner.current_job ?? "Starting..."}</div>
            {pct != null && (
              <div className="mini-progress-track">
                <div
                  className={`mini-progress-bar${pct >= 1 ? " over" : ""}`}
                  style={{ width: `${Math.min(pct, 1) * 100}%` }}
                />
              </div>
            )}
          </div>
        );
      })}

      {busy.length === 0 && runners.length > 0 && (
        <div className="mini-empty">All runners idle</div>
      )}
      {runners.length === 0 && <div className="mini-empty">No runners</div>}
    </div>
  );
}
```

- [ ] **Step 2: Add mini-view CSS**

Add to the bottom of `apps/desktop/src/index.css`:

```css
/* ---- Mini View (always-on-top window) ---- */
.mini-view {
  background: rgba(13, 17, 23, 0.82);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid rgba(48, 54, 61, 0.6);
  border-radius: 12px;
  padding: 12px 14px;
  font-size: 12px;
  color: var(--text-primary);
  min-height: 60px;
  user-select: none;
  -webkit-user-select: none;
}

.mini-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.mini-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.mini-health-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.mini-health-dot.online {
  background: var(--accent-green);
}

.mini-health-dot.offline {
  background: var(--text-secondary);
}

.mini-label {
  font-weight: 600;
  font-size: 11px;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
}

.mini-header-right {
  display: flex;
  gap: 8px;
  font-size: 11px;
}

.mini-count.online {
  color: var(--accent-green);
}

.mini-count.busy {
  color: var(--accent-yellow);
}

.mini-count.offline {
  color: var(--text-secondary);
}

.mini-runner-card {
  background: rgba(33, 38, 45, 0.7);
  border-radius: 8px;
  padding: 8px 10px;
  margin-bottom: 6px;
}

.mini-runner-card:last-child {
  margin-bottom: 0;
}

.mini-runner-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.mini-runner-name {
  font-size: 11px;
  font-weight: 500;
}

.mini-runner-time {
  font-size: 10px;
  color: var(--text-secondary);
}

.mini-runner-job {
  font-size: 10px;
  color: var(--accent-yellow);
  margin-bottom: 5px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-progress-track {
  height: 3px;
  background: rgba(48, 54, 61, 0.8);
  border-radius: 2px;
  overflow: hidden;
}

.mini-progress-bar {
  height: 100%;
  background: var(--accent-yellow);
  border-radius: 2px;
  transition: width 1s linear;
}

.mini-progress-bar.over {
  background: var(--accent-red);
}

.mini-empty {
  font-size: 11px;
  color: var(--text-secondary);
  text-align: center;
  padding: 8px 0;
}
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd apps/desktop && npx tsc --noEmit
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/pages/MiniView.tsx apps/desktop/src/index.css
git commit -m "feat(desktop): implement MiniView component with runner cards"
```

---

## Task 8: Build TrayPanel Component

**Files:**

- Modify: `apps/desktop/src/pages/TrayPanel.tsx`
- Modify: `apps/desktop/src/index.css`

- [ ] **Step 1: Implement the TrayPanel component**

Replace `apps/desktop/src/pages/TrayPanel.tsx`:

```tsx
import { useState, useEffect, useCallback } from "react";
import { api } from "../api/commands";
import type { RunnerInfo } from "../api/types";

function formatElapsed(jobStartedAt: string | null | undefined): string {
  if (!jobStartedAt) return "";
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return "";
  const secs = Math.floor((Date.now() - started) / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}m ${rem.toString().padStart(2, "0")}s`;
}

function jobProgress(
  jobStartedAt: string | null | undefined,
  estimatedDuration: number | null | undefined,
): number | null {
  if (!jobStartedAt || !estimatedDuration || estimatedDuration <= 0) return null;
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return null;
  const elapsed = (Date.now() - started) / 1000;
  return Math.min(elapsed / estimatedDuration, 1);
}

const stateColors: Record<string, string> = {
  online: "var(--accent-green)",
  busy: "var(--accent-yellow)",
  offline: "#484f58",
  error: "var(--accent-red)",
  creating: "var(--accent-blue)",
  registering: "var(--accent-blue)",
  stopping: "var(--accent-yellow)",
  deleting: "var(--accent-red)",
};

function stateLabel(r: RunnerInfo): string {
  if (r.state === "busy") return "";
  if (r.state === "online") return "Idle";
  return r.state.charAt(0).toUpperCase() + r.state.slice(1);
}

export function TrayPanel() {
  const [runners, setRunners] = useState<RunnerInfo[]>([]);
  const [daemonOk, setDaemonOk] = useState(true);
  const [daemonStopping, setDaemonStopping] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const data = await api.listRunners();
      setRunners(data);
      setDaemonOk(true);
    } catch {
      setRunners([]);
      setDaemonOk(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);
    return () => clearInterval(interval);
  }, [refresh]);

  const counts = { online: 0, busy: 0, offline: 0 };
  for (const r of runners) {
    if (r.state === "busy") counts.busy++;
    else if (r.state === "offline" || r.state === "error") counts.offline++;
    else counts.online++;
  }

  // Sort: busy first, then online, then offline
  const sorted = [...runners].sort((a, b) => {
    const order: Record<string, number> = { busy: 0, online: 1, creating: 1, registering: 1 };
    return (order[a.state] ?? 2) - (order[b.state] ?? 2);
  });

  return (
    <div className="tray-panel">
      {/* Header */}
      <div className="tray-header">
        <div className="tray-header-left">
          <span
            className="tray-health-dot"
            style={{ background: daemonOk ? "var(--accent-green)" : "#484f58" }}
          />
          <span className="tray-title">HomeRun</span>
        </div>
        <span className="tray-daemon-status">{daemonOk ? "Daemon running" : "Daemon offline"}</span>
      </div>

      {/* Summary bar */}
      <div className="tray-summary">
        <span>
          <strong style={{ color: "var(--accent-green)" }}>{counts.online}</strong>{" "}
          <span className="tray-muted">online</span>
        </span>
        <span>
          <strong style={{ color: "var(--accent-yellow)" }}>{counts.busy}</strong>{" "}
          <span className="tray-muted">busy</span>
        </span>
        <span>
          <strong className="tray-muted">{counts.offline}</strong>{" "}
          <span className="tray-muted">offline</span>
        </span>
      </div>

      {/* Runner list */}
      <div className="tray-runners">
        {sorted.map((runner) => {
          const pct = jobProgress(runner.job_started_at, runner.estimated_job_duration_secs);
          const dotColor = stateColors[runner.state] || "var(--text-secondary)";
          const isOff = runner.state === "offline" || runner.state === "error";
          return (
            <div key={runner.config.id} className="tray-runner-row">
              <span className="tray-runner-dot" style={{ background: dotColor }} />
              <div className="tray-runner-info">
                <div className="tray-runner-top">
                  <span
                    className="tray-runner-name"
                    style={isOff ? { color: "var(--text-secondary)" } : undefined}
                  >
                    {runner.config.name}
                  </span>
                  {runner.state === "busy" && (
                    <span className="tray-runner-time">{formatElapsed(runner.job_started_at)}</span>
                  )}
                  {runner.state !== "busy" && (
                    <span className="tray-runner-state" style={{ color: dotColor }}>
                      {stateLabel(runner)}
                    </span>
                  )}
                </div>
                {runner.state === "busy" && (
                  <>
                    <div className="tray-runner-job">{runner.current_job ?? "Starting..."}</div>
                    {pct != null && (
                      <div className="tray-progress-track">
                        <div
                          className="tray-progress-bar"
                          style={{ width: `${Math.min(pct, 1) * 100}%` }}
                        />
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          );
        })}
        {runners.length === 0 && (
          <div className="tray-no-runners">
            {daemonOk ? "No runners configured" : "Cannot reach daemon"}
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="tray-actions">
        <button className="tray-action" onClick={() => api.toggleMiniWindow()}>
          <span>Toggle Mini View</span>
          <span className="tray-shortcut">⌘⇧M</span>
        </button>
        <button className="tray-action" onClick={() => api.showMainWindow()}>
          <span>Open HomeRun</span>
          <span className="tray-shortcut">⌘⇧H</span>
        </button>
        <button
          className="tray-action danger"
          disabled={daemonStopping}
          onClick={async () => {
            setDaemonStopping(true);
            try {
              if (daemonOk) {
                await api.stopDaemon();
              } else {
                await api.startDaemon();
              }
            } catch {
              /* ignore */
            } finally {
              setDaemonStopping(false);
              refresh();
            }
          }}
        >
          {daemonOk ? "Stop Daemon" : "Start Daemon"}
        </button>
        <button className="tray-action" onClick={() => api.quitApp()}>
          <span>Quit HomeRun</span>
          <span className="tray-shortcut">⌘Q</span>
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add tray-panel CSS**

Add to the bottom of `apps/desktop/src/index.css`:

```css
/* ---- Tray Panel (dropdown from menu bar) ---- */
.tray-panel {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  font-size: 13px;
  color: var(--text-primary);
  overflow: hidden;
  user-select: none;
  -webkit-user-select: none;
}

.tray-header {
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.tray-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tray-health-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.tray-title {
  font-weight: 600;
}

.tray-daemon-status {
  font-size: 11px;
  color: var(--text-secondary);
}

.tray-summary {
  padding: 8px 14px;
  display: flex;
  gap: 12px;
  font-size: 12px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-primary);
}

.tray-muted {
  color: var(--text-secondary);
}

.tray-runners {
  padding: 6px 0;
  max-height: 240px;
  overflow-y: auto;
}

.tray-runner-row {
  padding: 8px 14px;
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.tray-runner-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  margin-top: 5px;
}

.tray-runner-info {
  flex: 1;
  min-width: 0;
}

.tray-runner-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.tray-runner-name {
  font-size: 12px;
  font-weight: 500;
}

.tray-runner-time {
  font-size: 10px;
  color: var(--text-secondary);
}

.tray-runner-state {
  font-size: 11px;
}

.tray-runner-job {
  font-size: 11px;
  color: var(--accent-yellow);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tray-progress-track {
  height: 2px;
  background: var(--bg-tertiary);
  border-radius: 1px;
  margin-top: 4px;
  overflow: hidden;
}

.tray-progress-bar {
  height: 100%;
  background: var(--accent-yellow);
  border-radius: 1px;
  transition: width 1s linear;
}

.tray-no-runners {
  padding: 12px 14px;
  font-size: 12px;
  color: var(--text-secondary);
  text-align: center;
}

.tray-actions {
  border-top: 1px solid var(--border);
  padding: 6px 0;
}

.tray-action {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  padding: 6px 14px;
  font-size: 12px;
  color: var(--text-secondary);
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
}

.tray-action:hover {
  background: var(--bg-tertiary);
}

.tray-action.danger {
  color: var(--accent-red);
}

.tray-shortcut {
  font-size: 10px;
  opacity: 0.6;
}
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd apps/desktop && npx tsc --noEmit
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/pages/TrayPanel.tsx apps/desktop/src/index.css
git commit -m "feat(desktop): implement TrayPanel component with runner list and actions"
```

---

## Task 9: Add useTrayIcon Hook for Automatic State Updates

**Files:**

- Create: `apps/desktop/src/hooks/useTrayIcon.ts`
- Modify: `apps/desktop/src/components/Layout.tsx`
- Modify: `apps/desktop/src/pages/MiniView.tsx`

The hook computes the aggregate tray icon state from the runner list and calls the `update_tray_icon` command whenever it changes.

- [ ] **Step 1: Create useTrayIcon hook**

Create `apps/desktop/src/hooks/useTrayIcon.ts`:

```typescript
import { useEffect, useRef } from "react";
import type { RunnerInfo, TrayIconState } from "../api/types";
import { api } from "../api/commands";

function computeTrayState(runners: RunnerInfo[], daemonOk: boolean): TrayIconState {
  if (!daemonOk) return "offline";
  if (runners.some((r) => r.state === "error")) return "error";
  if (runners.some((r) => r.state === "busy")) return "active";
  return "idle";
}

export function useTrayIcon(runners: RunnerInfo[], daemonOk: boolean) {
  const lastState = useRef<TrayIconState | null>(null);

  useEffect(() => {
    const state = computeTrayState(runners, daemonOk);
    if (state !== lastState.current) {
      lastState.current = state;
      api.updateTrayIcon(state).catch(() => {});
    }
  }, [runners, daemonOk]);
}
```

- [ ] **Step 2: Wire useTrayIcon into Layout.tsx**

Add to `apps/desktop/src/components/Layout.tsx`:

Import the hook:

```typescript
import { useTrayIcon } from "../hooks/useTrayIcon";
```

Call it inside the `Layout` component, after `const runnersHook = useRunners();`:

```typescript
useTrayIcon(runnersHook.runners, daemonConnected);
```

- [ ] **Step 3: Also wire into MiniView (it runs in its own window)**

Add to `apps/desktop/src/pages/MiniView.tsx`:

Import the hook:

```typescript
import { useTrayIcon } from "../hooks/useTrayIcon";
```

Call it inside `MiniView`, after the `useRunners()` call. The mini view doesn't track daemon health separately, so derive it from whether runners loaded:

```typescript
const daemonOk = error === null;
useTrayIcon(runners, daemonOk);
```

Also destructure `error` from `useRunners()`:

```typescript
const { runners, error } = useRunners();
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
cd apps/desktop && npx tsc --noEmit
```

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/hooks/useTrayIcon.ts apps/desktop/src/components/Layout.tsx apps/desktop/src/pages/MiniView.tsx
git commit -m "feat(desktop): add useTrayIcon hook for automatic tray icon state updates"
```

---

## Task 10: Wire Up Window Lifecycle (Main/Mini Mutual Exclusivity)

**Files:**

- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

When switching to mini view, hide the main window. When opening main, hide mini.

- [ ] **Step 1: Update toggle_mini_window to hide main**

In `apps/desktop/src-tauri/src/window.rs`, update the `toggle_mini_window` function. After creating or showing the mini window, hide the main window:

Add this at the end of the function, before `Ok(())` (both in the existing-window branch and the create branch):

After `win.show()`:

```rust
// Hide main window when showing mini
if let Some(main_win) = app.get_webview_window("main") {
    let _ = main_win.hide();
}
```

After the initial `builder.build()`:

```rust
// Hide main window
if let Some(main_win) = app.get_webview_window("main") {
    let _ = main_win.hide();
}
```

And in the existing-window `hide` branch — when hiding mini, show main:

```rust
win.hide().map_err(|e| e.to_string())?;
// Show main window when hiding mini
if let Some(main_win) = app.get_webview_window("main") {
    let _ = main_win.show();
    let _ = main_win.set_focus();
}
```

The full updated toggle logic should be:

```rust
pub fn toggle_mini_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MINI_LABEL) {
        if win.is_visible().unwrap_or(false) {
            win.hide().map_err(|e| e.to_string())?;
            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.show();
                let _ = main_win.set_focus();
            }
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.hide();
            }
        }
        return Ok(());
    }

    let url = WebviewUrl::App("/mini".into());
    let builder = WebviewWindowBuilder::new(app, MINI_LABEL, url)
        .title("HomeRun Mini")
        .inner_size(MINI_WIDTH, MINI_HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true);

    let win = builder.build().map_err(|e| e.to_string())?;

    if let Some(pos) = load_mini_position(app) {
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            pos.x, pos.y,
        )));
    } else if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let x = (screen.width as f64 / scale) - MINI_WIDTH - 20.0;
        let y = 40.0;
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
    }

    // Hide main window
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.hide();
    }

    Ok(())
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd apps/desktop/src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/window.rs
git commit -m "feat(desktop): wire main/mini mutual exclusivity on toggle"
```

---

## Task 11: Add Transparent Background for Mini and Tray Windows

**Files:**

- Modify: `apps/desktop/src/index.css`

The mini window and tray panel need transparent `html`/`body` backgrounds so the frosted glass effect shows through.

- [ ] **Step 1: Add transparent background styles scoped to mini/tray routes**

Since the mini and tray views are separate Tauri windows loading `/mini` and `/tray` routes, we can scope the transparent background using a parent class. Update the MiniView and TrayPanel root elements — they already use `.mini-view` and `.tray-panel` classes.

Add to the top of `apps/desktop/src/index.css`, after the `#root` rule:

```css
/* Transparent background for standalone windows (mini, tray) */
html:has(.mini-view),
html:has(.mini-view) body,
html:has(.mini-view) #root {
  background: transparent;
}

html:has(.tray-panel),
html:has(.tray-panel) body,
html:has(.tray-panel) #root {
  background: transparent;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/desktop/src/index.css
git commit -m "style(desktop): add transparent background for mini and tray windows"
```

---

## Task 12: Manual Integration Test

- [ ] **Step 1: Build and run the app**

```bash
cd apps/desktop
npm run tauri dev
```

- [ ] **Step 2: Verify tray icon appears**

Look for the HomeRun "H" icon in the macOS menu bar. It should show the idle (green dot) icon.

- [ ] **Step 3: Test tray dropdown**

Click the tray icon. A dropdown panel should appear below it showing:

- Daemon status header
- Runner summary counts
- Runner list
- Action buttons (Toggle Mini View, Open HomeRun, Stop Daemon, Quit)

Click outside the panel — it should hide.

- [ ] **Step 4: Test mini window toggle**

Click the tray icon → click "Toggle Mini View". The main window should hide, and a small transparent mini window should appear in the top-right corner showing runner status.

- [ ] **Step 5: Test mini window drag + position persistence**

Drag the mini window to a different position. Click the tray → "Toggle Mini View" (hides mini, shows main). Click again to re-show mini — it should appear at the dragged position.

- [ ] **Step 6: Test main window restore**

From the tray dropdown, click "Open HomeRun". The main window should appear and the mini window should hide.

- [ ] **Step 7: Test tray icon state changes**

If runners are configured, observe the tray icon change:

- Idle (green dot) when all runners online
- Active (orange gear) when a runner picks up a job
- Error (red) if a runner errors
- Offline (dark) if daemon is stopped

- [ ] **Step 8: Fix any issues found during testing**

Address visual glitches, positioning issues, or event handling bugs. Common issues:

- Transparent background not working → verify `transparent(true)` on both `WebviewWindowBuilder` and CSS
- Tray panel not positioning correctly → check positioner plugin registration
- Mini window not draggable → verify `data-tauri-drag-region` attributes

- [ ] **Step 9: Final commit**

```bash
git add -A
git commit -m "feat(desktop): complete compact mini-view and menu bar tray integration"
```
