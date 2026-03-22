import { Fragment, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import type { RunnerInfo } from "../api/types";
import { StatusBadge } from "./StatusBadge";
import { RunnerActions } from "./RunnerActions";
import { RunnerGroupRow } from "./RunnerGroupRow";

// Persists across navigations (module-level)
const persistedExpandedGroups = new Set<string>();

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

function CpuValue({ value }: { value: number | undefined }) {
  if (value == null) return <span className="text-muted font-mono">--</span>;
  const color =
    value > 80
      ? "var(--accent-red)"
      : value > 50
        ? "var(--accent-yellow)"
        : "var(--text-secondary)";
  return (
    <span className="font-mono" style={{ color, fontSize: 13 }}>
      {value.toFixed(1)}%
    </span>
  );
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
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(
    () => new Set(persistedExpandedGroups),
  );

  const toggleGroup = (groupId: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
        persistedExpandedGroups.delete(groupId);
      } else {
        next.add(groupId);
        persistedExpandedGroups.add(groupId);
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
    <div className="runner-list">
      {/* Column header */}
      <div className="runner-list-header">
        <div className="runner-row-grid">
          <div className="runner-col-name">NAME</div>
          <div className="runner-col-repo">REPOSITORY</div>
          <div className="runner-col-status">STATUS</div>
          <div className="runner-col-actions"></div>
        </div>
      </div>

      {/* Groups */}
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
                  <RunnerRow
                    key={runner.config.id}
                    runner={runner}
                    cpuValue={metrics?.get(runner.config.id)}
                    loading={rowLoading}
                    readOnly={readOnly}
                    indented
                    inGroup
                    onStart={onStart}
                    onStop={onStop}
                    onRestart={onRestart}
                    onDelete={onDelete}
                    onClick={() => navigate(`/runners/${runner.config.id}`)}
                  />
                );
              })}
          </Fragment>
        );
      })}

      {/* Solo runners */}
      {soloRunners.map((runner) => (
        <RunnerRow
          key={runner.config.id}
          runner={runner}
          cpuValue={metrics?.get(runner.config.id)}
          loading={pendingActions?.has(runner.config.id)}
          readOnly={readOnly}
          onStart={onStart}
          onStop={onStop}
          onRestart={onRestart}
          onDelete={onDelete}
          onClick={() => navigate(`/runners/${runner.config.id}`)}
        />
      ))}
    </div>
  );
}

function RunnerRow({
  runner,
  cpuValue,
  loading,
  readOnly,
  indented,
  inGroup,
  onStart,
  onStop,
  onRestart,
  onDelete,
  onClick,
}: {
  runner: RunnerInfo;
  cpuValue?: number;
  loading?: boolean;
  readOnly: boolean;
  indented?: boolean;
  inGroup?: boolean;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
  onClick: () => void;
}) {
  return (
    <div
      className={`runner-row ${indented ? "runner-row-indented" : ""}`}
      style={{
        cursor: loading ? "default" : "pointer",
        opacity: loading ? 0.6 : 1,
        pointerEvents: loading ? "none" : undefined,
      }}
      onClick={onClick}
    >
      <div className="runner-row-grid">
        <div className="runner-col-name">
          {indented && <span style={{ width: 20, display: "inline-block" }} />}
          <span className="font-mono" style={{ fontSize: 14, fontWeight: 500 }}>
            {runner.config.name}
          </span>
        </div>
        <div className="runner-col-repo">
          {!inGroup && `${runner.config.repo_owner}/${runner.config.repo_name}`}
        </div>
        <div className="runner-col-status">
          <StatusBadge state={runner.state} />
        </div>
        <div className="runner-col-actions" onClick={(e) => e.stopPropagation()}>
          <CpuValue value={cpuValue} />
          <RunnerActions
            runner={runner}
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
            onDelete={onDelete}
            loading={loading}
            readOnly={readOnly}
          />
        </div>
      </div>
    </div>
  );
}
