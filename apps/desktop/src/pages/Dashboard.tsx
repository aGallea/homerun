import { useState } from "react";
import { useRunners } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { StatsCard } from "../components/StatsCard";
import { RunnerTable } from "../components/RunnerTable";
import { NewRunnerWizard } from "../components/NewRunnerWizard";

export function Dashboard() {
  const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner, createRunner } =
    useRunners();
  const { metrics } = useMetrics();
  const [showWizard, setShowWizard] = useState(false);

  const online = runners.filter((r) => r.state === "online" || r.state === "busy").length;
  const busy = runners.filter((r) => r.state === "busy").length;

  const cpuMap = new Map<string, number>();
  metrics?.runners.forEach((m) => cpuMap.set(m.runner_id, m.cpu_percent));
  const avgCpu =
    metrics && metrics.runners.length > 0
      ? metrics.runners.reduce((sum, r) => sum + r.cpu_percent, 0) / metrics.runners.length
      : 0;

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
        <button className="btn btn-primary" onClick={() => setShowWizard(true)}>
          + New Runner
        </button>
      </div>

      <div className="stats-grid">
        <StatsCard label="Total Runners" value={runners.length} />
        <StatsCard label="Online" value={online} color="var(--accent-green)" />
        <StatsCard label="Busy" value={busy} color="var(--accent-yellow)" />
        <StatsCard label="Avg CPU" value={`${avgCpu.toFixed(1)}%`} color="var(--accent-blue)" />
      </div>

      <RunnerTable
        runners={runners}
        onStart={startRunner}
        onStop={stopRunner}
        onRestart={restartRunner}
        onDelete={deleteRunner}
        metrics={cpuMap}
      />

      {showWizard && (
        <NewRunnerWizard onClose={() => setShowWizard(false)} onCreate={createRunner} />
      )}
    </div>
  );
}
