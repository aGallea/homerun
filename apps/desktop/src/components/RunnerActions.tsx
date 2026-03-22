import { useState } from "react";
import type { RunnerInfo } from "../api/types";
import { ConfirmDialog } from "./ConfirmDialog";

interface RunnerActionsProps {
  runner: RunnerInfo;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
  loading?: boolean;
  readOnly?: boolean;
}

export function RunnerActions({
  runner,
  onStart,
  onStop,
  onRestart,
  onDelete,
  loading = false,
  readOnly = false,
}: RunnerActionsProps) {
  if (readOnly) return null;
  const [confirm, setConfirm] = useState<"delete" | null>(null);

  const isRunning = runner.state === "online" || runner.state === "busy";
  const isStopped = runner.state === "offline" || runner.state === "error";

  return (
    <div className="runner-actions-bar">
      {loading && <Spinner />}
      {isStopped && (
        <button
          className="icon-btn"
          onClick={() => onStart(runner.config.id)}
          title="Start"
          disabled={loading}
        >
          ▶
        </button>
      )}
      {isRunning && (
        <button
          className="icon-btn"
          onClick={() => onStop(runner.config.id)}
          title="Stop"
          disabled={loading}
        >
          ■
        </button>
      )}
      <button
        className="icon-btn"
        onClick={() => onRestart(runner.config.id)}
        title="Restart"
        disabled={loading}
      >
        ↻
      </button>
      <button
        className="icon-btn icon-btn-danger"
        onClick={() => setConfirm("delete")}
        title="Delete"
        disabled={loading}
      >
        ✕
      </button>
      {confirm === "delete" && (
        <ConfirmDialog
          title="Delete Runner"
          message={`Are you sure you want to delete "${runner.config.name}"? This will stop the runner, deregister it from GitHub, and remove its local data.`}
          confirmLabel="Delete Runner"
          danger
          onConfirm={() => {
            onDelete(runner.config.id);
            setConfirm(null);
          }}
          onCancel={() => setConfirm(null)}
        />
      )}
    </div>
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
