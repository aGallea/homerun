import { useState, useEffect, useRef } from "react";
import { useParams, useNavigate, Link, useOutletContext } from "react-router-dom";
import type { RunnersContextType } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { useAuth } from "../hooks/useAuth";
import { api } from "../api/commands";
import type { LogEntry } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { JobProgress } from "../components/JobProgress";
import { useJobSteps } from "../hooks/useJobSteps";
import { useJobHistory } from "../hooks/useJobHistory";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
  return `${Math.round(bytes / (1024 * 1024))} MB`;
}

function makeResizeHandler(
  setter: React.Dispatch<React.SetStateAction<number>>,
  min: number,
  max: number,
) {
  return (e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    let currentHeight: number | null = null;
    // Read initial height on first move
    const onMouseMove = (ev: MouseEvent) => {
      if (currentHeight === null) {
        // Get height from setter's current value via a trick
        setter((h) => {
          currentHeight = h;
          return h;
        });
      }
      if (currentHeight !== null) {
        const delta = ev.clientY - startY;
        const newH = Math.max(min, Math.min(max, currentHeight + delta));
        setter(newH);
      }
    };
    const onMouseUp = () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  };
}

function ResizeHandle({ onMouseDown }: { onMouseDown: (e: React.MouseEvent) => void }) {
  return (
    <div
      onMouseDown={onMouseDown}
      style={{
        position: "absolute",
        bottom: 0,
        left: 0,
        right: 0,
        height: 10,
        cursor: "ns-resize",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        opacity: 0.4,
        color: "var(--text-secondary)",
        userSelect: "none",
        transition: "opacity 0.15s",
      }}
      title="Drag to resize"
      onMouseEnter={(e) => (e.currentTarget.style.opacity = "0.8")}
      onMouseLeave={(e) => (e.currentTarget.style.opacity = "0.4")}
    >
      <svg width="24" height="6" viewBox="0 0 24 6">
        <line
          x1="8"
          y1="1"
          x2="16"
          y2="1"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
        <line
          x1="8"
          y1="4.5"
          x2="16"
          y2="4.5"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
      </svg>
    </div>
  );
}

function cpuColor(percent: number): string {
  if (percent <= 60) return "var(--accent-green)";
  if (percent <= 80) return "var(--accent-yellow)";
  return "#f97316";
}

function StatusPill({ state, currentJob }: { state: string; currentJob?: string | null }) {
  const colorMap: Record<string, { border: string; text: string; bg: string; glow: string }> = {
    online: {
      border: "var(--accent-green)",
      text: "var(--accent-green)",
      bg: "rgba(63, 185, 80, 0.1)",
      glow: "0 0 10px rgba(63, 185, 80, 0.5)",
    },
    busy: {
      border: "var(--accent-yellow)",
      text: "var(--accent-yellow)",
      bg: "rgba(210, 153, 34, 0.1)",
      glow: "0 0 10px rgba(210, 153, 34, 0.5)",
    },
    offline: {
      border: "var(--text-secondary)",
      text: "var(--text-secondary)",
      bg: "rgba(125, 133, 144, 0.1)",
      glow: "none",
    },
    error: {
      border: "var(--accent-red)",
      text: "var(--accent-red)",
      bg: "rgba(218, 54, 51, 0.1)",
      glow: "0 0 10px rgba(218, 54, 51, 0.5)",
    },
  };

  const c = colorMap[state] ?? colorMap.offline;
  const label =
    state === "busy" && currentJob
      ? `Busy: ${currentJob}`
      : state.charAt(0).toUpperCase() + state.slice(1);

  return (
    <div
      className="status-pill"
      style={{
        border: `1px solid ${c.border}`,
        color: c.text,
        background: c.bg,
        boxShadow: c.glow,
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: c.border,
          flexShrink: 0,
        }}
      />
      {label}
    </div>
  );
}

function JobProgressBar({
  estimatedDurationSecs,
  jobStartedAt,
}: {
  estimatedDurationSecs?: number;
  jobStartedAt?: string;
}) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, []);

  if (estimatedDurationSecs == null || jobStartedAt == null) {
    return (
      <div className="glow-bar-track">
        <div className="glow-bar-fill-indeterminate" />
      </div>
    );
  }

  const elapsedSecs = (Date.now() - new Date(jobStartedAt).getTime()) / 1000;
  const progress = Math.min(elapsedSecs / estimatedDurationSecs, 0.99);
  const percent = Math.round(progress * 100);
  const exceeding = elapsedSecs > estimatedDurationSecs;
  const significantlyExceeding = elapsedSecs > estimatedDurationSecs + 5;

  return (
    <div>
      <div className="glow-bar-track">
        <div
          className="glow-bar-fill"
          style={{
            width: `${percent}%`,
            background: exceeding ? "var(--accent-yellow)" : "var(--accent-blue)",
            boxShadow: exceeding
              ? "0 0 8px rgba(210, 153, 34, 0.8)"
              : "0 0 8px rgba(59, 130, 246, 0.8)",
          }}
        />
      </div>
      {significantlyExceeding && (
        <span
          style={{ fontSize: 11, color: "var(--accent-yellow)", marginTop: 2, display: "block" }}
        >
          taking longer than usual
        </span>
      )}
    </div>
  );
}

export function RunnerDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { auth, handleUnauthorized } = useAuth();
  const isAuthenticated = auth.authenticated;
  const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner } =
    useOutletContext<RunnersContextType>();
  const { metrics } = useMetrics();
  const runner = runners.find((r) => r.config.id === id);
  const { steps, stepsDiscovered, jobName, expandedStep, stepLogs, toggleStep } = useJobSteps(
    id,
    runner?.state === "busy",
  );
  const { history, refresh: refreshHistory } = useJobHistory(id);
  const [expandedHistoryIndices, setExpandedHistoryIndices] = useState<Set<number>>(new Set());
  const expandedHistoryRef = useRef<HTMLDivElement | null>(null);
  const [deletingHistoryEntries, setDeletingHistoryEntries] = useState<Set<string>>(new Set());
  // Tracks entries where rerun is in-flight (spinner) or queued (queued icon)
  const [rerunningEntries, setRerunningEntries] = useState<Map<string, "loading" | "queued">>(
    new Map(),
  );
  const rerunTimers = useRef<Map<string, number>>(new Map());
  const [clearingHistory, setClearingHistory] = useState(false);

  // Poll queued entries every 10s to verify they're still queued.
  // Clears the entry if the run is no longer queued (started, completed, or error).
  useEffect(() => {
    const queuedEntries = [...rerunningEntries.entries()].filter(([, s]) => s === "queued");
    if (queuedEntries.length === 0 || !id) return;

    const interval = setInterval(() => {
      for (const [key] of queuedEntries) {
        // Find the history entry to get the run_url
        const entry = history.find((e) => e.started_at === key);
        if (!entry?.run_url) continue;
        api
          .getRunStatus(id!, entry.run_url)
          .then((res) => {
            // "queued" or "waiting" means still in queue — keep showing queued
            if (res.status === "queued" || res.status === "waiting") return;
            // Any other status (in_progress, completed) — clear it
            setRerunningEntries((m) => {
              const next = new Map(m);
              next.delete(key);
              return next;
            });
          })
          .catch(() => {
            // API error — clear to avoid stuck state
            setRerunningEntries((m) => {
              const next = new Map(m);
              next.delete(key);
              return next;
            });
          });
      }
    }, 10_000);

    return () => clearInterval(interval);
  }, [rerunningEntries, id, history]);

  // Cleanup all timers on unmount
  useEffect(() => {
    return () => {
      for (const t of rerunTimers.current.values()) clearTimeout(t);
    };
  }, []);

  useEffect(() => {
    if (expandedHistoryIndices.size > 0 && expandedHistoryRef.current) {
      setTimeout(
        () => expandedHistoryRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" }),
        50,
      );
    }
  }, [expandedHistoryIndices]);

  const [logsHeight, setLogsHeight] = useState(140);
  const [logsCollapsed, setLogsCollapsed] = useState(false);
  const [stepsHeight, setStepsHeight] = useState(300);
  const [stepsCollapsed, setStepsCollapsed] = useState(false);
  const [historyHeight, setHistoryHeight] = useState(200);
  const [historyCollapsed, setHistoryCollapsed] = useState(false);

  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [followLogs, setFollowLogs] = useState(true);
  const logContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!id) return;
    async function fetchLogs() {
      try {
        const entries = await api.getRunnerLogs(id!);
        setLogs(entries);
      } catch {
        // ignore errors (runner may be offline)
      }
    }
    fetchLogs();
    const timer = setInterval(fetchLogs, 2000);
    return () => clearInterval(timer);
  }, [id]);

  useEffect(() => {
    if (followLogs && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, followLogs]);

  const runnerMetrics = metrics?.runners.find((m) => m.runner_id === id);

  if (loading) {
    return (
      <div className="page">
        <p className="text-muted">Loading...</p>
      </div>
    );
  }

  if (!runner) {
    return (
      <div className="page">
        <div className="page-header">
          <button className="btn" onClick={() => navigate("/dashboard")}>
            ← Back to Runners
          </button>
        </div>
        <p className="text-muted">Runner not found.</p>
      </div>
    );
  }

  const { config, state, uptime_secs, jobs_completed, jobs_failed, current_job, job_context } =
    runner;
  const isRunning = state === "online" || state === "busy";
  const isStopped = state === "offline" || state === "error";
  const isTransient =
    state === "creating" || state === "registering" || state === "stopping" || state === "deleting";
  const canRestart = isRunning || isStopped;
  const canDelete = !isTransient && state !== "busy";

  async function doAction(fn: () => Promise<void>) {
    if (deleting) return;
    setActionError(null);
    try {
      await fn();
    } catch (e) {
      const msg = String(e);
      setActionError(msg);
      if (msg.includes("401")) handleUnauthorized();
    }
  }

  async function handleDelete() {
    setConfirmDelete(false);
    setDeleting(true);
    try {
      await deleteRunner(config.id);
      navigate("/dashboard");
    } catch (e) {
      setDeleting(false);
      setActionError(String(e));
    }
  }

  const cpuPercent = runnerMetrics?.cpu_percent ?? 0;

  return (
    <div className="runner-detail-page">
      {/* Top bar: breadcrumbs + status + uptime */}
      <header className="runner-detail-header">
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
        <div className="flex items-center gap-16">
          <StatusPill state={state} currentJob={current_job} />
          {uptime_secs != null && (
            <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              Uptime:{" "}
              <span style={{ color: "var(--text-primary)", fontWeight: 500 }}>
                {formatUptime(uptime_secs)}
              </span>
            </span>
          )}
        </div>
      </header>

      {/* Content area */}
      <div className="runner-detail-content">
        {actionError && <div className="error-banner">{actionError}</div>}
        {state === "error" && runner.error_message && !actionError && (
          <div className="error-banner">{runner.error_message}</div>
        )}
        {/* Top section: current job card on left, actions + stats on right */}
        <div style={{ display: "flex", flexDirection: "row-reverse", gap: 16, marginBottom: 12 }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            {/* Action buttons */}
            {isAuthenticated && (
              <div className="flex items-center gap-8" style={{ marginBottom: 16 }}>
                {(isTransient || deleting) && (
                  <span
                    style={{
                      display: "inline-block",
                      width: 16,
                      height: 16,
                      border: "2px solid var(--border)",
                      borderTopColor: "var(--text-primary)",
                      borderRadius: "50%",
                      animation: "spin 0.6s linear infinite",
                    }}
                  />
                )}
                {deleting && (
                  <span className="text-muted" style={{ fontSize: 13 }}>
                    Deleting…
                  </span>
                )}
                {isStopped && (
                  <button
                    className="btn btn-primary"
                    onClick={() => doAction(() => startRunner(config.id))}
                    disabled={deleting}
                  >
                    ▶ Start
                  </button>
                )}
                {isRunning && (
                  <button
                    className="runner-action-btn"
                    onClick={() => doAction(() => stopRunner(config.id))}
                    disabled={deleting}
                  >
                    ■ Stop
                  </button>
                )}
                <button
                  className="runner-action-btn"
                  onClick={() => doAction(() => restartRunner(config.id))}
                  disabled={!canRestart || deleting}
                >
                  ↺ Restart
                </button>
                <button
                  className="runner-action-btn runner-action-btn-danger"
                  onClick={() => setConfirmDelete(true)}
                  disabled={!canDelete || deleting}
                >
                  Delete
                </button>
              </div>
            )}

            {/* Compact stats row */}
            <div style={{ display: "flex", gap: 12, flexWrap: "wrap", marginBottom: 12 }}>
              <div className="flex items-center gap-8" style={{ fontSize: 13 }}>
                <span
                  className="job-stat-icon job-stat-icon-green"
                  style={{ width: 20, height: 20, fontSize: 11 }}
                >
                  ✓
                </span>
                <span style={{ fontWeight: 600, color: "var(--accent-green)" }}>
                  {jobs_completed}
                </span>
                <span style={{ color: "var(--text-secondary)" }}>passed</span>
              </div>
              <div className="flex items-center gap-8" style={{ fontSize: 13 }}>
                <span
                  className="job-stat-icon job-stat-icon-red"
                  style={{ width: 20, height: 20, fontSize: 11 }}
                >
                  ✕
                </span>
                <span
                  style={{
                    fontWeight: 600,
                    color: jobs_failed > 0 ? "var(--accent-red)" : "var(--text-secondary)",
                  }}
                >
                  {jobs_failed}
                </span>
                <span style={{ color: "var(--text-secondary)" }}>failed</span>
              </div>
              {runnerMetrics && (
                <>
                  <span style={{ color: "var(--border)" }}>|</span>
                  <div className="flex items-center gap-4" style={{ fontSize: 13 }}>
                    <span style={{ color: "var(--text-secondary)" }}>CPU</span>
                    <span
                      className="font-mono"
                      style={{ fontWeight: 600, color: cpuColor(cpuPercent) }}
                    >
                      {cpuPercent.toFixed(1)}%
                    </span>
                  </div>
                  <div className="flex items-center gap-4" style={{ fontSize: 13 }}>
                    <span style={{ color: "var(--text-secondary)" }}>MEM</span>
                    <span
                      className="font-mono"
                      style={{ fontWeight: 600, color: "var(--accent-blue)" }}
                    >
                      {formatBytes(runnerMetrics.memory_bytes)}
                    </span>
                  </div>
                </>
              )}
            </div>
          </div>
          {/* Right: Current Job card */}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="runner-card runner-card-job" style={{ overflow: "hidden" }}>
              <div className="runner-card-glow runner-card-glow-blue" />
              <div className="flex items-center justify-between">
                <h3 className="runner-card-label">{current_job ? "Current Job" : "Last Job"}</h3>
                {current_job && (
                  <a
                    href="#"
                    onClick={(e) => {
                      e.preventDefault();
                      let url = `https://github.com/${config.repo_owner}/${config.repo_name}/actions`;
                      if (job_context?.run_url) {
                        url =
                          job_context.job_id != null
                            ? `${job_context.run_url}/job/${job_context.job_id}`
                            : job_context.run_url;
                      }
                      import("@tauri-apps/plugin-shell").then(({ open }) => open(url));
                    }}
                    style={{ fontSize: 12, color: "var(--accent-blue)", whiteSpace: "nowrap" }}
                  >
                    View →
                  </a>
                )}
              </div>
              {current_job ? (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  <span
                    style={{
                      fontSize: 15,
                      fontWeight: 500,
                      color: "var(--text-primary)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                    title={current_job}
                  >
                    {current_job}
                  </span>
                  {job_context && (
                    <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                      Branch:{" "}
                      <span style={{ color: "var(--text-primary)" }}>{job_context.branch}</span>
                      {job_context.pr_number != null && (
                        <a
                          href="#"
                          onClick={(e) => {
                            e.preventDefault();
                            if (job_context.pr_url) {
                              import("@tauri-apps/plugin-shell").then(({ open }) =>
                                open(job_context.pr_url!),
                              );
                            }
                          }}
                          style={{ color: "var(--accent-blue)", marginLeft: 8 }}
                        >
                          PR #{job_context.pr_number}
                        </a>
                      )}
                    </div>
                  )}
                  <JobProgressBar
                    estimatedDurationSecs={runner.estimated_job_duration_secs ?? undefined}
                    jobStartedAt={runner.job_started_at ?? undefined}
                  />
                </div>
              ) : runner.last_completed_job ? (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  <div className="flex items-center gap-8">
                    <span
                      style={{
                        fontSize: 15,
                        fontWeight: 500,
                        color: "var(--text-primary)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        flex: 1,
                      }}
                      title={runner.last_completed_job.job_name}
                    >
                      {runner.last_completed_job.job_name}
                    </span>
                    <span
                      style={{
                        fontSize: 11,
                        fontWeight: 600,
                        padding: "2px 8px",
                        borderRadius: 4,
                        background: runner.last_completed_job.succeeded
                          ? "rgba(63, 185, 80, 0.15)"
                          : "rgba(218, 54, 51, 0.15)",
                        color: runner.last_completed_job.succeeded
                          ? "var(--accent-green)"
                          : "var(--accent-red)",
                        flexShrink: 0,
                      }}
                    >
                      {runner.last_completed_job.succeeded ? "Succeeded" : "Failed"}
                    </span>
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    {formatUptime(runner.last_completed_job.duration_secs)}
                    {runner.last_completed_job.branch && (
                      <>
                        {" · "}
                        <span style={{ color: "var(--text-primary)" }}>
                          {runner.last_completed_job.branch}
                        </span>
                      </>
                    )}
                    {runner.last_completed_job.pr_number != null && (
                      <span style={{ color: "var(--accent-blue)", marginLeft: 4 }}>
                        PR #{runner.last_completed_job.pr_number}
                      </span>
                    )}
                  </div>
                  {!runner.last_completed_job.succeeded &&
                    runner.last_completed_job.error_message && (
                      <div
                        style={{
                          fontSize: 11,
                          color: "var(--accent-red)",
                          opacity: 0.8,
                        }}
                      >
                        {runner.last_completed_job.error_message}
                      </div>
                    )}
                  {runner.last_completed_job.run_url && (
                    <a
                      href="#"
                      onClick={(e) => {
                        e.preventDefault();
                        import("@tauri-apps/plugin-shell").then(({ open }) =>
                          open(runner.last_completed_job!.run_url!),
                        );
                      }}
                      style={{ fontSize: 12, color: "var(--accent-blue)" }}
                    >
                      View on GitHub →
                    </a>
                  )}
                </div>
              ) : (
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    import("@tauri-apps/plugin-shell").then(({ open }) => {
                      open(`https://github.com/${config.repo_owner}/${config.repo_name}/actions`);
                    });
                  }}
                  style={{ color: "var(--accent-blue)", fontSize: 13 }}
                >
                  View Actions on GitHub →
                </a>
              )}
            </div>
          </div>
        </div>

        {/* Runner Process Logs — full width */}
        <div
          className="logs-panel"
          style={{
            display: "flex",
            flexDirection: "column",
            position: "relative",
            height: logsCollapsed ? "auto" : logsHeight,
            flex: "none",
          }}
        >
          <div
            className="logs-header"
            style={{ cursor: "pointer" }}
            onClick={() => setLogsCollapsed((c) => !c)}
          >
            <div className="flex items-center gap-8">
              <span
                style={{
                  fontSize: 16,
                  lineHeight: 1,
                  position: "relative" as const,
                  top: -1,
                  color: "var(--text-secondary)",
                  flexShrink: 0,
                }}
              >
                {logsCollapsed ? "\u25B8" : "\u25BE"}
              </span>
              <span className="runner-card-label" style={{ margin: 0, fontSize: 11 }}>
                Runner Process Logs
              </span>
            </div>
            {!logsCollapsed && (
              <label className="follow-toggle" onClick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  checked={followLogs}
                  onChange={(e) => setFollowLogs(e.target.checked)}
                />
                <span className="follow-toggle-track">
                  <span className="follow-toggle-thumb" />
                </span>
                <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>Follow</span>
              </label>
            )}
          </div>
          {!logsCollapsed && (
            <>
              <div
                ref={logContainerRef}
                className="logs-content font-mono"
                style={{ flex: 1, minHeight: 0, overflow: "auto" }}
              >
                {logs.length === 0 ? (
                  <div className="logs-empty">
                    {runner.state === "online" || runner.state === "busy"
                      ? "Waiting for log output..."
                      : "Runner is not active."}
                  </div>
                ) : (
                  <table className="logs-table">
                    <tbody>
                      {logs.map((entry, i) => (
                        <tr key={i}>
                          <td
                            style={{
                              color:
                                entry.stream === "stderr"
                                  ? "var(--accent-red)"
                                  : "var(--text-primary)",
                            }}
                          >
                            {entry.line}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
              <ResizeHandle onMouseDown={makeResizeHandler(setLogsHeight, 150, 600)} />
            </>
          )}
        </div>

        {/* Job Progress — full width, only when busy */}
        {state === "busy" && steps.length > 0 && (
          <JobProgress
            steps={steps}
            stepsDiscovered={stepsDiscovered}
            jobName={jobName}
            expandedStep={expandedStep}
            stepLogs={stepLogs}
            onToggleStep={toggleStep}
            height={stepsHeight}
            resizeHandle={
              <ResizeHandle onMouseDown={makeResizeHandler(setStepsHeight, 150, 600)} />
            }
            collapsed={stepsCollapsed}
            onToggleCollapsed={() => setStepsCollapsed((c) => !c)}
          />
        )}

        {/* Job History */}
        {history.length > 0 && (
          <div
            className="logs-panel"
            style={{
              display: "flex",
              flexDirection: "column",
              position: "relative",
              height: historyCollapsed ? "auto" : historyHeight,
              maxHeight: historyCollapsed ? undefined : "calc(100vh - 300px)",
              flex: "none",
            }}
          >
            <div
              className="logs-header"
              style={{ cursor: "pointer" }}
              onClick={() => setHistoryCollapsed((c) => !c)}
            >
              <div className="flex items-center gap-8">
                <span
                  style={{
                    fontSize: 16,
                    lineHeight: 1,
                    color: "var(--text-secondary)",
                    flexShrink: 0,
                  }}
                >
                  {historyCollapsed ? "\u25B8" : "\u25BE"}
                </span>
                <span className="runner-card-label" style={{ margin: 0, fontSize: 11 }}>
                  Job History
                </span>
                <span
                  style={{
                    fontSize: 12,
                    padding: "2px 8px",
                    borderRadius: 10,
                    background: "var(--bg-tertiary)",
                    color: "var(--text-secondary)",
                    fontWeight: 500,
                  }}
                >
                  {history.length} {history.length === 1 ? "job" : "jobs"}
                </span>
              </div>
              {!historyCollapsed && (
                <button
                  disabled={clearingHistory || deleting}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (deleting) return;
                    setClearingHistory(true);
                    api
                      .clearRunnerHistory(id!)
                      .then(() => {
                        setExpandedHistoryIndices(new Set());
                        return refreshHistory();
                      })
                      .finally(() => setClearingHistory(false));
                  }}
                  style={{
                    fontSize: 11,
                    color: "var(--text-secondary)",
                    background: "none",
                    border: "none",
                    cursor: clearingHistory ? "default" : "pointer",
                    padding: "2px 6px",
                    marginLeft: "auto",
                    opacity: clearingHistory ? 0.5 : 1,
                  }}
                  title="Clear all history"
                >
                  {clearingHistory ? "Clearing..." : "Clear all"}
                </button>
              )}
            </div>
            {!historyCollapsed && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 1,
                  overflow: "auto",
                  flex: 1,
                  minHeight: 0,
                }}
              >
                {history.map((entry, i) => {
                  const duration = Math.round(
                    (new Date(entry.completed_at).getTime() -
                      new Date(entry.started_at).getTime()) /
                      1000,
                  );
                  const isExpanded = expandedHistoryIndices.has(i);
                  const hasSteps = entry.steps && entry.steps.length > 0;
                  const isDeleting =
                    deleting || clearingHistory || deletingHistoryEntries.has(entry.started_at);
                  return (
                    <div key={i} ref={isExpanded ? expandedHistoryRef : undefined}>
                      <div
                        onClick={() => {
                          if (hasSteps) {
                            setExpandedHistoryIndices((prev) => {
                              const next = new Set(prev);
                              if (isExpanded) next.delete(i);
                              else next.add(i);
                              return next;
                            });
                          }
                        }}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 12,
                          padding: "6px 12px",
                          background: i % 2 === 0 ? "var(--bg-secondary)" : "var(--bg-primary)",
                          fontSize: 13,
                          cursor: hasSteps && !isDeleting ? "pointer" : "default",
                          opacity: isDeleting ? 0.4 : 1,
                          pointerEvents: isDeleting ? "none" : "auto",
                          transition: "opacity 0.2s",
                        }}
                      >
                        <span
                          style={{
                            width: 10,
                            height: 10,
                            borderRadius: "50%",
                            background: entry.succeeded
                              ? "var(--accent-green)"
                              : "var(--accent-red)",
                            flexShrink: 0,
                          }}
                        />
                        {hasSteps && (
                          <span
                            style={{
                              fontSize: 16,
                              lineHeight: 1,
                              color: "var(--text-secondary)",
                              flexShrink: 0,
                            }}
                          >
                            {isExpanded ? "\u25BE" : "\u25B8"}
                          </span>
                        )}
                        <div style={{ flex: 1, minWidth: 0 }}>
                          {entry.latest_attempt && (
                            <span
                              style={{
                                display: "inline-block",
                                fontSize: 10,
                                fontWeight: 600,
                                padding: "1px 6px",
                                borderRadius: 3,
                                lineHeight: "16px",
                                background: entry.latest_attempt.succeeded
                                  ? "rgba(34, 197, 94, 0.15)"
                                  : "rgba(239, 68, 68, 0.15)",
                                color: entry.latest_attempt.succeeded
                                  ? "var(--accent-green)"
                                  : "var(--accent-red)",
                                whiteSpace: "nowrap",
                              }}
                              title={`Re-run on ${entry.latest_attempt.runner_name}: ${entry.latest_attempt.succeeded ? "succeeded" : "failed"}`}
                            >
                              Re-run: {entry.latest_attempt.succeeded ? "\u2713" : "\u2717"}{" "}
                              {entry.latest_attempt.runner_name}
                            </span>
                          )}
                          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                            <span
                              style={{
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                whiteSpace: "nowrap",
                                color: "var(--text-primary)",
                                fontWeight: 500,
                              }}
                              title={entry.job_name}
                            >
                              {entry.job_name}
                            </span>
                          </div>
                          <div
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: 6,
                              fontSize: 11,
                              color: "var(--text-secondary)",
                              marginTop: entry.latest_attempt ? 0 : 2,
                            }}
                          >
                            {entry.branch && <span>{entry.branch}</span>}
                            {entry.pr_number != null && (
                              <span style={{ color: "var(--accent-blue)" }}>
                                #{entry.pr_number}
                              </span>
                            )}
                            {(entry.branch || entry.pr_number != null) && <span>·</span>}
                            <span>{new Date(entry.completed_at).toLocaleTimeString()}</span>
                            {!entry.succeeded && entry.error_message && (
                              <span
                                style={{
                                  color: "var(--accent-red)",
                                  opacity: 0.8,
                                  overflow: "hidden",
                                  textOverflow: "ellipsis",
                                  whiteSpace: "nowrap",
                                }}
                                title={entry.error_message}
                              >
                                · {entry.error_message}
                              </span>
                            )}
                          </div>
                        </div>
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 8,
                            flexShrink: 0,
                          }}
                        >
                          <span
                            className="font-mono"
                            style={{
                              fontSize: 11,
                              color: "var(--text-secondary)",
                              display: "flex",
                              alignItems: "center",
                              gap: 3,
                            }}
                          >
                            <svg
                              width="11"
                              height="11"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <circle cx="12" cy="12" r="10" />
                              <polyline points="12 6 12 12 16 14" />
                            </svg>
                            {formatUptime(duration)}
                          </span>
                          {entry.run_url && (
                            <a
                              href="#"
                              onClick={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                import("@tauri-apps/plugin-shell").then(({ open }) =>
                                  open(entry.run_url!),
                                );
                              }}
                              style={{ color: "var(--accent-blue)", display: "flex" }}
                              title="View on GitHub"
                            >
                              <svg
                                width="13"
                                height="13"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="2"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                                <polyline points="15 3 21 3 21 9" />
                                <line x1="10" y1="14" x2="21" y2="3" />
                              </svg>
                            </a>
                          )}
                          {entry.run_url &&
                            id &&
                            (() => {
                              const rerunState = rerunningEntries.get(entry.started_at);
                              return rerunState === "queued" ? (
                                <span
                                  style={{
                                    color: "var(--accent-yellow, #facc15)",
                                    display: "flex",
                                  }}
                                  title="Queued"
                                >
                                  <svg
                                    width="13"
                                    height="13"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    strokeWidth="2"
                                    strokeLinecap="round"
                                    strokeLinejoin="round"
                                  >
                                    <circle cx="12" cy="12" r="10" />
                                    <polyline points="12 6 12 12 16 14" />
                                  </svg>
                                </span>
                              ) : (
                                <a
                                  href="#"
                                  onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    if (rerunState) return;
                                    const key = entry.started_at;
                                    setRerunningEntries((m) => new Map(m).set(key, "loading"));
                                    api
                                      .rerunWorkflow(id!, entry.run_url!)
                                      .then(() => {
                                        setRerunningEntries((m) => new Map(m).set(key, "queued"));
                                      })
                                      .catch(() => {
                                        setRerunningEntries((m) => {
                                          const next = new Map(m);
                                          next.delete(key);
                                          return next;
                                        });
                                      });
                                  }}
                                  style={{
                                    color: "var(--text-secondary)",
                                    display: "flex",
                                    pointerEvents: rerunState ? "none" : undefined,
                                  }}
                                  title="Re-run"
                                >
                                  {rerunState === "loading" ? (
                                    <svg
                                      width="13"
                                      height="13"
                                      viewBox="0 0 24 24"
                                      fill="none"
                                      stroke="currentColor"
                                      strokeWidth="2.5"
                                      strokeLinecap="round"
                                      style={{ animation: "spin 1s linear infinite" }}
                                    >
                                      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
                                    </svg>
                                  ) : (
                                    <svg
                                      width="13"
                                      height="13"
                                      viewBox="0 0 24 24"
                                      fill="none"
                                      stroke="currentColor"
                                      strokeWidth="2"
                                      strokeLinecap="round"
                                      strokeLinejoin="round"
                                    >
                                      <polyline points="23 4 23 10 17 10" />
                                      <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
                                    </svg>
                                  )}
                                </a>
                              );
                            })()}
                          <a
                            href="#"
                            onClick={(e) => {
                              e.preventDefault();
                              e.stopPropagation();
                              const key = entry.started_at;
                              setDeletingHistoryEntries((s) => new Set(s).add(key));
                              api
                                .deleteHistoryEntry(id!, key)
                                .then(() => refreshHistory())
                                .finally(() =>
                                  setDeletingHistoryEntries((s) => {
                                    const next = new Set(s);
                                    next.delete(key);
                                    return next;
                                  }),
                                );
                            }}
                            style={{
                              color: "var(--text-secondary)",
                              display: "flex",
                              opacity: 0.5,
                            }}
                            title="Delete entry"
                          >
                            <svg
                              width="13"
                              height="13"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="3 6 5 6 21 6" />
                              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                            </svg>
                          </a>
                        </div>
                      </div>
                      {isExpanded && hasSteps && (
                        <div
                          style={{
                            background: "var(--bg-primary)",
                            borderTop: "1px solid var(--border)",
                            borderBottom: "1px solid var(--border)",
                            padding: "4px 0",
                          }}
                        >
                          {entry.steps.map((step) => {
                            const stepDuration =
                              step.started_at && step.completed_at
                                ? Math.max(
                                    0,
                                    Math.round(
                                      (new Date(step.completed_at).getTime() -
                                        new Date(step.started_at).getTime()) /
                                        1000,
                                    ),
                                  )
                                : null;
                            return (
                              <div
                                key={step.number}
                                style={{
                                  display: "flex",
                                  alignItems: "center",
                                  gap: 10,
                                  padding: "4px 16px 4px 48px",
                                  fontSize: 12,
                                }}
                              >
                                <span
                                  style={{
                                    width: 16,
                                    textAlign: "center",
                                    flexShrink: 0,
                                    fontSize: 13,
                                    fontWeight: 700,
                                    color:
                                      step.status === "succeeded"
                                        ? "var(--accent-green)"
                                        : step.status === "failed"
                                          ? "var(--accent-red)"
                                          : "var(--text-secondary)",
                                  }}
                                >
                                  {step.status === "succeeded"
                                    ? "\u2713"
                                    : step.status === "failed"
                                      ? "\u2715"
                                      : step.status === "skipped"
                                        ? "\u2298"
                                        : "\u25CB"}
                                </span>
                                <span
                                  style={{
                                    flex: 1,
                                    color: "var(--text-primary)",
                                    overflow: "hidden",
                                    textOverflow: "ellipsis",
                                    whiteSpace: "nowrap",
                                  }}
                                >
                                  {step.name}
                                </span>
                                {stepDuration !== null && (
                                  <span
                                    className="font-mono"
                                    style={{
                                      fontSize: 11,
                                      color: "var(--text-secondary)",
                                      flexShrink: 0,
                                    }}
                                  >
                                    {stepDuration < 60
                                      ? `${stepDuration}s`
                                      : `${Math.floor(stepDuration / 60)}m ${stepDuration % 60}s`}
                                  </span>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
            {!historyCollapsed && (
              <ResizeHandle onMouseDown={makeResizeHandler(setHistoryHeight, 150, 800)} />
            )}
          </div>
        )}
      </div>

      {confirmDelete && (
        <ConfirmDialog
          title="Delete Runner"
          message={`Are you sure you want to delete "${config.name}"? This will stop the runner and de-register it from GitHub.`}
          confirmLabel="Delete"
          danger
          onConfirm={handleDelete}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </div>
  );
}
