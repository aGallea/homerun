import { useState, useRef, useEffect } from "react";
import type { RunnerInfo } from "../api/types";
import { ConfirmDialog } from "./ConfirmDialog";

interface RunnerActionsProps {
  runner: RunnerInfo;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
}

export function RunnerActions({
  runner,
  onStart,
  onStop,
  onRestart,
  onDelete,
}: RunnerActionsProps) {
  const [open, setOpen] = useState(false);
  const [confirm, setConfirm] = useState<"delete" | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const isRunning = runner.state === "online" || runner.state === "busy";
  const isStopped = runner.state === "offline" || runner.state === "error";

  return (
    <div className="runner-actions" ref={menuRef}>
      <button className="btn btn-sm" onClick={() => setOpen(!open)}>
        ⋯
      </button>
      {open && (
        <div className="actions-menu">
          {isStopped && (
            <button
              className="actions-item"
              onClick={() => {
                onStart(runner.config.id);
                setOpen(false);
              }}
            >
              Start
            </button>
          )}
          {isRunning && (
            <button
              className="actions-item"
              onClick={() => {
                onStop(runner.config.id);
                setOpen(false);
              }}
            >
              Stop
            </button>
          )}
          {isRunning && (
            <button
              className="actions-item"
              onClick={() => {
                onRestart(runner.config.id);
                setOpen(false);
              }}
            >
              Restart
            </button>
          )}
          <button
            className="actions-item actions-item-danger"
            onClick={() => {
              setConfirm("delete");
              setOpen(false);
            }}
          >
            Delete
          </button>
        </div>
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
