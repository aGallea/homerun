import { Fragment, useState, useMemo, useEffect } from "react";
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

function SvcBadge() {
  return (
    <span
      title="Service runner"
      style={{
        display: "inline-flex",
        alignItems: "center",
        marginRight: 6,
        color: "var(--accent-blue)",
        opacity: 0.8,
      }}
    >
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <rect x="2" y="2" width="20" height="8" rx="2" />
        <rect x="2" y="14" width="20" height="8" rx="2" />
        <circle cx="6" cy="6" r="1" fill="currentColor" />
        <circle cx="6" cy="18" r="1" fill="currentColor" />
      </svg>
    </span>
  );
}

function CpuValue({ value }: { value: number | undefined }) {
  if (value == null) return null;
  const color =
    value > 80
      ? "var(--accent-red)"
      : value > 50
        ? "var(--accent-yellow)"
        : "var(--text-secondary)";
  return (
    <span
      className="font-mono"
      style={{
        color,
        fontSize: 11,
        border: `0.1px solid ${color}`,
        borderRadius: 4,
        width: 45,
        height: 30,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
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
    const byName = (a: RunnerInfo, b: RunnerInfo) =>
      a.config.name.localeCompare(b.config.name, undefined, { numeric: true });

    // Group by name prefix + repo (merges runners from separate batch creates)
    const mergedMap = new Map<string, RunnerInfo[]>();
    const solo: RunnerInfo[] = [];
    for (const runner of runners) {
      if (runner.config.group_id) {
        const prefix = runner.config.name.replace(/-\d+$/, "");
        const repo = `${runner.config.repo_owner}/${runner.config.repo_name}`;
        const key = `${prefix}::${repo}`;
        const existing = mergedMap.get(key) ?? [];
        existing.push(runner);
        mergedMap.set(key, existing);
      } else {
        solo.push(runner);
      }
    }
    // Sort runners within each group and solo runners by name (numeric-aware)
    for (const group of mergedMap.values()) group.sort(byName);
    solo.sort(byName);
    return { groups: mergedMap, soloRunners: solo };
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
      {/* Groups */}
      {Array.from(groups.entries()).map(([groupKey, groupRunners]) => {
        const isExpanded = effectiveExpanded.has(groupKey);
        const groupIds = [
          ...new Set(groupRunners.map((r) => r.config.group_id).filter(Boolean)),
        ] as string[];
        const firstGroupId = groupIds[0] ?? groupKey;
        const isLoading =
          pendingActions?.has(groupKey) || groupIds.some((gid) => pendingActions?.has(gid));
        return (
          <Fragment key={`group-${groupKey}`}>
            <RunnerGroupRow
              groupId={firstGroupId}
              groupIds={groupIds}
              runners={groupRunners}
              expanded={isExpanded}
              onToggle={() => toggleGroup(groupKey)}
              onStartGroup={onStartGroup}
              onStopGroup={onStopGroup}
              onRestartGroup={onRestartGroup}
              onDeleteGroup={onDeleteGroup}
              onScaleGroup={onScaleGroup}
              loading={isLoading}
              readOnly={readOnly}
            />
            {isExpanded &&
              groupRunners.map((runner) => {
                const rowLoading =
                  pendingActions?.has(runner.config.id) ||
                  groupIds.some((gid) => pendingActions?.has(gid));
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

function MiniProgressBar({
  estimatedDurationSecs,
  jobStartedAt,
}: {
  estimatedDurationSecs: number;
  jobStartedAt: string;
}) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 2000);
    return () => clearInterval(interval);
  }, []);

  const elapsedSecs = (Date.now() - new Date(jobStartedAt).getTime()) / 1000;
  const progress = Math.min(elapsedSecs / estimatedDurationSecs, 0.99);
  const percent = Math.round(progress * 100);
  const exceeding = elapsedSecs > estimatedDurationSecs;

  return (
    <div className="runner-progress-mini">
      <div
        className="runner-progress-mini-fill"
        style={{
          width: `${percent}%`,
          background: exceeding ? "var(--accent-yellow)" : "var(--accent-blue)",
        }}
      />
    </div>
  );
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  if (m < 60) return s > 0 ? `${m}m ${s}s` : `${m}m`;
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`;
}

function LastJobSummary({ runner }: { runner: RunnerInfo }) {
  const job = runner.last_completed_job;
  if (!job) return null;

  const icon = job.succeeded ? "\u2713" : "\u2717";
  const iconColor = job.succeeded ? "var(--accent-green)" : "var(--accent-red)";

  const nameDisplay =
    job.job_name.length > 20 ? job.job_name.slice(0, 20) + "\u2026" : job.job_name;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 4,
        fontSize: 11,
        color: "var(--text-secondary)",
        whiteSpace: "nowrap",
        overflow: "hidden",
      }}
      title={`Last job: ${job.job_name} — ${job.succeeded ? "succeeded" : "failed"} in ${formatDuration(job.duration_secs)}${job.latest_attempt ? ` (re-run ${job.latest_attempt.succeeded ? "succeeded" : "failed"} on ${job.latest_attempt.runner_name})` : ""}`}
    >
      <span style={{ color: iconColor, fontWeight: 700, flexShrink: 0 }}>{icon}</span>
      <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{nameDisplay}</span>
      {job.latest_attempt && (
        <>
          <span style={{ flexShrink: 0, opacity: 0.5 }}>&middot;</span>
          <span
            style={{
              fontSize: 9,
              fontWeight: 600,
              padding: "0px 4px",
              borderRadius: 3,
              background: job.latest_attempt.succeeded
                ? "rgba(34, 197, 94, 0.15)"
                : "rgba(239, 68, 68, 0.15)",
              color: job.latest_attempt.succeeded ? "var(--accent-green)" : "var(--accent-red)",
              flexShrink: 0,
            }}
          >
            Re-run: {job.latest_attempt.succeeded ? "\u2713" : "\u2717"}
          </span>
        </>
      )}
      {!job.latest_attempt && (
        <>
          <span style={{ flexShrink: 0, opacity: 0.5 }}>&middot;</span>
          <span style={{ flexShrink: 0 }}>{formatDuration(job.duration_secs)}</span>
        </>
      )}
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
          <div style={{ display: "flex", alignItems: "center" }}>
            {indented && (
              <span
                style={{
                  width: 28,
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  flexShrink: 0,
                }}
              >
                {runner.config.mode === "service" && <SvcBadge />}
              </span>
            )}
            {!indented && runner.config.mode === "service" && <SvcBadge />}
            <span className="font-mono" style={{ fontSize: 14, fontWeight: 500 }}>
              {runner.config.name}
            </span>
          </div>
          {!inGroup && (
            <div
              style={{
                fontSize: 11,
                color: "var(--text-secondary)",
                marginTop: 1,
                paddingLeft: runner.config.mode === "service" ? 32 : 0,
              }}
            >
              {runner.config.repo_owner}/{runner.config.repo_name}
            </div>
          )}
        </div>
        <div
          className="runner-col-status"
          title={
            runner.error_message ?? (runner.current_job ? `Busy: ${runner.current_job}` : undefined)
          }
        >
          <StatusBadge state={runner.state} currentJob={runner.current_job ?? undefined} />
        </div>
        <div className="runner-col-actions" onClick={(e) => e.stopPropagation()}>
          {runner.state === "busy" &&
          runner.estimated_job_duration_secs != null &&
          runner.job_started_at ? (
            <MiniProgressBar
              estimatedDurationSecs={runner.estimated_job_duration_secs}
              jobStartedAt={runner.job_started_at}
            />
          ) : (
            runner.state !== "busy" && <LastJobSummary runner={runner} />
          )}
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
