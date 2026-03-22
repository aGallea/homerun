import { useState, useEffect, useRef } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useRunners } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { api } from "../api/commands";
import type { LogEntry } from "../api/types";
import { StatusBadge } from "../components/StatusBadge";
import { ConfirmDialog } from "../components/ConfirmDialog";

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

function cpuBarColor(percent: number): string {
  if (percent <= 60) return "#22c55e";
  if (percent <= 80) return "linear-gradient(90deg, #22c55e, #eab308)";
  return "linear-gradient(90deg, #eab308, #ef4444)";
}

function ResourceBar({
  label,
  percent,
  value,
  color,
}: {
  label: string;
  percent: number;
  value: string;
  color: string;
}) {
  const clampedPercent = Math.max(0, Math.min(100, percent));
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
      <span style={{ fontSize: 12, color: "var(--text-secondary)", width: 32, textAlign: "right" }}>
        {label}
      </span>
      <div
        style={{
          flex: 1,
          height: 16,
          background: "var(--bg-tertiary)",
          borderRadius: 3,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: `${clampedPercent}%`,
            height: "100%",
            borderRadius: 3,
            background: color,
            transition: "width 0.3s ease",
          }}
        />
      </div>
      <span
        style={{
          fontSize: 13,
          fontFamily: "monospace",
          color: "var(--text-primary)",
          width: 72,
          textAlign: "right",
        }}
      >
        {value}
      </span>
    </div>
  );
}

export function RunnerDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner } = useRunners();
  const { metrics } = useMetrics();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const logContainerRef = useRef<HTMLDivElement>(null);

  // Poll for recent logs every 2 seconds
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

  // Auto-scroll logs
  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  const runner = runners.find((r) => r.config.id === id);
  const runnerMetrics = metrics?.runners.find((m) => m.runner_id === id);
  const os_total_memory = metrics?.system.memory_total_bytes ?? 0;

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

  async function doAction(fn: () => Promise<void>) {
    setActionError(null);
    try {
      await fn();
    } catch (e) {
      setActionError(String(e));
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

  return (
    <div className="page">
      {/* Header */}
      <div className="page-header">
        <div className="flex items-center gap-16">
          <button
            className="btn"
            onClick={() => navigate("/dashboard")}
            style={{ padding: "6px 12px" }}
          >
            ← Back
          </button>
          <div>
            <div className="flex items-center gap-12" style={{ marginBottom: 4 }}>
              <h1 className="page-title" style={{ fontSize: 20 }}>
                {config.name}
              </h1>
              <StatusBadge state={state} currentJob={current_job ?? undefined} />
              {uptime_secs != null && (
                <span className="text-muted" style={{ fontSize: 12 }}>
                  Uptime: {formatUptime(uptime_secs)}
                </span>
              )}
            </div>
            <p className="text-muted" style={{ fontSize: 12, margin: 0 }}>
              <a
                href="#"
                onClick={(e) => {
                  e.preventDefault();
                  import("@tauri-apps/plugin-shell").then(({ open }) => {
                    open(`https://github.com/${config.repo_owner}/${config.repo_name}`);
                  });
                }}
                style={{ color: "var(--accent-blue)", cursor: "pointer" }}
              >
                {config.repo_owner}/{config.repo_name}
              </a>
              <span style={{ margin: "0 8px", opacity: 0.3 }}>·</span>
              <span style={{ textTransform: "capitalize" }}>{config.mode} mode</span>
            </p>
          </div>
        </div>

        <div className="flex items-center gap-8">
          {isStopped && (
            <button
              className="btn btn-primary"
              onClick={() => doAction(() => startRunner(config.id))}
            >
              ▶ Start
            </button>
          )}
          {isRunning && (
            <button className="btn" onClick={() => doAction(() => stopRunner(config.id))}>
              ■ Stop
            </button>
          )}
          <button className="btn" onClick={() => doAction(() => restartRunner(config.id))}>
            ↺ Restart
          </button>
          <button className="btn btn-danger" onClick={() => setConfirmDelete(true)}>
            Delete
          </button>
        </div>
      </div>

      {actionError && (
        <div className="error-banner" style={{ marginBottom: 20 }}>
          {actionError}
        </div>
      )}

      {/* Info cards — compact layout */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
          gap: 16,
          marginBottom: 24,
        }}
      >
        {current_job ? (
          <InfoCard label="Current Job">
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <div className="flex items-center gap-8">
                <span style={{ color: "var(--accent-yellow)" }}>{current_job}</span>
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    const url =
                      job_context?.run_url ??
                      `https://github.com/${config.repo_owner}/${config.repo_name}/actions?query=is%3Ain_progress`;
                    import("@tauri-apps/plugin-shell").then(({ open }) => {
                      open(url);
                    });
                  }}
                  style={{ fontSize: 11, color: "var(--accent-blue)", cursor: "pointer" }}
                >
                  View →
                </a>
              </div>
              {job_context && (
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  Branch: <span style={{ color: "var(--text-primary)" }}>{job_context.branch}</span>
                  {job_context.pr_number != null && (
                    <a
                      href="#"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        if (job_context.pr_url) {
                          import("@tauri-apps/plugin-shell").then(({ open }) => {
                            open(job_context.pr_url!);
                          });
                        }
                      }}
                      style={{ color: "var(--accent-blue)", marginLeft: 8, cursor: "pointer" }}
                    >
                      PR #{job_context.pr_number}
                    </a>
                  )}
                </div>
              )}
            </div>
          </InfoCard>
        ) : (
          <InfoCard label="Actions">
            <a
              href="#"
              onClick={(e) => {
                e.preventDefault();
                import("@tauri-apps/plugin-shell").then(({ open }) => {
                  open(`https://github.com/${config.repo_owner}/${config.repo_name}/actions`);
                });
              }}
              style={{ color: "var(--accent-blue)", cursor: "pointer" }}
            >
              View on GitHub →
            </a>
          </InfoCard>
        )}

        <InfoCard label="Jobs">
          <div className="flex items-center gap-16">
            <span>
              <span style={{ color: "var(--accent-green)", fontWeight: 600 }}>
                {jobs_completed}
              </span>
              <span className="text-muted" style={{ fontSize: 11, marginLeft: 4 }}>
                passed
              </span>
            </span>
            <span>
              <span
                style={{
                  color: jobs_failed > 0 ? "var(--accent-red)" : "var(--text-secondary)",
                  fontWeight: 600,
                }}
              >
                {jobs_failed}
              </span>
              <span className="text-muted" style={{ fontSize: 11, marginLeft: 4 }}>
                failed
              </span>
            </span>
          </div>
        </InfoCard>

        <InfoCard label="Resources">
          {runnerMetrics ? (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <ResourceBar
                label="CPU"
                percent={runnerMetrics.cpu_percent}
                value={`${runnerMetrics.cpu_percent.toFixed(1)}%`}
                color={cpuBarColor(runnerMetrics.cpu_percent)}
              />
              <ResourceBar
                label="MEM"
                percent={Math.min((runnerMetrics.memory_bytes / (os_total_memory || 1)) * 100, 100)}
                value={formatBytes(runnerMetrics.memory_bytes)}
                color="linear-gradient(90deg, #6366f1, #818cf8)"
              />
            </div>
          ) : (
            <span className="text-muted">—</span>
          )}
        </InfoCard>

        <InfoCard label="Labels">
          <div className="flex" style={{ flexWrap: "wrap", gap: 4 }}>
            {config.labels.map((lbl) => (
              <span
                key={lbl}
                style={{
                  fontSize: 11,
                  padding: "2px 8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: 10,
                  color: "var(--text-secondary)",
                }}
              >
                {lbl}
              </span>
            ))}
          </div>
        </InfoCard>
      </div>

      {/* Log viewer */}
      <div className="card" style={{ marginBottom: 24 }}>
        <h2
          style={{
            fontSize: 14,
            fontWeight: 600,
            marginBottom: 12,
            color: "var(--text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
          }}
        >
          Logs
        </h2>
        <div
          ref={logContainerRef}
          className="font-mono"
          style={{
            background: "var(--bg-primary)",
            border: "1px solid var(--border)",
            borderRadius: 6,
            padding: 12,
            minHeight: 160,
            maxHeight: 400,
            overflowY: "auto",
            fontSize: 12,
            lineHeight: 1.6,
            color: "var(--text-secondary)",
          }}
        >
          {logs.length === 0 ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                minHeight: 140,
                color: "var(--text-secondary)",
              }}
            >
              {runner.state === "online" || runner.state === "busy"
                ? "Waiting for log output..."
                : "Runner is not active."}
            </div>
          ) : (
            logs.map((entry, i) => (
              <div key={i} style={{ display: "flex", gap: 8 }}>
                <span style={{ color: "var(--text-secondary)", opacity: 0.5, flexShrink: 0 }}>
                  {new Date(entry.timestamp).toLocaleTimeString()}
                </span>
                <span
                  style={{
                    color: entry.stream === "stderr" ? "var(--accent-red)" : "var(--text-primary)",
                  }}
                >
                  {entry.line}
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Spacer before confirm dialog */}
      <div style={{ marginBottom: 24 }}></div>

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

// Helper components

function InfoCard({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="card" style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div
        style={{
          fontSize: 11,
          fontWeight: 500,
          color: "var(--text-secondary)",
          textTransform: "uppercase",
          letterSpacing: "0.5px",
        }}
      >
        {label}
      </div>
      <div style={{ fontSize: 14, color: "var(--text-primary)" }}>{children}</div>
    </div>
  );
}
