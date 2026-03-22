import { useState, useRef, useEffect } from "react";
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
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const isRunning = runner.state === "online" || runner.state === "busy";
  const isStopped = runner.state === "offline" || runner.state === "error";

  useEffect(() => {
    if (!menuOpen) return;
    function handleClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [menuOpen]);

  return (
    <>
      {/* Inline buttons — hidden on small screens */}
      <div className="runner-actions-bar actions-inline">
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
      </div>

      {/* Dots menu — shown on small screens */}
      <div className="actions-dots" ref={menuRef}>
        <button className="icon-btn" onClick={() => setMenuOpen((v) => !v)} disabled={loading}>
          ⋯
        </button>
        {menuOpen && (
          <div className="actions-dropdown">
            {isStopped && (
              <button
                className="actions-dropdown-item"
                onClick={() => {
                  onStart(runner.config.id);
                  setMenuOpen(false);
                }}
              >
                ▶ Start
              </button>
            )}
            {isRunning && (
              <button
                className="actions-dropdown-item"
                onClick={() => {
                  onStop(runner.config.id);
                  setMenuOpen(false);
                }}
              >
                ■ Stop
              </button>
            )}
            <button
              className="actions-dropdown-item"
              onClick={() => {
                onRestart(runner.config.id);
                setMenuOpen(false);
              }}
            >
              ↻ Restart
            </button>
            <button
              className="actions-dropdown-item actions-dropdown-item-danger"
              onClick={() => {
                setConfirm("delete");
                setMenuOpen(false);
              }}
            >
              ✕ Delete
            </button>
          </div>
        )}
      </div>

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
