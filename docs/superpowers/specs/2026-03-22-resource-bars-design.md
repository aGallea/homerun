# Resource Bars Design

**Date:** 2026-03-22

## Problem

The runner detail page shows CPU and memory as plain text ("0.0% CPU 53 MB MEM"). This makes it hard to gauge resource usage at a glance, and the 5-second polling interval feels sluggish.

## Solution

Replace the plain text with htop-style stacked horizontal bars and increase polling frequency.

### Resource Bars

Replace the `InfoCard` content for "Resources" on the runner detail page with two stacked horizontal bars:

- **Layout:** Each bar is a row: left label (CPU/MEM) | colored fill track | right value
- **CPU bar colors** (gradient based on usage):
  - 0-60%: green (`#22c55e`)
  - 60-80%: green-to-yellow gradient (`#22c55e` → `#eab308`)
  - 80-100%: yellow-to-red gradient (`#eab308` → `#ef4444`)
- **Memory bar color:** indigo/purple gradient (`#6366f1` → `#818cf8`), always
- **Bar height:** 16px with 3px border-radius
- **Values:** CPU as `X.X%`, memory as `N MB` or `N.N GB` (auto-scale)

### Polling Interval

Change `useMetrics` hook default poll interval from 5000ms to 2000ms.

## Scope

- Modify: `apps/desktop/src/pages/RunnerDetail.tsx` (Resources InfoCard section)
- Modify: `apps/desktop/src/hooks/useMetrics.ts` (poll interval)
- No daemon, API, type, TUI, or other page changes
