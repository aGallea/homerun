import { useState, useEffect, useRef } from "react";
import { useDaemonLogs } from "../hooks/useDaemonLogs";
import { useMetrics } from "../hooks/useMetrics";
import { api } from "../api/commands";

const LOG_LEVELS = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

const LEVEL_COLORS: Record<string, string> = {
  ERROR: "var(--accent-red)",
  WARN: "var(--accent-yellow)",
  INFO: "var(--accent-green)",
  DEBUG: "var(--accent-blue)",
  TRACE: "var(--text-secondary)",
};

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h < 24) return `${h}h ${m}m`;
  const d = Math.floor(h / 24);
  return `${d}d ${h % 24}h`;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
  return `${Math.round(bytes / (1024 * 1024))} MB`;
}

function formatTime(timestamp: string): string {
  const match = timestamp.match(/T(\d{2}):(\d{2}):(\d{2})/);
  if (match) return `${match[1]}:${match[2]}:${match[3]}`;
  try {
    return new Date(timestamp).toLocaleTimeString("en-US", { hour12: false });
  } catch {
    return timestamp;
  }
}

function shortenTarget(target: string): string {
  const parts = target.split("::");
  return parts[parts.length - 1];
}

function cpuColor(percent: number): string {
  if (percent <= 60) return "var(--accent-green)";
  if (percent <= 80) return "var(--accent-yellow)";
  return "#f97316";
}

export function Daemon() {
  const { logs, level, setLevel, search, setSearch, follow, setFollow, loading, error } =
    useDaemonLogs();
  const { metrics } = useMetrics();
  const logContainerRef = useRef<HTMLDivElement>(null);
  const [childrenExpanded, setChildrenExpanded] = useState(true);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [actionResult, setActionResult] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);
  const resultTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const handleDaemonAction = async (action: "start" | "stop" | "restart") => {
    setActionLoading(action);
    setActionResult(null);
    if (resultTimerRef.current) clearTimeout(resultTimerRef.current);
    try {
      if (action === "start") await api.startDaemon();
      else if (action === "stop") await api.stopDaemon();
      else await api.restartDaemon();
      const label = action.charAt(0).toUpperCase() + action.slice(1);
      setActionResult({ type: "success", message: `Daemon ${label.toLowerCase()}ed successfully` });
      resultTimerRef.current = setTimeout(() => setActionResult(null), 4000);
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      setActionResult({ type: "error", message: reason });
    } finally {
      setActionLoading(null);
    }
  };

  const daemon = metrics?.daemon;

  useEffect(() => {
    if (follow && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs, follow]);

  return (
    <div className="page" style={{ display: "flex", flexDirection: "column", overflow: "hidden" }}>
      <div className="page-header">
        <h1 className="page-title">Daemon</h1>
      </div>

      {error && !error.includes("connect") && <div className="error-banner">{error}</div>}

      {/* Daemon controls */}
      <div style={{ display: "flex", gap: "8px", marginBottom: "16px" }}>
        <button
          className="btn btn-primary"
          onClick={() => handleDaemonAction("start")}
          disabled={actionLoading !== null || !!metrics?.daemon}
        >
          {actionLoading === "start" ? "Starting..." : "Start"}
        </button>
        <button
          className="btn btn-secondary"
          onClick={() => handleDaemonAction("stop")}
          disabled={actionLoading !== null || !metrics?.daemon}
        >
          {actionLoading === "stop" ? "Stopping..." : "Stop"}
        </button>
        <button
          className="btn btn-secondary"
          onClick={() => handleDaemonAction("restart")}
          disabled={actionLoading !== null || !metrics?.daemon}
        >
          {actionLoading === "restart" ? "Restarting..." : "Restart"}
        </button>
      </div>

      {actionResult && (
        <div
          style={{
            marginBottom: 12,
            padding: "8px 12px",
            borderRadius: 6,
            fontSize: 13,
            color: actionResult.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            background:
              actionResult.type === "success" ? "rgba(34, 197, 94, 0.1)" : "rgba(239, 68, 68, 0.1)",
            border: `1px solid ${
              actionResult.type === "success" ? "rgba(34, 197, 94, 0.2)" : "rgba(239, 68, 68, 0.2)"
            }`,
          }}
        >
          {actionResult.message}
        </div>
      )}

      {/* Status cards */}
      <div className="stats-grid" style={{ marginBottom: 16 }}>
        <div className="card" style={{ padding: "16px 20px" }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 4 }}>
            Status / PID
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {daemon ? (
              <span style={{ color: "var(--accent-green)" }}>Running</span>
            ) : (
              <span style={{ color: "var(--text-secondary)" }}>--</span>
            )}
            {daemon && (
              <span
                className="font-mono"
                style={{ fontSize: 13, color: "var(--text-secondary)", marginLeft: 8 }}
              >
                PID {daemon.pid}
              </span>
            )}
          </div>
        </div>

        <div className="card" style={{ padding: "16px 20px" }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 4 }}>
            Uptime
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {daemon ? formatUptime(daemon.uptime_seconds) : "--"}
          </div>
        </div>

        <div className="card" style={{ padding: "16px 20px" }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 4 }}>CPU</div>
          <div style={{ fontSize: 20, fontWeight: 600, marginBottom: 6 }}>
            {daemon ? `${daemon.cpu_percent.toFixed(1)}%` : "--"}
          </div>
          {daemon && (
            <div className="glow-bar-track">
              <div
                className="glow-bar-fill"
                style={{
                  width: `${Math.min(daemon.cpu_percent, 100)}%`,
                  background: cpuColor(daemon.cpu_percent),
                  boxShadow: `0 0 8px ${cpuColor(daemon.cpu_percent)}80`,
                }}
              />
            </div>
          )}
        </div>

        <div className="card" style={{ padding: "16px 20px" }}>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 4 }}>
            Memory
          </div>
          <div style={{ fontSize: 20, fontWeight: 600, marginBottom: 6 }}>
            {daemon ? formatBytes(daemon.memory_bytes) : "--"}
          </div>
          {daemon && metrics?.system && (
            <div className="glow-bar-track">
              <div
                className="glow-bar-fill"
                style={{
                  width: `${Math.min((daemon.memory_bytes / metrics.system.memory_total_bytes) * 100, 100)}%`,
                  background: "var(--accent-blue)",
                  boxShadow: "0 0 8px rgba(59, 130, 246, 0.8)",
                }}
              />
            </div>
          )}
        </div>
      </div>

      {/* Child processes */}
      {daemon && daemon.child_processes.length > 0 && (
        <div className="card" style={{ padding: "12px 16px", marginBottom: 16 }}>
          <button
            onClick={() => setChildrenExpanded((e) => !e)}
            style={{
              background: "none",
              border: "none",
              color: "var(--text-primary)",
              cursor: "pointer",
              fontSize: 13,
              fontWeight: 600,
              padding: 0,
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <span style={{ fontSize: 10 }}>{childrenExpanded ? "\u25BC" : "\u25B6"}</span>
            Child Processes ({daemon.child_processes.length})
          </button>
          {childrenExpanded && (
            <table
              style={{ width: "100%", marginTop: 8, fontSize: 13, borderCollapse: "collapse" }}
            >
              <thead>
                <tr style={{ color: "var(--text-secondary)", textAlign: "left" }}>
                  <th style={{ padding: "4px 8px", fontWeight: 500 }}>Runner</th>
                  <th style={{ padding: "4px 8px", fontWeight: 500 }}>PID</th>
                  <th style={{ padding: "4px 8px", fontWeight: 500 }}>CPU %</th>
                  <th style={{ padding: "4px 8px", fontWeight: 500 }}>Memory</th>
                </tr>
              </thead>
              <tbody>
                {daemon.child_processes.map((cp) => (
                  <tr key={cp.pid} style={{ borderTop: "1px solid var(--border)" }}>
                    <td style={{ padding: "6px 8px", color: "var(--text-primary)" }}>
                      {cp.runner_name}
                    </td>
                    <td
                      className="font-mono"
                      style={{ padding: "6px 8px", color: "var(--text-secondary)" }}
                    >
                      {cp.pid}
                    </td>
                    <td
                      className="font-mono"
                      style={{ padding: "6px 8px", color: cpuColor(cp.cpu_percent) }}
                    >
                      {cp.cpu_percent.toFixed(1)}%
                    </td>
                    <td
                      className="font-mono"
                      style={{ padding: "6px 8px", color: "var(--text-secondary)" }}
                    >
                      {formatBytes(cp.memory_bytes)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {/* Logs panel */}
      <div className="logs-panel" style={{ flex: 1, minHeight: 0 }}>
        <div className="logs-header">
          <h3 className="runner-card-label" style={{ margin: 0 }}>
            Logs
          </h3>
          <div className="flex items-center gap-16">
            {/* Level filter pills */}
            <div className="flex items-center gap-4">
              {LOG_LEVELS.map((lvl) => (
                <button
                  key={lvl}
                  onClick={() => setLevel(lvl)}
                  style={{
                    padding: "2px 8px",
                    fontSize: 11,
                    fontWeight: 600,
                    borderRadius: 4,
                    border: `1px solid ${level === lvl ? LEVEL_COLORS[lvl] : "var(--border)"}`,
                    background: level === lvl ? `${LEVEL_COLORS[lvl]}20` : "transparent",
                    color: level === lvl ? LEVEL_COLORS[lvl] : "var(--text-secondary)",
                    cursor: "pointer",
                  }}
                >
                  {lvl}
                </button>
              ))}
            </div>
            <div className="logs-search-wrapper">
              <span className="logs-search-icon">{"\u2315"}</span>
              <input
                className="logs-search-input"
                placeholder="Search"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
            <label className="follow-toggle">
              <input
                type="checkbox"
                checked={follow}
                onChange={(e) => setFollow(e.target.checked)}
              />
              <span className="follow-toggle-track">
                <span className="follow-toggle-thumb" />
              </span>
              <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>Follow</span>
            </label>
          </div>
        </div>
        <div ref={logContainerRef} className="logs-content font-mono">
          {loading ? (
            <div className="logs-empty">Loading logs...</div>
          ) : logs.length === 0 ? (
            <div className="logs-empty">No log entries found.</div>
          ) : (
            <table className="logs-table">
              <tbody>
                {logs.map((entry, i) => (
                  <tr key={i}>
                    <td className="logs-timestamp">{formatTime(entry.timestamp)}</td>
                    <td
                      style={{
                        color: LEVEL_COLORS[entry.level] ?? "var(--text-primary)",
                        fontWeight: entry.level === "ERROR" ? 600 : 400,
                        width: 48,
                        textAlign: "right",
                        paddingRight: 8,
                      }}
                    >
                      {entry.level}
                    </td>
                    <td
                      style={{
                        color: "var(--text-secondary)",
                        paddingRight: 8,
                        maxWidth: 120,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={entry.target}
                    >
                      {shortenTarget(entry.target)}
                    </td>
                    <td style={{ color: LEVEL_COLORS[entry.level] ?? "var(--text-primary)" }}>
                      {entry.message}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
