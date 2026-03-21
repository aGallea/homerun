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

const transientStates: Set<RunnerState> = new Set([
  "creating",
  "registering",
  "stopping",
  "deleting",
]);

export function StatusBadge({ state }: { state: RunnerState }) {
  const config = stateConfig[state] ?? {
    color: "var(--text-secondary)",
    label: state,
  };

  const isLoading = transientStates.has(state);

  return (
    <span className="status-badge" style={{ color: config.color }}>
      {isLoading ? (
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          style={{ animation: "spin 1s linear infinite", flexShrink: 0 }}
        >
          <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
          <path d="M12 2a10 10 0 0 1 10 10" />
        </svg>
      ) : (
        <span className="status-dot" style={{ background: config.color }} />
      )}
      {config.label}
    </span>
  );
}
