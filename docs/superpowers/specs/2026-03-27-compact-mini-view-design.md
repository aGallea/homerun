# Compact Mini-View & Menu Bar Tray — Design Spec

**Issue:** [#25](https://github.com/aGallea/homerun/issues/25) — Add compact mini-view for always-on-top runner monitoring
**Date:** 2026-03-27

## Overview

Two complementary features for at-a-glance runner monitoring without the full desktop app:

1. **Mini always-on-top window** — a small, transparent, draggable overlay showing runner status and active job progress
2. **macOS menu bar tray icon** — a system tray icon with status-based icons and a rich dropdown panel

The menu bar icon acts as the central control point for toggling between the main window and the mini view.

## Component 1: Mini Always-on-Top Window

### Content

- **Header row**: daemon health dot + "HOMERUN" label + runner status summary (e.g., "3 online / 2 busy / 1 off")
- **Busy runner cards**: one card per busy runner showing:
  - Runner name
  - Elapsed time
  - Current job name
  - Progress bar (based on estimated duration)
- No system metrics (CPU/memory) — kept minimal

### Window Behavior

- **Size**: ~280px wide, height adapts to content (number of busy runners)
- **Always on top**: stays above all other windows
- **Transparent**: frosted glass (vibrancy/blur) background, no standard window decorations
- **Draggable**: user can drag the window anywhere on screen
- **Position persistence**: remembers last position in user preferences; defaults to top-right corner on first launch
- **No title bar**: custom drag region on the header area

### Data Source

Reuses the existing `useRunners` hook (polls `/list_runners` every 2s). The mini window is a separate Tauri webview window pointing to a dedicated route (e.g., `/mini`), sharing the same daemon connection through Tauri IPC commands.

## Component 2: macOS Menu Bar Tray Icon

### Tray Icon States

Four custom icons in `assets/` — the "H" logo with different center indicators:

| State       | Icon File             | Condition                                          |
| ----------- | --------------------- | -------------------------------------------------- |
| **idle**    | `homerun_idle.png`    | Daemon running, all runners online, none busy      |
| **active**  | `homerun_active.png`  | Daemon running, at least one runner busy           |
| **error**   | `homerun_error.png`   | Daemon running, at least one runner in error state |
| **offline** | `homerun_offline.png` | Daemon not running or unreachable                  |

Priority: error > active > idle (if any runner has error, show error icon even if others are busy).

### Tray Dropdown (Webview Panel)

A custom webview popup (~300px wide) anchored below the tray icon. Not a native menu — a webview panel is required for rich content (progress bars, styled runner list).

**Sections (top to bottom):**

1. **Header**: daemon health dot + "HomeRun" + "Daemon running/stopped" status text
2. **Summary bar**: runner counts by state (e.g., "3 online / 2 busy / 1 offline")
3. **Runner list**:
   - Busy runners: status dot + name + elapsed time + job name + progress bar
   - Online/idle runners: status dot + name + "Idle" label
   - Offline runners: greyed out status dot + name + "Offline" label
4. **Actions**:
   - Toggle Mini View (⌘⇧M)
   - Open HomeRun (⌘⇧H)
   - Stop Daemon / Start Daemon (context-dependent)
   - Quit HomeRun (⌘Q)

### Tray Click Behavior

- **Left click**: opens/closes the dropdown panel
- **Dropdown dismiss**: closes when an action is clicked or when the panel loses focus (click outside)
- **Dropdown actions**: "Toggle Mini View" shows/hides the mini window; "Open HomeRun" shows/focuses the main window
- **Main and mini are mutually exclusive**: toggling mini view hides the main window and vice versa. Only one is visible at a time.

## Architecture

### Tauri Changes (Rust — `src-tauri/`)

**System tray setup** (`lib.rs`):

- Register a `SystemTray` with the idle icon on startup
- Handle tray events: left-click toggles the dropdown panel
- Add a Tauri command to update the tray icon based on runner state (called from the frontend polling loop)

**New Tauri commands** (`commands.rs`):

- `toggle_mini_window` — creates or shows/hides the mini window
- `update_tray_icon(state: String)` — swaps the tray icon (idle/active/error/offline)
- `toggle_tray_panel` — opens/closes the tray dropdown webview
- `save_mini_position(x: f64, y: f64)` / `get_mini_position() -> (f64, f64)` — persist mini window position in preferences

**Window configuration** (`tauri.conf.json`):

- Keep the existing `main` window config unchanged
- The mini window and tray panel are created programmatically at runtime (not in config) via `WebviewWindowBuilder`

### Frontend Changes (React — `src/`)

**New route `/mini`** — rendered in the mini window:

- Lightweight component using `useRunners` hook
- Shows header row with status counts + busy runner cards with progress
- Transparent background CSS (`background: transparent` on html/body)
- Custom drag region via Tauri's `data-tauri-drag-region`

**New route `/tray`** — rendered in the tray dropdown panel:

- Full runner list with status, jobs, progress
- Action buttons that invoke Tauri commands (toggle mini, open main, stop/start daemon, quit)
- Compact styling, no scrollbar unless runner list is long

**Tray icon state management**:

- A `useTrayIcon` hook (or logic within existing `useRunners`) computes the aggregate icon state from the runner list
- Calls `update_tray_icon` command whenever the state changes
- Priority: error > active > idle; offline when daemon is unreachable

### Preferences

Extend the existing preferences system (`get_preferences` / `update_preferences`) with:

- `mini_window_x: f64` — last X position (default: top-right)
- `mini_window_y: f64` — last Y position (default: top-right)

### Icon Assets

The four tray icons (`homerun_idle.png`, `homerun_active.png`, `homerun_error.png`, `homerun_offline.png`) are already generated in `assets/`. They need to be:

- Copied/referenced in `src-tauri/icons/` or bundled as Tauri resources
- Sized appropriately for macOS menu bar (template images, ideally 22x22 @1x / 44x44 @2x)

## Interactions & State Transitions

### Window Lifecycle

```
App launch → main window visible, tray icon active (idle/active/error based on runners)
              mini window hidden

User clicks tray → dropdown opens
  "Toggle Mini View" → mini window appears (top-right or last position)
                        main window hides
  "Open HomeRun" → main window shows/focuses
                    dropdown closes

Mini window visible → user drags → position saved to preferences
                   → user clicks tray "Toggle Mini View" → mini hides, main shows

Quit → all windows close, tray removed
```

### Tray Icon Update Loop

```
useRunners polls every 2s → compute aggregate state:
  if daemon unreachable → "offline"
  else if any runner.state == "error" → "error"
  else if any runner.state == "busy" → "active"
  else → "idle"
→ call update_tray_icon(state) only when state changes
```

## Out of Scope

- System metrics (CPU/memory) in the mini window
- Notifications / alerts from the tray icon
- Runner management actions from the mini window (it's read-only)
- Windows/Linux support (macOS only for now)
