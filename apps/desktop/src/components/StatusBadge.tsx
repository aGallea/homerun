import type { RunnerState } from "../api/types";

const stateConfig: Record<RunnerState, { color: string; label: string }> = {
  creating: { color: "var(--accent-blue)", label: "Creating" },
  registering: { color: "var(--accent-blue)", label: "Registering" },
  online: { color: "var(--accent-green)", label: "Online" },
  busy: { color: "var(--accent-yellow)", label: "Busy" },
  stopping: { color: "var(--accent-yellow)", label: "Stopping" },
  offline: { color: "var(--text-secondary)", label: "Offline" },
  error: { color: "var(--accent-red)", label: "Error" },
  deleting: { color: "var(--accent-red)", label: "Deleting" },
};

export function StatusBadge({ state }: { state: RunnerState }) {
  const config = stateConfig[state] ?? {
    color: "var(--text-secondary)",
    label: state,
  };

  return (
    <span className="status-badge" style={{ color: config.color }}>
      <span className="status-dot" style={{ background: config.color }} />
      {config.label}
    </span>
  );
}
