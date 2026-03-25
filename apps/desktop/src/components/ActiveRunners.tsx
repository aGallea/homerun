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
