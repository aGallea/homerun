import { Fragment, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import type { RunnerInfo } from "../api/types";
import { StatusBadge } from "./StatusBadge";
import { RunnerActions } from "./RunnerActions";
import { RunnerGroupRow } from "./RunnerGroupRow";

interface RunnerTableProps {
  runners: RunnerInfo[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
  onStartGroup: (groupId: string) => void;
  onStopGroup: (groupId: string) => void;
  onRestartGroup: (groupId: string) => void;
  onDeleteGroup: (groupId: string) => void;
  onScaleGroup: (groupId: string, count: number) => void;
  metrics?: Map<string, number>;
  forceExpandedGroups?: Set<string>;
  pendingActions?: Set<string>;
  readOnly?: boolean;
}

export function RunnerTable({
  runners,
  onStart,
  onStop,
  onRestart,
  onDelete,
  onStartGroup,
  onStopGroup,
  onRestartGroup,
  onDeleteGroup,
  onScaleGroup,
  metrics,
  forceExpandedGroups,
  pendingActions,
  readOnly = false,
}: RunnerTableProps) {
  const navigate = useNavigate();
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  const toggleGroup = (groupId: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  };

  const effectiveExpanded = useMemo(() => {
    if (!forceExpandedGroups || forceExpandedGroups.size === 0) return expandedGroups;
    const merged = new Set(expandedGroups);
    for (const gid of forceExpandedGroups) merged.add(gid);
    return merged;
  }, [expandedGroups, forceExpandedGroups]);

  const { groups, soloRunners } = useMemo(() => {
    const groupMap = new Map<string, RunnerInfo[]>();
    const solo: RunnerInfo[] = [];
    for (const runner of runners) {
      if (runner.config.group_id) {
        const existing = groupMap.get(runner.config.group_id) ?? [];
        existing.push(runner);
        groupMap.set(runner.config.group_id, existing);
      } else {
        solo.push(runner);
      }
    }
    return { groups: groupMap, soloRunners: solo };
  }, [runners]);

  if (runners.length === 0) {
    return (
      <div className="card" style={{ textAlign: "center", padding: "40px" }}>
        <p className="text-muted">No runners yet.</p>
        <p className="text-muted" style={{ marginTop: 8, fontSize: 12 }}>
          Click "+ New Runner" to get started.
        </p>
      </div>
    );
  }

  return (
    <div className="card" style={{ padding: 0, overflow: "visible" }}>
      <table className="table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Repository</th>
            <th>Status</th>
            <th>Current Job</th>
            <th>Mode</th>
            <th>CPU</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {Array.from(groups.entries()).map(([groupId, groupRunners]) => {
            const isExpanded = effectiveExpanded.has(groupId);
            return (
              <Fragment key={`group-${groupId}`}>
                <RunnerGroupRow
                  groupId={groupId}
                  runners={groupRunners}
                  expanded={isExpanded}
                  onToggle={() => toggleGroup(groupId)}
                  onStartGroup={onStartGroup}
                  onStopGroup={onStopGroup}
                  onRestartGroup={onRestartGroup}
                  onDeleteGroup={onDeleteGroup}
                  onScaleGroup={onScaleGroup}
                  loading={pendingActions?.has(groupId)}
                  readOnly={readOnly}
                />
                {isExpanded &&
                  groupRunners.map((runner) => {
                    const rowLoading =
                      pendingActions?.has(runner.config.id) || pendingActions?.has(groupId);
                    return (
                      <tr
                        key={runner.config.id}
                        style={{
                          cursor: rowLoading ? "default" : "pointer",
                          opacity: rowLoading ? 0.6 : 1,
                          pointerEvents: rowLoading ? "none" : undefined,
                        }}
                        onClick={() => navigate(`/runners/${runner.config.id}`)}
                      >
                        <td style={{ whiteSpace: "nowrap" }}>
                          <span className="font-mono" style={{ fontSize: 13, paddingLeft: 24 }}>
                            {runner.config.name}
                          </span>
                        </td>
                        <td className="text-muted">
                          {runner.config.repo_owner}/{runner.config.repo_name}
                        </td>
                        <td>
                          <StatusBadge state={runner.state} />
                        </td>
                        <td>
                          {runner.current_job ? (
                            <div>
                              <a
                                href="#"
                                onClick={(e) => {
                                  e.preventDefault();
                                  e.stopPropagation();
                                  const url =
                                    runner.job_context?.run_url ??
                                    `https://github.com/${runner.config.repo_owner}/${runner.config.repo_name}/actions?query=is%3Ain_progress`;
                                  import("@tauri-apps/plugin-shell").then(({ open }) => {
                                    open(url);
                                  });
                                }}
                                style={{
                                  color: "var(--accent-yellow)",
                                  fontSize: 12,
                                  cursor: "pointer",
                                }}
                              >
                                {runner.current_job}
                              </a>
                              {runner.job_context && (
                                <div
                                  style={{
                                    fontSize: 11,
                                    color: "var(--text-secondary)",
                                    marginTop: 2,
                                  }}
                                >
                                  {runner.job_context.branch}
                                  {runner.job_context.pr_number != null && (
                                    <a
                                      href="#"
                                      onClick={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        if (runner.job_context?.pr_url) {
                                          import("@tauri-apps/plugin-shell").then(({ open }) => {
                                            open(runner.job_context!.pr_url!);
                                          });
                                        }
                                      }}
                                      style={{
                                        color: "var(--accent-blue)",
                                        marginLeft: 6,
                                        cursor: "pointer",
                                      }}
                                    >
                                      PR #{runner.job_context.pr_number}
                                    </a>
                                  )}
                                </div>
                              )}
                            </div>
                          ) : (
                            <span className="text-muted" style={{ fontSize: 12 }}>
                              —
                            </span>
                          )}
                        </td>
                        <td className="text-muted" style={{ textTransform: "capitalize" }}>
                          {runner.config.mode}
                        </td>
                        <td className="font-mono text-muted">
                          {metrics?.get(runner.config.id) != null
                            ? `${metrics.get(runner.config.id)!.toFixed(1)}%`
                            : "--"}
                        </td>
                        <td onClick={(e) => e.stopPropagation()}>
                          <RunnerActions
                            runner={runner}
                            onStart={onStart}
                            onStop={onStop}
                            onRestart={onRestart}
                            onDelete={onDelete}
                            loading={rowLoading}
                            readOnly={readOnly}
                          />
                        </td>
                      </tr>
                    );
                  })}
              </Fragment>
            );
          })}
          {soloRunners.map((runner) => (
            <tr
              key={runner.config.id}
              style={{
                cursor: pendingActions?.has(runner.config.id) ? "default" : "pointer",
                opacity: pendingActions?.has(runner.config.id) ? 0.6 : 1,
                pointerEvents: pendingActions?.has(runner.config.id) ? "none" : undefined,
              }}
              onClick={() => navigate(`/runners/${runner.config.id}`)}
            >
              <td style={{ whiteSpace: "nowrap" }}>
                <span className="font-mono" style={{ fontSize: 13 }}>
                  {runner.config.name}
                </span>
              </td>
              <td className="text-muted">
                {runner.config.repo_owner}/{runner.config.repo_name}
              </td>
              <td>
                <StatusBadge state={runner.state} />
              </td>
              <td>
                {runner.current_job ? (
                  <div>
                    <a
                      href="#"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        const url =
                          runner.job_context?.run_url ??
                          `https://github.com/${runner.config.repo_owner}/${runner.config.repo_name}/actions?query=is%3Ain_progress`;
                        import("@tauri-apps/plugin-shell").then(({ open }) => {
                          open(url);
                        });
                      }}
                      style={{ color: "var(--accent-yellow)", fontSize: 12, cursor: "pointer" }}
                    >
                      {runner.current_job}
                    </a>
                    {runner.job_context && (
                      <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>
                        {runner.job_context.branch}
                        {runner.job_context.pr_number != null && (
                          <a
                            href="#"
                            onClick={(e) => {
                              e.preventDefault();
                              e.stopPropagation();
                              if (runner.job_context?.pr_url) {
                                import("@tauri-apps/plugin-shell").then(({ open }) => {
                                  open(runner.job_context!.pr_url!);
                                });
                              }
                            }}
                            style={{
                              color: "var(--accent-blue)",
                              marginLeft: 6,
                              cursor: "pointer",
                            }}
                          >
                            PR #{runner.job_context.pr_number}
                          </a>
                        )}
                      </div>
                    )}
                  </div>
                ) : (
                  <span className="text-muted" style={{ fontSize: 12 }}>
                    —
                  </span>
                )}
              </td>
              <td className="text-muted" style={{ textTransform: "capitalize" }}>
                {runner.config.mode}
              </td>
              <td className="font-mono text-muted">
                {metrics?.get(runner.config.id) != null
                  ? `${metrics.get(runner.config.id)!.toFixed(1)}%`
                  : "--"}
              </td>
              <td onClick={(e) => e.stopPropagation()}>
                <RunnerActions
                  runner={runner}
                  onStart={onStart}
                  onStop={onStop}
                  onRestart={onRestart}
                  onDelete={onDelete}
                  loading={pendingActions?.has(runner.config.id)}
                  readOnly={readOnly}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
