import { useNavigate } from "react-router-dom";
import type { RunnerInfo } from "../api/types";
import { StatusBadge } from "./StatusBadge";
import { RunnerActions } from "./RunnerActions";

interface RunnerTableProps {
  runners: RunnerInfo[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onRestart: (id: string) => void;
  onDelete: (id: string) => void;
  metrics?: Map<string, number>;
}

export function RunnerTable({
  runners,
  onStart,
  onStop,
  onRestart,
  onDelete,
  metrics,
}: RunnerTableProps) {
  const navigate = useNavigate();

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
            <th>Mode</th>
            <th>CPU</th>
            <th style={{ width: 60 }}></th>
          </tr>
        </thead>
        <tbody>
          {runners.map((runner) => (
            <tr
              key={runner.config.id}
              style={{ cursor: "pointer" }}
              onClick={() => navigate(`/runners/${runner.config.id}`)}
            >
              <td>
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
              <td
                className="text-muted"
                style={{ textTransform: "capitalize" }}
              >
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
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
