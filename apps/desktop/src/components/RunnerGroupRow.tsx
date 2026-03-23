import { useState, useRef, useEffect } from "react";
import type { RunnerInfo } from "../api/types";
import { ConfirmDialog } from "./ConfirmDialog";

interface RunnerGroupRowProps {
  groupId: string;
  groupIds?: string[];
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
  groupIds,
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
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const namePrefix = runners[0]?.config.name.replace(/-\d+$/, "") ?? "group";
  const repo = runners[0] ? `${runners[0].config.repo_owner}/${runners[0].config.repo_name}` : "";
  const allGroupIds = groupIds ?? [groupId];
  const forEachGroup = (fn: (gid: string) => void) => allGroupIds.forEach(fn);

  const activeCount = runners.filter((r) => r.state === "online" || r.state === "busy").length;
  const hasRunning = activeCount > 0;
  const hasStopped = runners.some((r) => r.state === "offline" || r.state === "error");

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
          <div className="runner-col-cpu"></div>
          <div className="runner-col-actions" onClick={(e) => e.stopPropagation()}>
            {!readOnly && (
              <>
                {/* Inline buttons — hidden on small screens */}
                <div className="runner-actions-bar actions-inline" style={{ marginLeft: "auto" }}>
                  {loading && <Spinner />}
                  {hasStopped && (
                    <button
                      className="icon-btn"
                      onClick={() => forEachGroup(onStartGroup)}
                      title="Start all"
                      disabled={loading}
                    >
                      ▶
                    </button>
                  )}
                  {hasRunning && (
                    <button
                      className="icon-btn"
                      onClick={() => forEachGroup(onStopGroup)}
                      title="Stop all"
                      disabled={loading}
                    >
                      ■
                    </button>
                  )}
                  <button
                    className="icon-btn"
                    onClick={() => forEachGroup(onRestartGroup)}
                    title="Restart all"
                    disabled={loading}
                  >
                    ↻
                  </button>
                  <button
                    className="icon-btn"
                    onClick={() => onScaleGroup(allGroupIds[0], runners.length + 1)}
                    title="Scale up"
                    disabled={loading || runners.length >= 10}
                  >
                    ▲
                  </button>
                  <button
                    className="icon-btn"
                    onClick={() => onScaleGroup(allGroupIds[0], runners.length - 1)}
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

                {/* Dots menu — shown on small screens */}
                <div className="actions-dots" style={{ marginLeft: "auto" }} ref={menuRef}>
                  <button
                    className="icon-btn"
                    onClick={() => setMenuOpen((v) => !v)}
                    disabled={loading}
                  >
                    ⋯
                  </button>
                  {menuOpen && (
                    <div className="actions-dropdown">
                      {hasStopped && (
                        <button
                          className="actions-dropdown-item"
                          onClick={() => {
                            forEachGroup(onStartGroup);
                            setMenuOpen(false);
                          }}
                        >
                          ▶ Start All
                        </button>
                      )}
                      {hasRunning && (
                        <button
                          className="actions-dropdown-item"
                          onClick={() => {
                            forEachGroup(onStopGroup);
                            setMenuOpen(false);
                          }}
                        >
                          ■ Stop All
                        </button>
                      )}
                      <button
                        className="actions-dropdown-item"
                        onClick={() => {
                          forEachGroup(onRestartGroup);
                          setMenuOpen(false);
                        }}
                      >
                        ↻ Restart All
                      </button>
                      <button
                        className="actions-dropdown-item"
                        onClick={() => {
                          onScaleGroup(allGroupIds[0], runners.length + 1);
                          setMenuOpen(false);
                        }}
                        disabled={runners.length >= 10}
                      >
                        ▲ Scale Up
                      </button>
                      <button
                        className="actions-dropdown-item"
                        onClick={() => {
                          onScaleGroup(allGroupIds[0], runners.length - 1);
                          setMenuOpen(false);
                        }}
                        disabled={runners.length <= 1}
                      >
                        ▼ Scale Down
                      </button>
                      <button
                        className="actions-dropdown-item actions-dropdown-item-danger"
                        onClick={() => {
                          setConfirmDelete(true);
                          setMenuOpen(false);
                        }}
                      >
                        ✕ Delete All
                      </button>
                    </div>
                  )}
                </div>
              </>
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
            forEachGroup(onDeleteGroup);
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
