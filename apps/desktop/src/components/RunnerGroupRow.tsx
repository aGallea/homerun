import { useState } from "react";
import type { RunnerInfo, RunnerState } from "../api/types";
import { StatusBadge } from "./StatusBadge";
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
}: RunnerGroupRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  const namePrefix = runners[0]?.config.name.replace(/-\d+$/, "") ?? "group";
  const repo = runners[0] ? `${runners[0].config.repo_owner}/${runners[0].config.repo_name}` : "";

  const statusCounts = new Map<string, number>();
  for (const r of runners) {
    statusCounts.set(r.state, (statusCounts.get(r.state) ?? 0) + 1);
  }

  const hasRunning = runners.some((r) => r.state === "online" || r.state === "busy");
  const hasStopped = runners.some((r) => r.state === "offline" || r.state === "error");

  return (
    <>
      <tr
        className="group-row"
        onClick={loading ? undefined : onToggle}
        style={{
          cursor: loading ? "default" : "pointer",
          opacity: loading ? 0.6 : 1,
        }}
      >
        <td style={{ whiteSpace: "nowrap" }}>
          <span style={{ marginRight: 8 }}>{expanded ? "▼" : "▶"}</span>
          <span className="font-mono" style={{ fontWeight: 600 }}>
            {namePrefix}
          </span>
          <span className="text-muted" style={{ marginLeft: 8 }}>
            ({runners.length})
          </span>
        </td>
        <td className="text-muted">{repo}</td>
        <td>
          {Array.from(statusCounts.entries()).map(([state]) => (
            <span key={state} style={{ marginRight: 6 }}>
              <StatusBadge state={state as RunnerState} />
            </span>
          ))}
        </td>
        <td></td>
        <td></td>
        <td></td>
        <td onClick={(e) => e.stopPropagation()}>
          <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
            {loading && <Spinner />}
            {hasStopped && (
              <button
                className="btn btn-sm"
                onClick={() => onStartGroup(groupId)}
                title="Start all"
                disabled={loading}
              >
                ▶
              </button>
            )}
            {hasRunning && (
              <button
                className="btn btn-sm"
                onClick={() => onStopGroup(groupId)}
                title="Stop all"
                disabled={loading}
              >
                ■
              </button>
            )}
            <button
              className="btn btn-sm"
              onClick={() => onRestartGroup(groupId)}
              title="Restart all"
              disabled={loading}
            >
              ↻
            </button>
            <button
              className="btn btn-sm"
              onClick={() => onScaleGroup(groupId, runners.length + 1)}
              title="Scale up"
              disabled={loading || runners.length >= 10}
            >
              +
            </button>
            <button
              className="btn btn-sm"
              onClick={() => onScaleGroup(groupId, runners.length - 1)}
              title="Scale down"
              disabled={loading || runners.length <= 1}
            >
              −
            </button>
            <button
              className="btn btn-sm"
              style={{
                color: loading ? undefined : "var(--accent-red)",
                opacity: loading ? 0.4 : 1,
              }}
              onClick={() => setConfirmDelete(true)}
              title="Delete all"
              disabled={loading}
            >
              ✕
            </button>
          </div>
        </td>
      </tr>
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
