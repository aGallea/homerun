import { useEffect, useRef } from "react";
import { useRunners } from "../hooks/useRunners";
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

function countByState(runners: RunnerInfo[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const r of runners) {
    const key = r.state === "busy" ? "busy" : r.state === "offline" ? "offline" : "online";
    counts[key] = (counts[key] || 0) + 1;
  }
  return counts;
}

export function MiniView() {
  const { runners, error } = useRunners();
  const positionSaved = useRef(false);

  const busy = runners
    .filter((r) => r.state === "busy")
    .sort((a, b) => {
      const aTime = a.job_started_at ? new Date(a.job_started_at).getTime() : -Infinity;
      const bTime = b.job_started_at ? new Date(b.job_started_at).getTime() : -Infinity;
      return bTime - aTime;
    });

  const counts = countByState(runners);
  const daemonOk = error === null;

  // Save position on window move (debounced)
  useEffect(() => {
    async function onMove() {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const pos = await win.outerPosition();
      const scale = (await win.scaleFactor()) || 1;
      await api.saveMiniPosition(pos.x / scale, pos.y / scale);
    }

    const handler = () => {
      if (positionSaved.current) return;
      positionSaved.current = true;
      setTimeout(() => {
        positionSaved.current = false;
        onMove().catch(() => {});
      }, 500);
    };

    window.addEventListener("mouseup", handler);
    return () => window.removeEventListener("mouseup", handler);
  }, []);

  return (
    <div className="mini-view" data-tauri-drag-region>
      <div className="mini-header" data-tauri-drag-region>
        <div className="mini-header-left" data-tauri-drag-region>
          <span className={`mini-health-dot ${daemonOk ? "online" : "offline"}`} />
          <span className="mini-label">HOMERUN</span>
        </div>
        <div className="mini-header-right" data-tauri-drag-region>
          {(counts.online || 0) > 0 && (
            <span className="mini-count online">{counts.online} online</span>
          )}
          {(counts.busy || 0) > 0 && <span className="mini-count busy">{counts.busy} busy</span>}
          {(counts.offline || 0) > 0 && (
            <span className="mini-count offline">{counts.offline} off</span>
          )}
        </div>
      </div>

      {busy.map((runner) => {
        const pct = jobProgress(runner.job_started_at, runner.estimated_job_duration_secs);
        return (
          <div key={runner.config.id} className="mini-runner-card">
            <div className="mini-runner-top">
              <span className="mini-runner-name">{runner.config.name}</span>
              <span className="mini-runner-time">{formatElapsed(runner.job_started_at)}</span>
            </div>
            <div className="mini-runner-job">{runner.current_job ?? "Starting..."}</div>
            {pct != null && (
              <div className="mini-progress-track">
                <div
                  className={`mini-progress-bar${pct >= 1 ? " over" : ""}`}
                  style={{ width: `${Math.min(pct, 1) * 100}%` }}
                />
              </div>
            )}
          </div>
        );
      })}

      {busy.length === 0 && runners.length > 0 && (
        <div className="mini-empty">All runners idle</div>
      )}
      {runners.length === 0 && <div className="mini-empty">No runners</div>}
    </div>
  );
}
