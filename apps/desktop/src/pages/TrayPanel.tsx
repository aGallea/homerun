import { useState, useEffect, useCallback } from "react";
import { api } from "../api/commands";
import type { RunnerInfo } from "../api/types";

function formatElapsed(jobStartedAt: string | null | undefined): string {
  if (!jobStartedAt) return "";
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return "";
  const secs = Math.floor((Date.now() - started) / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}m ${rem.toString().padStart(2, "0")}s`;
}

function jobProgress(
  jobStartedAt: string | null | undefined,
  estimatedDuration: number | null | undefined,
): number | null {
  if (!jobStartedAt || !estimatedDuration || estimatedDuration <= 0) return null;
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return null;
  const elapsed = (Date.now() - started) / 1000;
  return Math.min(elapsed / estimatedDuration, 1);
}

const stateColors: Record<string, string> = {
  online: "var(--accent-green)",
  busy: "var(--accent-yellow)",
  offline: "#484f58",
  error: "var(--accent-red)",
  creating: "var(--accent-blue)",
  registering: "var(--accent-blue)",
  stopping: "var(--accent-yellow)",
  deleting: "var(--accent-red)",
};

function stateLabel(r: RunnerInfo): string {
  if (r.state === "busy") return "";
  if (r.state === "online") return "Idle";
  return r.state.charAt(0).toUpperCase() + r.state.slice(1);
}

export function TrayPanel() {
  const [runners, setRunners] = useState<RunnerInfo[]>([]);
  const [daemonOk, setDaemonOk] = useState(true);
  const [daemonStopping, setDaemonStopping] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const data = await api.listRunners();
      setRunners(data);
      setDaemonOk(true);
    } catch {
      setRunners([]);
      setDaemonOk(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);
    return () => clearInterval(interval);
  }, [refresh]);

  const counts = { online: 0, busy: 0, offline: 0 };
  for (const r of runners) {
    if (r.state === "busy") counts.busy++;
    else if (r.state === "offline" || r.state === "error") counts.offline++;
    else counts.online++;
  }

  const sorted = [...runners].sort((a, b) => {
    const order: Record<string, number> = { busy: 0, online: 1, creating: 1, registering: 1 };
    return (order[a.state] ?? 2) - (order[b.state] ?? 2);
  });

  return (
    <div className="tray-panel">
      <div className="tray-header">
        <div className="tray-header-left">
          <span
            className="tray-health-dot"
            style={{ background: daemonOk ? "var(--accent-green)" : "#484f58" }}
          />
          <span className="tray-title">HomeRun</span>
        </div>
        <span className="tray-daemon-status">{daemonOk ? "Daemon running" : "Daemon offline"}</span>
      </div>

      <div className="tray-summary">
        <span>
          <strong style={{ color: "var(--accent-green)" }}>{counts.online}</strong>{" "}
          <span className="tray-muted">online</span>
        </span>
        <span>
          <strong style={{ color: "var(--accent-yellow)" }}>{counts.busy}</strong>{" "}
          <span className="tray-muted">busy</span>
        </span>
        <span>
          <strong className="tray-muted">{counts.offline}</strong>{" "}
          <span className="tray-muted">offline</span>
        </span>
      </div>

      <div className="tray-runners">
        {sorted.map((runner) => {
          const pct = jobProgress(runner.job_started_at, runner.estimated_job_duration_secs);
          const dotColor = stateColors[runner.state] || "var(--text-secondary)";
          const isOff = runner.state === "offline" || runner.state === "error";
          return (
            <div key={runner.config.id} className="tray-runner-row">
              <span className="tray-runner-dot" style={{ background: dotColor }} />
              <div className="tray-runner-info">
                <div className="tray-runner-top">
                  <span
                    className="tray-runner-name"
                    style={isOff ? { color: "var(--text-secondary)" } : undefined}
                  >
                    {runner.config.name}
                  </span>
                  {runner.state === "busy" && (
                    <span className="tray-runner-time">{formatElapsed(runner.job_started_at)}</span>
                  )}
                  {runner.state !== "busy" && (
                    <span className="tray-runner-state" style={{ color: dotColor }}>
                      {stateLabel(runner)}
                    </span>
                  )}
                </div>
                {runner.state === "busy" && (
                  <>
                    <div className="tray-runner-job">{runner.current_job ?? "Starting..."}</div>
                    {pct != null && (
                      <div className="tray-progress-track">
                        <div
                          className="tray-progress-bar"
                          style={{ width: `${Math.min(pct, 1) * 100}%` }}
                        />
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
          );
        })}
        {runners.length === 0 && (
          <div className="tray-no-runners">
            {daemonOk ? "No runners configured" : "Cannot reach daemon"}
          </div>
        )}
      </div>

      <div className="tray-actions">
        <button className="tray-action" onClick={() => api.toggleMiniWindow()}>
          <span>Toggle Mini View</span>
          <span className="tray-shortcut">⌘⇧M</span>
        </button>
        <button className="tray-action" onClick={() => api.showMainWindow()}>
          <span>Open HomeRun</span>
          <span className="tray-shortcut">⌘⇧H</span>
        </button>
        <button
          className="tray-action danger"
          disabled={daemonStopping}
          onClick={async () => {
            setDaemonStopping(true);
            try {
              if (daemonOk) {
                await api.stopDaemon();
              } else {
                await api.startDaemon();
              }
            } catch {
              /* ignore */
            } finally {
              setDaemonStopping(false);
              refresh();
            }
          }}
        >
          {daemonOk ? "Stop Daemon" : "Start Daemon"}
        </button>
        <button className="tray-action" onClick={() => api.quitApp()}>
          <span>Quit HomeRun</span>
          <span className="tray-shortcut">⌘Q</span>
        </button>
      </div>
    </div>
  );
}
