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
  const [showAllHistory, setShowAllHistory] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [logSearch, setLogSearch] = useState("");
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

  const filteredLogs = logSearch
    ? logs.filter((entry) => entry.line.toLowerCase().includes(logSearch.toLowerCase()))
    : logs;

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
            <span style={{ fontWeight: 600, color: "var(--accent-green)" }}>{jobs_completed}</span>
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
          {config.labels.length > 0 && (
            <>
              <span style={{ color: "var(--border)" }}>|</span>
              <div className="flex items-center" style={{ gap: 4 }}>
                {config.labels.map((lbl) => (
                  <span
                    key={lbl}
                    className="label-tag"
                    style={{ fontSize: 11, padding: "1px 6px" }}
                  >
                    {lbl}
                  </span>
                ))}
              </div>
            </>
          )}
        </div>

        {/* Main content: Current Job (30%) + Logs (70%) */}
        <div style={{ display: "flex", gap: 12, height: 160 }}>
          {/* Left: Current Job */}
          <div style={{ flex: "0 0 30%", minWidth: 0, display: "flex" }}>
            <div className="runner-card runner-card-job" style={{ flex: 1 }}>
              <div className="runner-card-glow runner-card-glow-blue" />
              <div className="flex items-center justify-between">
                <h3 className="runner-card-label">Current Job</h3>
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
                  <div className="glow-bar-track">
                    <div
                      className="glow-bar-fill"
                      style={{
                        width: "60%",
                        background: "var(--accent-blue)",
                        boxShadow: "0 0 8px rgba(59, 130, 246, 0.8)",
                      }}
                    />
                  </div>
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

          {/* Right: Logs panel */}
          <div
            className="logs-panel"
            style={{
              flex: "1 1 70%",
              minWidth: 0,
              display: "flex",
              flexDirection: "column",
              height: "100%",
            }}
          >
            <div className="logs-header">
              <span className="runner-card-label" style={{ margin: 0, fontSize: 11 }}>
                Runner Process Logs
              </span>
              <div className="flex items-center gap-16">
                <div className="logs-search-wrapper">
                  <span className="logs-search-icon">⌕</span>
                  <input
                    className="logs-search-input"
                    placeholder="Search"
                    value={logSearch}
                    onChange={(e) => setLogSearch(e.target.value)}
                  />
                </div>
                <label className="follow-toggle">
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
              </div>
            </div>
            <div
              ref={logContainerRef}
              className="logs-content font-mono"
              style={{ flex: 1, minHeight: 0, overflow: "auto" }}
            >
              {filteredLogs.length === 0 ? (
                <div className="logs-empty">
                  {runner.state === "online" || runner.state === "busy"
                    ? "Waiting for log output..."
                    : "Runner is not active."}
                </div>
              ) : (
                <table className="logs-table">
                  <tbody>
                    {filteredLogs.map((entry, i) => (
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
          </div>
        </div>
        {/* end side-by-side */}

        {/* Job Progress — full width, only when busy */}
        {state === "busy" && steps.length > 0 && (
          <JobProgress
            steps={steps}
            stepsDiscovered={stepsDiscovered}
            jobName={jobName}
            expandedStep={expandedStep}
            stepLogs={stepLogs}
            onToggleStep={toggleStep}
          />
        )}

        {/* Job History */}
        {history.length > 0 && (
          <div style={{ marginTop: 12 }}>
            <h3 className="runner-card-label" style={{ marginBottom: 8, fontSize: 11 }}>
              Job History
            </h3>
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 1,
                borderRadius: 8,
                overflow: "hidden",
                border: "1px solid var(--border)",
              }}
            >
              {history.slice(0, showAllHistory ? history.length : 20).map((entry, i) => {
                const duration = Math.round(
                  (new Date(entry.completed_at).getTime() - new Date(entry.started_at).getTime()) /
                    1000,
                );
                return (
                  <div
                    key={i}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 12,
                      padding: "8px 12px",
                      background: i % 2 === 0 ? "var(--bg-secondary)" : "var(--bg-primary)",
                      fontSize: 13,
                    }}
                  >
                    <span
                      style={{
                        width: 8,
                        height: 8,
                        borderRadius: "50%",
                        background: entry.succeeded ? "var(--accent-green)" : "var(--accent-red)",
                        flexShrink: 0,
                      }}
                    />
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
                    {entry.branch && (
                      <span
                        style={{
                          fontSize: 11,
                          color: "var(--text-secondary)",
                          flexShrink: 0,
                        }}
                      >
                        {entry.branch}
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
                  </div>
                );
              })}
            </div>
            {!showAllHistory && history.length > 20 && (
              <button
                className="btn"
                style={{ marginTop: 8, fontSize: 12 }}
                onClick={() => setShowAllHistory(true)}
              >
                Show all {history.length} entries
              </button>
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
