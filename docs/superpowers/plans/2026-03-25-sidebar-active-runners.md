# Sidebar Active Runners Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show busy runners in the sidebar so users have visibility into active work from any page.

**Architecture:** New `ActiveRunners` component in the sidebar, fed by lifting `useRunners` to `Layout.tsx` and sharing it via Outlet context. No backend changes — all data already available.

**Tech Stack:** React 19, TypeScript, react-router-dom (useOutletContext), Vitest, CSS

---

## Task 0: Set up test infrastructure

**Files:**

- Modify: `apps/desktop/package.json` (add devDependencies)
- Create: `apps/desktop/vitest.config.ts`

The desktop app has no test runner or testing-library installed. This task adds the minimum test infrastructure.

- [ ] **Step 1: Install test dependencies**

```bash
cd apps/desktop && npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

- [ ] **Step 2: Create vitest config**

```typescript
// apps/desktop/vitest.config.ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: true,
  },
});
```

- [ ] **Step 3: Verify vitest runs**

Run: `cd apps/desktop && npx vitest run`
Expected: "No test files found" (no errors)

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/vitest.config.ts
git commit -m "chore: add vitest and testing-library for desktop app tests (#71)"
```

---

## Task 1: Create `formatElapsed` utility and test it

**Files:**

- Create: `apps/desktop/src/utils/formatElapsed.ts`
- Create: `apps/desktop/src/utils/formatElapsed.test.ts`

**Dependencies:** Task 0

- [ ] **Step 1: Write the test file**

```typescript
// apps/desktop/src/utils/formatElapsed.test.ts
import { describe, it, expect } from "vitest";
import { formatElapsed } from "./formatElapsed";

describe("formatElapsed", () => {
  it("returns '< 1m' for durations under 60 seconds", () => {
    expect(formatElapsed(0)).toBe("< 1m");
    expect(formatElapsed(30)).toBe("< 1m");
    expect(formatElapsed(59)).toBe("< 1m");
  });

  it("returns 'Xm' for durations under 60 minutes", () => {
    expect(formatElapsed(60)).toBe("1m");
    expect(formatElapsed(90)).toBe("1m");
    expect(formatElapsed(300)).toBe("5m");
    expect(formatElapsed(3540)).toBe("59m");
  });

  it("returns 'XhYm' for durations of 60 minutes or more", () => {
    expect(formatElapsed(3600)).toBe("1h0m");
    expect(formatElapsed(3660)).toBe("1h1m");
    expect(formatElapsed(7380)).toBe("2h3m");
  });

  it("returns '...' for null or negative input", () => {
    expect(formatElapsed(null)).toBe("...");
    expect(formatElapsed(undefined)).toBe("...");
    expect(formatElapsed(-1)).toBe("...");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/desktop && npx vitest run src/utils/formatElapsed.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Write the implementation**

```typescript
// apps/desktop/src/utils/formatElapsed.ts
export function formatElapsed(seconds: number | null | undefined): string {
  if (seconds == null || seconds < 0) return "...";
  if (seconds < 60) return "< 1m";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h${remainingMinutes}m`;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/desktop && npx vitest run src/utils/formatElapsed.test.ts`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/utils/formatElapsed.ts apps/desktop/src/utils/formatElapsed.test.ts
git commit -m "feat: add formatElapsed utility for sidebar active runners (#71)"
```

---

## Task 2: Create `ActiveRunners` component and test it

**Files:**

- Create: `apps/desktop/src/components/ActiveRunners.tsx`
- Create: `apps/desktop/src/components/ActiveRunners.test.tsx`

**Dependencies:** Task 1 (formatElapsed)

- [ ] **Step 1: Write the test file**

Tests use `vi.spyOn(Date, "now")` to make time-dependent assertions deterministic.

```typescript
// apps/desktop/src/components/ActiveRunners.test.tsx
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ActiveRunners } from "./ActiveRunners";
import type { RunnerInfo } from "../api/types";

const FIXED_NOW = new Date("2026-03-25T12:00:00Z").getTime();

function makeRunner(overrides: Partial<RunnerInfo> & { name: string }): RunnerInfo {
  return {
    config: {
      id: overrides.name,
      name: overrides.name,
      repo_owner: "org",
      repo_name: "repo",
      labels: [],
      mode: "service",
      work_dir: "/tmp",
    },
    state: overrides.state ?? "online",
    pid: 1234,
    uptime_secs: 100,
    jobs_completed: 0,
    jobs_failed: 0,
    current_job: overrides.current_job ?? null,
    job_started_at: overrides.job_started_at ?? null,
    ...overrides,
  };
}

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ActiveRunners", () => {
  it("renders nothing when no runners are busy", () => {
    const { container } = renderWithRouter(
      <ActiveRunners
        runners={[makeRunner({ name: "r1", state: "online" })]}
        collapsed={false}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders busy runners with name, job, and elapsed time", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const threeMinAgo = new Date(FIXED_NOW - 180_000).toISOString();
    renderWithRouter(
      <ActiveRunners
        runners={[
          makeRunner({
            name: "runner-1",
            state: "busy",
            current_job: "build-and-test",
            job_started_at: threeMinAgo,
          }),
        ]}
        collapsed={false}
      />,
    );
    expect(screen.getByText("runner-1")).toBeTruthy();
    expect(screen.getByText("build-and-test")).toBeTruthy();
    expect(screen.getByText("3m")).toBeTruthy();
  });

  it("shows 'Starting...' when current_job is null", () => {
    renderWithRouter(
      <ActiveRunners
        runners={[makeRunner({ name: "r1", state: "busy", current_job: null })]}
        collapsed={false}
      />,
    );
    expect(screen.getByText("Starting...")).toBeTruthy();
  });

  it("caps at 3 runners and shows overflow link", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const runners = Array.from({ length: 5 }, (_, i) =>
      makeRunner({
        name: `runner-${i}`,
        state: "busy",
        current_job: `job-${i}`,
        job_started_at: new Date(FIXED_NOW - i * 60_000).toISOString(),
      }),
    );
    renderWithRouter(<ActiveRunners runners={runners} collapsed={false} />);
    // 3 runner names visible
    expect(screen.getByText("runner-0")).toBeTruthy();
    expect(screen.getByText("runner-1")).toBeTruthy();
    expect(screen.getByText("runner-2")).toBeTruthy();
    // runner-3 and runner-4 not rendered
    expect(screen.queryByText("runner-3")).toBeNull();
    // overflow link
    expect(screen.getByText("+2 more runners")).toBeTruthy();
  });

  it("sorts by job_started_at descending (most recent first)", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const runners = [
      makeRunner({
        name: "old-runner",
        state: "busy",
        current_job: "j1",
        job_started_at: new Date(FIXED_NOW - 300_000).toISOString(),
      }),
      makeRunner({
        name: "new-runner",
        state: "busy",
        current_job: "j2",
        job_started_at: new Date(FIXED_NOW - 60_000).toISOString(),
      }),
    ];
    renderWithRouter(<ActiveRunners runners={runners} collapsed={false} />);
    const entries = document.querySelectorAll(".sidebar-active-entry");
    expect(entries[0].textContent).toContain("new-runner");
    expect(entries[1].textContent).toContain("old-runner");
  });

  it("shows only badge with count when collapsed", () => {
    renderWithRouter(
      <ActiveRunners
        runners={[
          makeRunner({ name: "r1", state: "busy", current_job: "j1" }),
          makeRunner({ name: "r2", state: "busy", current_job: "j2" }),
        ]}
        collapsed={true}
      />,
    );
    expect(screen.getByText("2")).toBeTruthy();
    // Runner names should NOT be visible
    expect(screen.queryByText("r1")).toBeNull();
    expect(screen.queryByText("r2")).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/desktop && npx vitest run src/components/ActiveRunners.test.tsx`
Expected: FAIL — module not found

- [ ] **Step 3: Write the component**

```tsx
// apps/desktop/src/components/ActiveRunners.tsx
import { Link } from "react-router-dom";
import type { RunnerInfo } from "../api/types";
import { formatElapsed } from "../utils/formatElapsed";

const MAX_VISIBLE = 3;

function elapsedSeconds(jobStartedAt: string | null | undefined): number | null {
  if (!jobStartedAt) return null;
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return null;
  return Math.floor((Date.now() - started) / 1000);
}

export function ActiveRunners({
  runners,
  collapsed,
}: {
  runners: RunnerInfo[];
  collapsed: boolean;
}) {
  const busy = runners
    .filter((r) => r.state === "busy")
    .sort((a, b) => {
      // Most recently started first; null job_started_at sorts last
      const aTime = a.job_started_at ? new Date(a.job_started_at).getTime() : 0;
      const bTime = b.job_started_at ? new Date(b.job_started_at).getTime() : 0;
      if (bTime !== aTime) return bTime - aTime;
      return a.config.name.localeCompare(b.config.name);
    });

  if (busy.length === 0) return null;

  if (collapsed) {
    return (
      <div className="sidebar-active-badge">
        <span className="sidebar-active-badge-count">{busy.length}</span>
      </div>
    );
  }

  const visible = busy.slice(0, MAX_VISIBLE);
  const overflow = busy.length - MAX_VISIBLE;

  return (
    <div className="sidebar-active">
      <div className="sidebar-active-header">
        <span className="sidebar-active-label">ACTIVE</span>
        <span className="sidebar-active-count">{busy.length}</span>
      </div>
      <div className="sidebar-active-list">
        {visible.map((runner) => (
          <Link
            key={runner.config.id}
            to={`/runners/${runner.config.id}`}
            className="sidebar-active-entry"
            title={`${runner.config.name} — ${runner.current_job ?? "Starting..."}`}
          >
            <span className="sidebar-active-dot" />
            <div className="sidebar-active-info">
              <span className="sidebar-active-name">{runner.config.name}</span>
              <span className="sidebar-active-job">
                {runner.current_job ?? <em>Starting...</em>}
              </span>
            </div>
            <span className="sidebar-active-time">
              {formatElapsed(elapsedSeconds(runner.job_started_at))}
            </span>
          </Link>
        ))}
        {overflow > 0 && (
          <Link to="/dashboard" className="sidebar-active-overflow">
            +{overflow} more runners
          </Link>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd apps/desktop && npx vitest run src/components/ActiveRunners.test.tsx`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/ActiveRunners.tsx apps/desktop/src/components/ActiveRunners.test.tsx
git commit -m "feat: add ActiveRunners component for sidebar (#71)"
```

---

## Task 3: Add CSS styles for `ActiveRunners`

**Files:**

- Modify: `apps/desktop/src/index.css` (after `.sidebar-username` block, around line 420)

**Note:** The sidebar is `display: flex; flex-direction: column` and `.sidebar-nav` has `flex: 1`, which already pushes everything after it to the bottom. The ActiveRunners section will naturally sit between the nav and footer with no extra spacing tricks needed.

- [ ] **Step 1: Add the CSS**

Insert after the `.sidebar-username` block (line ~420) in `apps/desktop/src/index.css`:

```css
/* Sidebar active runners */
.sidebar-active {
  border-top: 1px solid var(--border);
  padding: 8px;
}

.sidebar-active-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 6px;
  margin-bottom: 4px;
}

.sidebar-active-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: #484f58;
  font-weight: 600;
}

.sidebar-active-count {
  font-size: 10px;
  color: var(--accent-yellow);
  font-weight: 600;
}

.sidebar-active-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.sidebar-active-entry {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-radius: 5px;
  background: rgba(210, 153, 34, 0.08);
  text-decoration: none;
  color: inherit;
  transition: background 0.15s;
}

.sidebar-active-entry:hover {
  background: rgba(210, 153, 34, 0.15);
  text-decoration: none;
}

.sidebar-active-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent-yellow);
  flex-shrink: 0;
}

.sidebar-active-info {
  overflow: hidden;
  flex: 1;
  min-width: 0;
}

.sidebar-active-name {
  display: block;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-active-job {
  display: block;
  font-size: 10px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-active-time {
  font-size: 10px;
  color: #484f58;
  flex-shrink: 0;
}

.sidebar-active-overflow {
  display: block;
  padding: 4px 8px;
  text-align: center;
  font-size: 11px;
  color: var(--accent-blue);
  text-decoration: none;
}

.sidebar-active-overflow:hover {
  text-decoration: underline;
}

.sidebar-active-badge {
  display: flex;
  justify-content: center;
  padding: 8px 0;
}

.sidebar-active-badge-count {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--accent-yellow);
  color: #0d1117;
  font-size: 11px;
  font-weight: 700;
}

/* Collapsed sidebar: hide active section, only show badge */
.sidebar-collapsed .sidebar-active {
  display: none;
}
```

- [ ] **Step 2: Verify CSS parses correctly**

Run: `cd apps/desktop && npx prettier --check src/index.css`
Expected: PASS (or auto-format if needed)

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/index.css
git commit -m "style: add CSS for sidebar active runners section (#71)"
```

---

## Task 4: Export `RunnersContextType` from `useRunners`

**Files:**

- Modify: `apps/desktop/src/hooks/useRunners.ts`

This adds a named export type so consumers using `useOutletContext` can type-safely access the shared runners hook.

- [ ] **Step 1: Add the type export**

At the end of `apps/desktop/src/hooks/useRunners.ts` (after the closing `}` of `useRunners`), add:

```typescript
export type RunnersContextType = ReturnType<typeof useRunners>;
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/hooks/useRunners.ts
git commit -m "refactor: export RunnersContextType from useRunners (#71)"
```

---

## Task 5: Lift `useRunners` to Layout and wire up Outlet context

**Files:**

- Modify: `apps/desktop/src/components/Layout.tsx`
- Modify: `apps/desktop/src/components/Sidebar.tsx`
- Modify: `apps/desktop/src/pages/Dashboard.tsx`
- Modify: `apps/desktop/src/pages/RunnerDetail.tsx`
- Modify: `apps/desktop/src/pages/Repositories.tsx`

**Dependencies:** Tasks 2, 3, 4

This task has the most moving parts. The key changes:

1. `Layout.tsx` — call `useRunners()`, pass `runners` to `Sidebar`, pass full hook return via `<Outlet context={...} />`
2. `Sidebar.tsx` — accept `runners` prop, render `ActiveRunners` between nav and footer
3. `Dashboard.tsx` — replace `useRunners()` with `useOutletContext()`
4. `RunnerDetail.tsx` — replace `useRunners()` with `useOutletContext()`
5. `Repositories.tsx` — replace `useRunners()` with `useOutletContext()`

- [ ] **Step 1: Update `Layout.tsx`**

Replace the contents of `apps/desktop/src/components/Layout.tsx`:

```tsx
import { useState, useEffect } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { api } from "../api/commands";
import { useRunners } from "../hooks/useRunners";

export function Layout() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [daemonConnected, setDaemonConnected] = useState(true);
  const runnersHook = useRunners();

  useEffect(() => {
    let cancelled = false;
    async function check() {
      try {
        const ok = await api.healthCheck();
        if (!cancelled) setDaemonConnected(ok);
      } catch {
        if (!cancelled) setDaemonConnected(false);
      }
    }
    check();
    const timer = setInterval(check, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return (
    <div className="app">
      <div className="sidebar-wrapper">
        <Sidebar collapsed={sidebarCollapsed} runners={runnersHook.runners} />
        <button
          className="sidebar-fab"
          onClick={() => setSidebarCollapsed((c) => !c)}
          title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {sidebarCollapsed ? "›" : "‹"}
        </button>
      </div>
      <main className="main-content">
        {!daemonConnected && (
          <div
            className="error-banner"
            style={{
              margin: "16px 24px 0",
              padding: "12px 16px",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <span>Unable to connect to the HomeRun daemon.</span>
            <button
              className="btn btn-primary btn-sm"
              onClick={async () => {
                try {
                  await api.startDaemon();
                } catch (err) {
                  console.error("Failed to start daemon:", err);
                }
              }}
            >
              Start daemon
            </button>
          </div>
        )}
        <Outlet context={runnersHook} />
      </main>
    </div>
  );
}
```

- [ ] **Step 2: Update `Sidebar.tsx`**

Add `runners` prop and render `ActiveRunners` between the `sidebar-nav` div and `sidebar-footer` div in `apps/desktop/src/components/Sidebar.tsx`:

Change the function signature from:

```tsx
export function Sidebar({ collapsed }: { collapsed: boolean }) {
```

to:

```tsx
export function Sidebar({ collapsed, runners }: { collapsed: boolean; runners: RunnerInfo[] }) {
```

Add imports at the top:

```tsx
import type { RunnerInfo } from "../api/types";
import { ActiveRunners } from "./ActiveRunners";
```

Insert `<ActiveRunners runners={runners} collapsed={collapsed} />` between the closing `</div>` of `sidebar-nav` and the opening `<div className="sidebar-footer">` (between lines 114 and 115 in the current file):

```tsx
      </div>
      <ActiveRunners runners={runners} collapsed={collapsed} />
      <div className="sidebar-footer">
```

- [ ] **Step 3: Update `Dashboard.tsx`**

In `apps/desktop/src/pages/Dashboard.tsx`:

Replace the import:

```tsx
import { useRunners } from "../hooks/useRunners";
```

with:

```tsx
import { useOutletContext } from "react-router-dom";
import type { RunnersContextType } from "../hooks/useRunners";
```

(Merge `useOutletContext` into the existing `react-router-dom` import on line 2.)

Replace the `useRunners()` destructuring (lines 14-29):

```tsx
  } = useRunners();
```

with:

```tsx
  } = useOutletContext<RunnersContextType>();
```

- [ ] **Step 4: Update `RunnerDetail.tsx`**

In `apps/desktop/src/pages/RunnerDetail.tsx`:

Replace the import:

```tsx
import { useRunners } from "../hooks/useRunners";
```

with (merged into the existing react-router-dom import on line 2):

```tsx
import { useParams, useNavigate, Link, useOutletContext } from "react-router-dom";
import type { RunnersContextType } from "../hooks/useRunners";
```

Replace the `useRunners()` call (line 230):

```tsx
const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner } = useRunners();
```

with:

```tsx
const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner } =
  useOutletContext<RunnersContextType>();
```

- [ ] **Step 5: Update `Repositories.tsx`**

In `apps/desktop/src/pages/Repositories.tsx`:

Replace the import:

```tsx
import { useRunners } from "../hooks/useRunners";
```

with (merged into the existing react-router-dom import):

```tsx
import { useNavigate, useOutletContext } from "react-router-dom";
import type { RunnersContextType } from "../hooks/useRunners";
```

Replace the `useRunners()` call (line 12):

```tsx
const { runners, createRunner, createBatch } = useRunners();
```

with:

```tsx
const { runners, createRunner, createBatch } = useOutletContext<RunnersContextType>();
```

- [ ] **Step 6: Verify TypeScript compiles**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 7: Run all tests**

Run: `cd apps/desktop && npx vitest run`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/components/Layout.tsx apps/desktop/src/components/Sidebar.tsx apps/desktop/src/pages/Dashboard.tsx apps/desktop/src/pages/RunnerDetail.tsx apps/desktop/src/pages/Repositories.tsx apps/desktop/src/hooks/useRunners.ts
git commit -m "feat: wire ActiveRunners into sidebar via shared useRunners context (#71)"
```

---

## Task 6: Verify the full integration

**Files:** None (verification only)

- [ ] **Step 1: Run the type checker**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 2: Run all tests**

Run: `cd apps/desktop && npx vitest run`
Expected: All tests PASS

- [ ] **Step 3: Run prettier**

Run: `cd apps/desktop && npx prettier --write src/`
Expected: Files formatted (commit if any changes)

- [ ] **Step 4: Run the app visually (manual check)**

Run: `cd apps/desktop && npm run dev` (requires daemon running)
Verify:

- Sidebar shows "ACTIVE" section with busy runners when runners are executing jobs
- Section disappears when no runners are busy
- Clicking a runner entry navigates to `/runners/:id`
- "+N more" link navigates to `/dashboard`
- Collapsed sidebar shows yellow badge with count
- Long runner/job names truncate with ellipsis
- Hover shows full text in tooltip

- [ ] **Step 5: Commit any formatting fixes**

```bash
git add -u
git commit -m "style: format sidebar active runners code (#71)"
```
