import { useState } from "react";
import type { RunnerInfo } from "../api/types";
import { ConfirmDialog } from "./ConfirmDialog";

interface RunnerGroupRowProps {
  groupId: string;
  runners: RunnerInfo[];
  expanded: boolean;
  onToggle: () => void;
  onStartGroup: (groupId: string) => void;
  onStopGroup: (groupId: string) => void;
  onRestartGroup: (groupId: string) => void;
  onDeleteGroup: (groupId: string) => void;
  onScaleGroup: (groupId: string, count: number) => void;
  loading?: boolean;
  readOnly?: boolean;
}

export function RunnerGroupRow({
  groupId,
  runners,
  expanded,
  onToggle,
  onStartGroup,
  onStopGroup,
  onRestartGroup,
  onDeleteGroup,
  onScaleGroup,
  loading = false,
  readOnly = false,
}: RunnerGroupRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  const namePrefix = runners[0]?.config.name.replace(/-\d+$/, "") ?? "group";
  const repo = runners[0] ? `${runners[0].config.repo_owner}/${runners[0].config.repo_name}` : "";

  const activeCount = runners.filter((r) => r.state === "online" || r.state === "busy").length;
  const hasRunning = activeCount > 0;
  const hasStopped = runners.some((r) => r.state === "offline" || r.state === "error");

  return (
    <>
      <div
        className="runner-row runner-row-group"
        onClick={loading ? undefined : onToggle}
        style={{
          cursor: loading ? "default" : "pointer",
          opacity: loading ? 0.6 : 1,
        }}
      >
        <div className="runner-row-grid">
          <div className="runner-col-name">
            <span className="runner-expand-icon">{expanded ? "▼" : "▶"}</span>
            <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 14 }}>
              {namePrefix}
            </span>
            <span className="text-muted">({runners.length})</span>
          </div>
          <div className="runner-col-repo">{repo}</div>
          <div className="runner-col-status">
            <span
              className="status-badge"
              style={{ color: activeCount > 0 ? "var(--accent-green)" : "var(--text-secondary)" }}
            >
              <span
                className="status-dot"
                style={{
                  background: activeCount > 0 ? "var(--accent-green)" : "var(--text-secondary)",
                }}
              />
              {activeCount}/{runners.length} Online
            </span>
          </div>
          <div className="runner-col-actions" onClick={(e) => e.stopPropagation()}>
            {!readOnly && (
              <div className="runner-actions-bar" style={{ marginLeft: "auto" }}>
                {loading && <Spinner />}
                {hasStopped && (
                  <button
                    className="icon-btn"
                    onClick={() => onStartGroup(groupId)}
                    title="Start all"
                    disabled={loading}
                  >
                    ▶
                  </button>
                )}
                {hasRunning && (
                  <button
                    className="icon-btn"
                    onClick={() => onStopGroup(groupId)}
                    title="Stop all"
                    disabled={loading}
                  >
                    ■
                  </button>
                )}
                <button
                  className="icon-btn"
                  onClick={() => onRestartGroup(groupId)}
                  title="Restart all"
                  disabled={loading}
                >
                  ↻
                </button>
                <button
                  className="icon-btn"
                  onClick={() => onScaleGroup(groupId, runners.length + 1)}
                  title="Scale up"
                  disabled={loading || runners.length >= 10}
                >
                  ▲
                </button>
                <button
                  className="icon-btn"
                  onClick={() => onScaleGroup(groupId, runners.length - 1)}
                  title="Scale down"
                  disabled={loading || runners.length <= 1}
                >
                  ▼
                </button>
                <button
                  className="icon-btn icon-btn-danger"
                  onClick={() => setConfirmDelete(true)}
                  title="Delete all"
                  disabled={loading}
                >
                  ✕
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
      {confirmDelete && (
        <ConfirmDialog
          title="Delete Group"
          message={`Delete all ${runners.length} runners in this group? Busy runners will be skipped.`}
          confirmLabel="Delete All"
          danger
          onConfirm={() => {
            onDeleteGroup(groupId);
            setConfirmDelete(false);
          }}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </>
  );
}

function Spinner() {
  return (
    <span
      style={{
        display: "inline-block",
        width: 14,
        height: 14,
        border: "2px solid var(--border)",
        borderTopColor: "var(--text-muted)",
        borderRadius: "50%",
        animation: "spin 0.6s linear infinite",
      }}
    />
  );
}
