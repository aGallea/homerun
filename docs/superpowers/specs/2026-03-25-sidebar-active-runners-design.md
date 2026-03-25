# Sidebar Active Runners

**Issue:** [#71 — Show active/busy runners in sidebar](https://github.com/aGallea/homerun/issues/71)
**Date:** 2026-03-25

## Overview

Add a section to the sidebar that lists currently busy runners, giving users visibility into active work regardless of which page they're on. The section appears only when at least one runner is busy and caps at 3 visible entries to avoid clutter.

## Design Decisions

| Question                              | Decision                                                        |
| ------------------------------------- | --------------------------------------------------------------- |
| Compact summary vs individual runners | Individual runners                                              |
| Info per runner                       | Name + job name + elapsed time                                  |
| Long text handling                    | text-overflow ellipsis + native tooltip on hover                |
| Overflow (>3 busy)                    | Show 3 most recent, "+N more" link to dashboard                 |
| Clickable?                            | Click runner → `/runners/{config.id}`; "+N more" → `/dashboard` |
| When visible?                         | Only when >= 1 runner has state "busy"                          |

## Components

### `ActiveRunners` component

New component rendered in `Sidebar.tsx` between the `sidebar-nav` div and the `sidebar-footer` div (outside both, as a sibling).

**Props:**

- `runners: RunnerInfo[]` — full runner list (component filters internally)
- `collapsed: boolean` — sidebar collapsed state

**Behavior:**

- Filters runners to `state === "busy"`
- If zero busy runners, renders nothing (returns `null`)
- Sorts by `job_started_at` descending (most recently started first); runners with null/undefined `job_started_at` sort last
- Secondary sort tiebreaker: `config.name` alphabetical
- Renders up to 3 runner entries
- If more than 3 busy, shows "+N more runners" link that navigates to `/dashboard`

**Each runner entry shows:**

- Yellow status dot (6px)
- Runner name (`config.name`) — truncated with ellipsis
- Job name (`current_job`) — truncated with ellipsis, smaller text. If `current_job` is null/undefined, show "Starting..." in italic
- Elapsed time since `job_started_at` — right-aligned, compact format (e.g., "3m", "1h2m"). If `job_started_at` is null, show "..." instead
- Full runner name + job name in `title` attribute for tooltip
- Entry is a `<a>` tag (or `<NavLink>`) navigating to `/runners/${runner.config.id}` for proper link semantics and keyboard navigation

**Collapsed sidebar (64px):**

- Hide the runner list entirely
- Show a small yellow badge with the busy count centered below the last nav icon
- Badge only visible when count > 0

### Data flow change

`useRunners` is currently called only in `Dashboard.tsx`. To give the sidebar access to runner data:

- Move `useRunners()` call from `Dashboard.tsx` to `Layout.tsx`
- Pass `runners` down to `Sidebar` as a prop
- Use React `useOutletContext` to pass the full `useRunners` return value to child pages via `<Outlet context={runnersHook} />`
- `Dashboard.tsx` calls `useOutletContext()` instead of `useRunners()` directly
- `RunnerDetail.tsx` also consumes the shared context instead of its own `useRunners()` call
- This avoids duplicate polling — one 2-second poll serves sidebar, dashboard, and detail pages

### Styling

**Active runners section:**

- Separated from nav by `border-top: 1px solid var(--border)` with `margin-top: auto` to push it toward the bottom
- Section header: "ACTIVE" label (10px, uppercase, letter-spacing: 0.5px) with count badge
- Runner entries: `background: rgba(210, 153, 34, 0.08)`, border-radius: 5px, padding: 6px 8px
- Yellow dot: `background: var(--accent-yellow)`, 6px diameter, border-radius: 50%
- Runner name: 11px, font-weight: 500, `var(--text-primary)`
- Job name: 10px, `var(--text-secondary)`
- Elapsed time: 10px, `color: #484f58` (matches existing muted text in sidebar footer)
- "+N more" link: 11px, `var(--accent-blue)`, centered, padding: 4px 8px

### Elapsed time formatting

Utility function to format seconds into compact time:

- < 60s → "< 1m"
- < 60m → "Xm" (e.g., "3m")
- > = 60m → "XhYm" (e.g., "1h2m")

Calculated from `job_started_at` (ISO timestamp) relative to current time. Re-renders on the existing 2-second `useRunners` poll cycle — no separate timer needed.

## Out of Scope

- No changes to the daemon or REST API — all data already available in `RunnerInfo`
- No new WebSocket events — the existing 2s poll provides sufficient freshness
- No runner actions from the sidebar — it's read-only with navigation
- No "all idle" state display — section simply disappears when no runners are busy
