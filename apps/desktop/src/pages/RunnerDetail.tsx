import { useState, useEffect, useRef } from "react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { useRunners } from "../hooks/useRunners";
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
        right: 0,
        width: 20,
        height: 20,
        cursor: "nwse-resize",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        opacity: 0.4,
        fontSize: 10,
        color: "var(--text-secondary)",
        userSelect: "none",
      }}
      title="Drag to resize"
    >
      ⟍
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
      {exceeding && (
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
  const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner } = useRunners();
  const { metrics } = useMetrics();
  const runner = runners.find((r) => r.config.id === id);
  const { steps, stepsDiscovered, jobName, expandedStep, stepLogs, toggleStep } = useJobSteps(
    id,
    runner?.state === "busy",
  );
  const { history } = useJobHistory(id);
  const [expandedHistoryIndex, setExpandedHistoryIndex] = useState<number | null>(null);
  const expandedHistoryRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (expandedHistoryIndex != null && expandedHistoryRef.current) {
      setTimeout(
        () => expandedHistoryRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" }),
        50,
      );
    }
  }, [expandedHistoryIndex]);

  const [logsHeight, setLogsHeight] = useState(140);
  const [logsCollapsed, setLogsCollapsed] = useState(false);
  const [stepsHeight, setStepsHeight] = useState(300);
  const [stepsCollapsed, setStepsCollapsed] = useState(false);
  const [historyHeight, setHistoryHeight] = useState(200);
  const [historyCollapsed, setHistoryCollapsed] = useState(false);

  const [confirmDelete, setConfirmDelete] = useState(false);
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
            ← Back to Dashboard
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
    try {
      await deleteRunner(config.id);
      navigate("/dashboard");
    } catch (e) {
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
            Dashboard
          </Link>
          <span className="breadcrumb-sep">›</span>
          <span className="breadcrumb-current">{config.name}</span>
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
        {/* Top section: actions + stats on left, current job card on right */}
        <div style={{ display: "flex", gap: 16, marginBottom: 12 }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            {/* Action buttons */}
            {isAuthenticated && (
              <div className="flex items-center gap-8" style={{ marginBottom: 16 }}>
                {isTransient && (
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
                {isStopped && (
                  <button
                    className="btn btn-primary"
                    onClick={() => doAction(() => startRunner(config.id))}
                  >
                    ▶ Start
                  </button>
                )}
                {isRunning && (
                  <button
                    className="runner-action-btn"
                    onClick={() => doAction(() => stopRunner(config.id))}
                  >
                    ■ Stop
                  </button>
                )}
                <button
                  className="runner-action-btn"
                  onClick={() => doAction(() => restartRunner(config.id))}
                  disabled={!canRestart}
                >
                  ↺ Restart
                </button>
                <button
                  className="runner-action-btn runner-action-btn-danger"
                  onClick={() => setConfirmDelete(true)}
                  disabled={!canDelete}
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
                      const url =
                        job_context?.run_url ??
                        `https://github.com/${config.repo_owner}/${config.repo_name}/actions?query=is%3Ain_progress`;
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
              height: historyCollapsed
                ? "auto"
                : expandedHistoryIndex != null
                  ? "auto"
                  : historyHeight,
              maxHeight: historyCollapsed
                ? undefined
                : expandedHistoryIndex != null
                  ? "50vh"
                  : undefined,
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
                  const isExpanded = expandedHistoryIndex === i;
                  const hasSteps = entry.steps && entry.steps.length > 0;
                  return (
                    <div key={i} ref={isExpanded ? expandedHistoryRef : undefined}>
                      <div
                        onClick={() => {
                          if (hasSteps) setExpandedHistoryIndex(isExpanded ? null : i);
                        }}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 12,
                          padding: "6px 12px",
                          background: i % 2 === 0 ? "var(--bg-secondary)" : "var(--bg-primary)",
                          fontSize: 13,
                          cursor: hasSteps ? "pointer" : "default",
                        }}
                      >
                        <span
                          style={{
                            width: 8,
                            height: 8,
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
                              fontSize: 14,
                              lineHeight: 1,
                              color: "var(--text-secondary)",
                              flexShrink: 0,
                            }}
                          >
                            {isExpanded ? "\u25BE" : "\u25B8"}
                          </span>
                        )}
                        <span
                          style={{
                            flex: 1,
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
                        {(entry.branch || entry.pr_number != null) && (
                          <span
                            style={{
                              fontSize: 11,
                              color: "var(--text-secondary)",
                              flexShrink: 0,
                              display: "flex",
                              alignItems: "center",
                              gap: 4,
                            }}
                          >
                            {entry.branch && <span>{entry.branch}</span>}
                            {entry.pr_number != null && (
                              <span style={{ color: "var(--accent-blue)" }}>
                                #{entry.pr_number}
                              </span>
                            )}
                          </span>
                        )}
                        <span
                          className="font-mono"
                          style={{
                            fontSize: 11,
                            color: "var(--text-secondary)",
                            flexShrink: 0,
                          }}
                        >
                          {formatUptime(duration)}
                        </span>
                        <span
                          style={{
                            fontSize: 11,
                            color: "var(--text-secondary)",
                            flexShrink: 0,
                          }}
                        >
                          {new Date(entry.completed_at).toLocaleTimeString()}
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
                            style={{
                              fontSize: 11,
                              color: "var(--accent-blue)",
                              flexShrink: 0,
                            }}
                          >
                            View →
                          </a>
                        )}
                        {entry.run_url && id && (
                          <a
                            href="#"
                            onClick={(e) => {
                              e.preventDefault();
                              e.stopPropagation();
                              api.rerunWorkflow(id!, entry.run_url!).catch(() => {});
                            }}
                            style={{
                              fontSize: 11,
                              color: "var(--text-secondary)",
                              flexShrink: 0,
                            }}
                          >
                            Re-run
                          </a>
                        )}
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
