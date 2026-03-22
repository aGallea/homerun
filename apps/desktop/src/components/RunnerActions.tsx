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
}

export function RunnerActions({
  runner,
  onStart,
  onStop,
  onRestart,
  onDelete,
  loading = false,
}: RunnerActionsProps) {
  const [confirm, setConfirm] = useState<"delete" | null>(null);

  const isRunning = runner.state === "online" || runner.state === "busy";
  const isStopped = runner.state === "offline" || runner.state === "error";

  // Ghost/outline style for individual runner buttons (visually distinct from group buttons)
  const ghostStyle: React.CSSProperties = {
    opacity: loading ? 0.4 : 0.7,
    fontSize: 11,
    padding: "2px 4px",
    minWidth: 22,
    height: 22,
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
  };

  return (
    <div style={{ display: "flex", gap: 3, alignItems: "center" }}>
      {loading && <Spinner />}
      {isStopped && (
        <button
          className="btn btn-sm"
          style={ghostStyle}
          onClick={() => onStart(runner.config.id)}
          title="Start"
          disabled={loading}
        >
          ▶
        </button>
      )}
      {isRunning && (
        <button
          className="btn btn-sm"
          style={ghostStyle}
          onClick={() => onStop(runner.config.id)}
          title="Stop"
          disabled={loading}
        >
          ■
        </button>
      )}
      {isRunning && (
        <button
          className="btn btn-sm"
          style={ghostStyle}
          onClick={() => onRestart(runner.config.id)}
          title="Restart"
          disabled={loading}
        >
          ↻
        </button>
      )}
      <button
        className="btn btn-sm"
        style={{ ...ghostStyle, color: loading ? undefined : "var(--accent-red)" }}
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
