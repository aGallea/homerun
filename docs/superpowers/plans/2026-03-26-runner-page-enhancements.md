# Runner Page Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add runner ID to the detail page header and show last job info in the runner list table when idle (#83).

**Architecture:** Two isolated frontend changes — one in RunnerDetail.tsx (breadcrumb area), one in RunnerTable.tsx (progress bar column). No backend changes. All data already available on `RunnerInfo`.

**Tech Stack:** React 19, TypeScript, inline styles (matching existing patterns)

---

## Task 1: Add runner ID to the detail page header

**Files:**

- Modify: `apps/desktop/src/pages/RunnerDetail.tsx:399-405`

- [ ] **Step 1: Add the runner ID span below the breadcrumb current name**

In `apps/desktop/src/pages/RunnerDetail.tsx`, find the breadcrumb section (around line 399-405):

```tsx
<div className="runner-detail-breadcrumbs">
  <Link to="/dashboard" className="breadcrumb-link">
    Runners
  </Link>
  <span className="breadcrumb-sep">›</span>
  <span className="breadcrumb-current">{config.name}</span>
</div>
```

Replace it with:

```tsx
<div className="runner-detail-breadcrumbs">
  <Link to="/dashboard" className="breadcrumb-link">
    Runners
  </Link>
  <span className="breadcrumb-sep">›</span>
  <span className="breadcrumb-current">{config.name}</span>
  <span
    title={config.id}
    style={{
      fontSize: 11,
      color: "var(--text-secondary)",
      marginLeft: 8,
      opacity: 0.7,
    }}
  >
    ID: {config.id.slice(0, 8)}
  </span>
</div>
```

- [ ] **Step 2: Verify the build passes**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/pages/RunnerDetail.tsx
git commit -m "feat: show runner ID in detail page header (#83)"
```

---

## Task 2: Show last job info in runner list table when idle

**Files:**

- Modify: `apps/desktop/src/components/RunnerTable.tsx:239-371`

- [ ] **Step 1: Add a `LastJobSummary` component after `MiniProgressBar` in RunnerTable.tsx**

In `apps/desktop/src/components/RunnerTable.tsx`, add this new component right after the `MiniProgressBar` component (after line 269):

```tsx
function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  if (m < 60) return s > 0 ? `${m}m ${s}s` : `${m}m`;
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`;
}

function LastJobSummary({ runner }: { runner: RunnerInfo }) {
  const job = runner.last_completed_job;
  if (!job) return null;

  const icon = job.succeeded ? "\u2713" : "\u2717";
  const iconColor = job.succeeded ? "var(--accent-green)" : "var(--accent-red)";

  const branchDisplay = job.branch
    ? job.branch.length > 20
      ? job.branch.slice(0, 20) + "\u2026"
      : job.branch
    : null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 4,
        fontSize: 11,
        color: "var(--text-secondary)",
        whiteSpace: "nowrap",
        overflow: "hidden",
      }}
      title={`Last job: ${job.job_name}${job.branch ? ` (${job.branch})` : ""} — ${job.succeeded ? "succeeded" : "failed"} in ${formatDuration(job.duration_secs)}`}
    >
      <span style={{ color: iconColor, fontWeight: 700, flexShrink: 0 }}>{icon}</span>
      {branchDisplay && (
        <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{branchDisplay}</span>
      )}
      {job.pr_number != null && <span style={{ flexShrink: 0 }}>#{job.pr_number}</span>}
      <span style={{ flexShrink: 0, opacity: 0.5 }}>&middot;</span>
      <span style={{ flexShrink: 0 }}>{formatDuration(job.duration_secs)}</span>
    </div>
  );
}
```

- [ ] **Step 2: Update `RunnerRow` to show `LastJobSummary` when not busy**

In the `RunnerRow` component, find the progress bar conditional block (around line 349-356):

```tsx
{
  runner.state === "busy" &&
    runner.estimated_job_duration_secs != null &&
    runner.job_started_at && (
      <MiniProgressBar
        estimatedDurationSecs={runner.estimated_job_duration_secs}
        jobStartedAt={runner.job_started_at}
      />
    );
}
```

Replace it with:

```tsx
{
  runner.state === "busy" && runner.estimated_job_duration_secs != null && runner.job_started_at ? (
    <MiniProgressBar
      estimatedDurationSecs={runner.estimated_job_duration_secs}
      jobStartedAt={runner.job_started_at}
    />
  ) : (
    runner.state !== "busy" && <LastJobSummary runner={runner} />
  );
}
```

- [ ] **Step 3: Verify the build passes**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/RunnerTable.tsx
git commit -m "feat: show last job info in runner list when idle (#83)"
```
