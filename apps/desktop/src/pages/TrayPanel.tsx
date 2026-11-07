import { useState, useEffect, useRef, useCallback } from "react";
import { useRunners } from "../hooks/useRunners";
import { useTrayIcon } from "../hooks/useTrayIcon";
import { api } from "../api/commands";
import { jobProgress, formatJobElapsed } from "../utils/runnerHelpers";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useRunnerDisplayPreferences } from "../hooks/useRunnerDisplayPreferences";
import { filterCompactRunners, sortRunnersByActivity } from "../utils/runnerOrdering";

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

function stateLabel(state: string): string {
  if (state === "busy") return "";
  if (state === "online") return "Idle";
  return state.charAt(0).toUpperCase() + state.slice(1);
}

const TRAY_PANEL_WIDTH = 300;

export function TrayPanel() {
  const { runners, loading, error } = useRunners();
  const daemonOk = !loading && error === null;
  const [daemonStopping, setDaemonStopping] = useState(false);
  const { hideOfflineRunnersInMiniView, sortRunnersByActivity: sortByRecentActivity } =
    useRunnerDisplayPreferences();
  const containerRef = useRef<HTMLDivElement>(null);

  useTrayIcon(runners, daemonOk);

  const resizeToFit = useCallback(() => {
    if (!containerRef.current) return;
    const height = containerRef.current.offsetHeight;
    getCurrentWindow()
      .setSize(new LogicalSize(TRAY_PANEL_WIDTH, height))
      .catch(() => {});
  }, []);

  useEffect(() => {
    resizeToFit();
  }, [runners, hideOfflineRunnersInMiniView, resizeToFit]);

  const counts = { online: 0, busy: 0, offline: 0, transitioning: 0 };
  for (const runner of runners) {
    if (runner.state === "busy") counts.busy++;
    else if (runner.state === "online") counts.online++;
    else if (runner.state === "offline" || runner.state === "error") counts.offline++;
    else counts.transitioning++;
  }

  const compactRunners = filterCompactRunners(runners, hideOfflineRunnersInMiniView);
  const sorted = sortByRecentActivity
    ? sortRunnersByActivity(compactRunners)
    : [...compactRunners].sort((a, b) => {
        const order: Record<string, number> = { busy: 0, online: 1, creating: 1, registering: 1 };
        return (order[a.state] ?? 2) - (order[b.state] ?? 2);
      });

  return (
    <div className="tray-panel" ref={containerRef}>
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
        {counts.transitioning > 0 && (
          <span>
            <strong style={{ color: "var(--accent-blue)" }}>{counts.transitioning}</strong>{" "}
            <span className="tray-muted">starting/stopping</span>
          </span>
        )}
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
                    {runner.config.display_name ?? runner.config.name}
                  </span>
                  {runner.state === "busy" && (
                    <span className="tray-runner-time">
                      {formatJobElapsed(runner.job_started_at)}
                    </span>
                  )}
                  {runner.state !== "busy" && (
                    <span className="tray-runner-state" style={{ color: dotColor }}>
                      {stateLabel(runner.state)}
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
        {sorted.length === 0 && (
          <div className="tray-no-runners">
            {!daemonOk
              ? "Cannot reach daemon"
              : runners.length > 0 && hideOfflineRunnersInMiniView
                ? "Offline runners hidden"
                : "No runners configured"}
          </div>
        )}
      </div>

      <div className="tray-actions">
        <button className="tray-action" onClick={() => api.showMainWindow()}>
          Open Main Window
        </button>
        <button className="tray-action" onClick={() => api.toggleMiniWindow()}>
          Toggle Mini View
        </button>
        <button className="tray-action" onClick={() => api.hideAllWindows()}>
          Hide View
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
            }
          }}
        >
          {daemonOk ? "Stop Daemon" : "Start Daemon"}
        </button>
        <button className="tray-action" onClick={() => api.quitApp()}>
          Quit HomeRun
        </button>
      </div>
    </div>
  );
}
