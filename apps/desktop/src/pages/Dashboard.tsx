import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useRunners } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { useAuth } from "../hooks/useAuth";
import { StatsCard } from "../components/StatsCard";
import { RunnerTable } from "../components/RunnerTable";
import { NewRunnerWizard } from "../components/NewRunnerWizard";

export function Dashboard() {
  const { auth } = useAuth();
  const navigate = useNavigate();
  const isAuthenticated = auth.authenticated;
  const {
    runners,
    loading,
    pendingActions,
    startRunner,
    stopRunner,
    restartRunner,
    deleteRunner,
    createRunner,
    createBatch,
    startGroup,
    stopGroup,
    restartGroup,
    deleteGroup,
    scaleGroup,
  } = useRunners();
  const { metrics } = useMetrics();
  const [showWizard, setShowWizard] = useState(false);
  const [filter, setFilter] = useState("");

  const online = runners.filter((r) => r.state === "online" || r.state === "busy").length;
  const busy = runners.filter((r) => r.state === "busy").length;

  const cpuMap = new Map<string, number>();
  metrics?.runners.forEach((m) => cpuMap.set(m.runner_id, m.cpu_percent));
  const avgCpu =
    metrics && metrics.runners.length > 0
      ? metrics.runners.reduce((sum, r) => sum + r.cpu_percent, 0) / metrics.runners.length
      : 0;

  const filtered = useMemo(() => {
    if (!filter) return runners;
    const q = filter.toLowerCase();

    // Find group IDs where any member matches
    const matchingGroupIds = new Set<string>();
    for (const runner of runners) {
      if (runner.config.group_id) {
        const nameMatch = runner.config.name.toLowerCase().includes(q);
        const repoMatch = `${runner.config.repo_owner}/${runner.config.repo_name}`
          .toLowerCase()
          .includes(q);
        // Also match on group name prefix
        const prefix = runner.config.name.replace(/-\d+$/, "").toLowerCase();
        const prefixMatch = prefix.includes(q);
        if (nameMatch || repoMatch || prefixMatch) {
          matchingGroupIds.add(runner.config.group_id);
        }
      }
    }

    return runners.filter((r) => {
      // Include all runners from matching groups
      if (r.config.group_id && matchingGroupIds.has(r.config.group_id)) return true;
      // Include matching solo runners
      if (!r.config.group_id) {
        return (
          r.config.name.toLowerCase().includes(q) ||
          `${r.config.repo_owner}/${r.config.repo_name}`.toLowerCase().includes(q)
        );
      }
      return false;
    });
  }, [runners, filter]);

  const forceExpandedGroups = useMemo(() => {
    if (!filter) return new Set<string>();
    const forced = new Set<string>();
    const q = filter.toLowerCase();
    for (const runner of runners) {
      if (
        runner.config.group_id &&
        (runner.config.name.toLowerCase().includes(q) ||
          `${runner.config.repo_owner}/${runner.config.repo_name}`.toLowerCase().includes(q))
      ) {
        forced.add(runner.config.group_id);
      }
    }
    return forced;
  }, [runners, filter]);

  if (loading) {
    return (
      <div className="page">
        <p className="text-muted">Loading...</p>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <div className="flex items-center gap-8">
          <input
            className="input"
            placeholder="Filter runners..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          {isAuthenticated && (
            <button className="btn btn-primary" onClick={() => setShowWizard(true)}>
              + New Runner
            </button>
          )}
        </div>
      </div>

      {!isAuthenticated && (
        <div
          className="card"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 20,
            padding: "12px 20px",
            background: "rgba(210, 153, 34, 0.1)",
            border: "1px solid rgba(210, 153, 34, 0.3)",
          }}
        >
          <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
            Sign in with GitHub to create and manage runners.
          </span>
          <button
            className="btn btn-primary"
            style={{ fontSize: 12, padding: "4px 12px" }}
            onClick={() => navigate("/settings")}
          >
            Sign in
          </button>
        </div>
      )}

      <div className="stats-grid">
        <StatsCard label="Total Runners" value={runners.length} />
        <StatsCard label="Online" value={online} color="var(--accent-green)" />
        <StatsCard label="Busy" value={busy} color="var(--accent-yellow)" />
        <StatsCard label="Avg CPU" value={`${avgCpu.toFixed(1)}%`} color="var(--accent-blue)" />
      </div>

      <RunnerTable
        runners={filtered}
        onStart={startRunner}
        onStop={stopRunner}
        onRestart={restartRunner}
        onDelete={deleteRunner}
        onStartGroup={startGroup}
        onStopGroup={stopGroup}
        onRestartGroup={restartGroup}
        onDeleteGroup={deleteGroup}
        onScaleGroup={scaleGroup}
        metrics={cpuMap}
        forceExpandedGroups={forceExpandedGroups}
        pendingActions={pendingActions}
        readOnly={!isAuthenticated}
      />

      {showWizard && (
        <NewRunnerWizard
          onClose={() => setShowWizard(false)}
          onCreate={createRunner}
          onCreateBatch={createBatch}
        />
      )}
    </div>
  );
}
