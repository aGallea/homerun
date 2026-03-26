# Runner Page Enhancements — Design Spec

**Issue:** #83
**Date:** 2026-03-26

## Overview

Two frontend-only enhancements to the desktop app's runner UI. No backend/API changes needed — all required data already exists on `RunnerInfo`.

## 1. Runner ID in Detail Page Title

**File:** `apps/desktop/src/pages/RunnerDetail.tsx`

**Current behavior:** Breadcrumbs show `Runners › {runner.config.name}`. No runner ID visible.

**New behavior:** Add the runner ID below or beside the runner name in the header area, displayed in a muted/secondary style. The UUID is truncated to 8 characters for readability, with the full ID shown on hover via a `title` attribute.

**Data source:** `runner.config.id` (already available in the component).

**Visual treatment:**

- Smaller font size, muted color (e.g., `text-xs text-zinc-500`)
- Format: `ID: abc12def`
- Full UUID on hover

## 2. Last Job Info in Runner List Table

**File:** `apps/desktop/src/components/RunnerTable.tsx`

**Current behavior:** The progress bar column shows a mini progress bar when a runner is busy, and is empty when idle.

**New behavior:** When a runner is **not busy** and has a `last_completed_job`, display a compact inline summary in the progress bar column area:

- Success/failure indicator: green checkmark (✓) for success, red X (✗) for failure
- Branch name (truncated if > ~20 chars) and/or PR number as `#N`
- Duration formatted as human-readable (e.g., `3m 12s`)
- Compact inline layout: `✓ main #42 · 3m 12s`

When busy: show progress bar as today (no change).
When idle with no `last_completed_job`: leave empty (no change).

**Data source:** `runner.last_completed_job` — a `CompletedJob` object with fields:

- `succeeded: boolean`
- `branch: string | null`
- `pr_number: number | null`
- `duration_secs: number`

## Scope

- Frontend only (React/TypeScript)
- Two files modified: `RunnerDetail.tsx`, `RunnerTable.tsx`
- No new API endpoints, no Rust changes
- No new dependencies
