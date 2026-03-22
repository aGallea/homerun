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
    opacity: 0.7,
    fontSize: 12,
    padding: "2px 6px",
    minWidth: 28,
    height: 24,
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
  };

  return (
    <div style={{ display: "flex", gap: 3, alignItems: "center" }}>
      {loading ? (
        <span className="text-muted" style={{ fontSize: 12 }}>
          ...
        </span>
      ) : (
        <>
          {isStopped && (
            <button
              className="btn btn-sm"
              style={ghostStyle}
              onClick={() => onStart(runner.config.id)}
              title="Start"
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
            >
              ↻
            </button>
          )}
          <button
            className="btn btn-sm"
            style={{ ...ghostStyle, color: "var(--accent-red)" }}
            onClick={() => setConfirm("delete")}
            title="Delete"
          >
            ✕
          </button>
        </>
      )}
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
