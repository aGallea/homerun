import { useState } from "react";
import { useRunners } from "../hooks/useRunners";
import { useMetrics } from "../hooks/useMetrics";
import { RunnerTable } from "../components/RunnerTable";
import { NewRunnerWizard } from "../components/NewRunnerWizard";

export function Runners() {
  const { runners, loading, startRunner, stopRunner, restartRunner, deleteRunner, createRunner } =
    useRunners();
  const { metrics } = useMetrics();
  const [showWizard, setShowWizard] = useState(false);
  const [filter, setFilter] = useState("");

  const cpuMap = new Map<string, number>();
  metrics?.runners.forEach((m) => cpuMap.set(m.runner_id, m.cpu_percent));

  const filtered = runners.filter(
    (r) =>
      r.config.name.toLowerCase().includes(filter.toLowerCase()) ||
      `${r.config.repo_owner}/${r.config.repo_name}`.toLowerCase().includes(filter.toLowerCase()),
  );

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
        <h1 className="page-title">Runners</h1>
        <div className="flex items-center gap-8">
          <input
            className="input"
            placeholder="Filter runners..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          <button className="btn btn-primary" onClick={() => setShowWizard(true)}>
            + New Runner
          </button>
        </div>
      </div>

      <RunnerTable
        runners={filtered}
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
