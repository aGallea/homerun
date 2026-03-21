import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useRunners } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { StatusBadge } from "../components/StatusBadge";
import { ConfirmDialog } from "../components/ConfirmDialog";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

export function RunnerDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const {
    runners,
    loading,
    startRunner,
    stopRunner,
    restartRunner,
    deleteRunner,
  } = useRunners();
  const { metrics } = useMetrics();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const runner = runners.find((r) => r.config.id === id);
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
          <button className="btn" onClick={() => navigate("/runners")}>
            ← Back to Runners
          </button>
        </div>
        <p className="text-muted">Runner not found.</p>
      </div>
    );
  }

  const { config, state, uptime_secs, jobs_completed, jobs_failed } = runner;
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
      navigate("/runners");
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
            onClick={() => navigate("/runners")}
            style={{ padding: "6px 12px" }}
          >
            ← Back
          </button>
          <div>
            <div className="flex items-center gap-12" style={{ marginBottom: 4 }}>
              <h1 className="page-title" style={{ fontSize: 20 }}>
                {config.name}
              </h1>
              <StatusBadge state={state} />
            </div>
            <p className="text-muted" style={{ fontSize: 12, margin: 0 }}>
              {config.repo_owner}/{config.repo_name}
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
            <button
              className="btn"
              onClick={() => doAction(() => stopRunner(config.id))}
            >
              ■ Stop
            </button>
          )}
          <button
            className="btn"
            onClick={() => doAction(() => restartRunner(config.id))}
          >
            ↺ Restart
          </button>
          <button
            className="btn btn-danger"
            onClick={() => setConfirmDelete(true)}
          >
            Delete
          </button>
        </div>
      </div>

      {actionError && (
        <div className="error-banner" style={{ marginBottom: 20 }}>
          {actionError}
        </div>
      )}

      {/* Info cards */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
          gap: 16,
          marginBottom: 24,
        }}
      >
        <InfoCard label="Repository">
          <a
            href={`https://github.com/${config.repo_owner}/${config.repo_name}`}
            target="_blank"
            rel="noreferrer"
          >
            {config.repo_owner}/{config.repo_name}
          </a>
        </InfoCard>

        <InfoCard label="Mode">
          <span style={{ textTransform: "capitalize" }}>{config.mode}</span>
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

        <InfoCard label="Uptime">
          {uptime_secs != null ? formatUptime(uptime_secs) : "--"}
        </InfoCard>

        <InfoCard label="Jobs Completed">
          <span style={{ color: "var(--accent-green)", fontWeight: 600 }}>
            {jobs_completed}
          </span>
        </InfoCard>

        <InfoCard label="Jobs Failed">
          <span
            style={{
              color:
                jobs_failed > 0
                  ? "var(--accent-red)"
                  : "var(--text-secondary)",
              fontWeight: 600,
            }}
          >
            {jobs_failed}
          </span>
        </InfoCard>

        {runnerMetrics && (
          <InfoCard label="CPU Usage">
            <MetricBar value={runnerMetrics.cpu_percent} />
          </InfoCard>
        )}

        {runnerMetrics && (
          <InfoCard label="Memory">
            <span className="font-mono" style={{ fontSize: 13 }}>
              {(runnerMetrics.memory_bytes / 1024 / 1024).toFixed(0)} MB
            </span>
          </InfoCard>
        )}
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
          className="font-mono"
          style={{
            background: "var(--bg-primary)",
            border: "1px solid var(--border)",
            borderRadius: 6,
            padding: 16,
            minHeight: 160,
            fontSize: 12,
            color: "var(--text-secondary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          Logs will stream here when connected.
        </div>
      </div>

      {/* Danger zone */}
      <div
        className="card"
        style={{ borderColor: "var(--accent-red)", marginBottom: 24 }}
      >
        <h2
          style={{
            fontSize: 14,
            fontWeight: 600,
            marginBottom: 8,
            color: "var(--accent-red)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
          }}
        >
          Danger Zone
        </h2>
        <div
          className="flex items-center justify-between"
          style={{ padding: "12px 0" }}
        >
          <div>
            <div style={{ fontWeight: 500, marginBottom: 4 }}>
              Delete this runner
            </div>
            <p className="text-muted" style={{ fontSize: 12, margin: 0 }}>
              The runner will be stopped, de-registered from GitHub, and
              permanently removed.
            </p>
          </div>
          <button
            className="btn btn-danger"
            style={{ flexShrink: 0, marginLeft: 24 }}
            onClick={() => setConfirmDelete(true)}
          >
            Delete Runner
          </button>
        </div>
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

// Helper components

function InfoCard({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
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
      <div style={{ fontSize: 14, color: "var(--text-primary)" }}>
        {children}
      </div>
    </div>
  );
}

function MetricBar({ value }: { value: number }) {
  const color =
    value >= 80
      ? "var(--accent-red)"
      : value >= 60
        ? "var(--accent-yellow)"
        : "var(--accent-green)";

  return (
    <div>
      <div className="flex items-center justify-between" style={{ marginBottom: 4 }}>
        <span className="font-mono" style={{ fontSize: 12 }}>
          {value.toFixed(1)}%
        </span>
      </div>
      <div
        style={{
          height: 6,
          background: "var(--bg-tertiary)",
          borderRadius: 3,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            height: "100%",
            width: `${Math.min(value, 100)}%`,
            background: color,
            borderRadius: 3,
            transition: "width 0.3s ease",
          }}
        />
      </div>
    </div>
  );
}
