import { useState, useEffect, useRef } from "react";
import type { StepInfo, StepStatus } from "../api/types";

export interface JobProgressProps {
  steps: StepInfo[];
  stepsDiscovered: number;
  jobName: string | null;
  expandedStep: number | null;
  stepLogs: Record<number, string[]>;
  onToggleStep: (stepNumber: number) => void;
}

function formatDuration(startedAt: string | null, completedAt: string | null): string {
  if (!startedAt) return "\u2014";
  const start = new Date(startedAt).getTime();
  const end = completedAt ? new Date(completedAt).getTime() : Date.now();
  const totalSecs = Math.max(0, Math.floor((end - start) / 1000));
  if (totalSecs < 60) return `${totalSecs}s`;
  const mins = Math.floor(totalSecs / 60);
  const secs = totalSecs % 60;
  return `${mins}m ${secs}s`;
}

function StepIcon({ status }: { status: StepStatus }) {
  switch (status) {
    case "succeeded":
      return (
        <span style={{ color: "var(--accent-green)", fontSize: 14, fontWeight: 700 }}>
          {"\u2713"}
        </span>
      );
    case "failed":
      return (
        <span style={{ color: "var(--accent-red)", fontSize: 14, fontWeight: 700 }}>
          {"\u2715"}
        </span>
      );
    case "skipped":
      return (
        <span style={{ color: "var(--text-secondary)", fontSize: 14, fontWeight: 700 }}>
          {"\u2298"}
        </span>
      );
    case "running":
      return <span className="step-spinner" />;
    case "pending":
    default:
      return (
        <span style={{ color: "var(--text-secondary)", fontSize: 14, opacity: 0.5 }}>
          {"\u25CB"}
        </span>
      );
  }
}

export function JobProgress({
  steps,
  stepsDiscovered,
  jobName,
  expandedStep,
  stepLogs,
  onToggleStep,
}: JobProgressProps) {
  const [, setTick] = useState(0);
  const logRefs = useRef<Record<number, HTMLDivElement | null>>({});

  // Live timer tick for running steps
  const hasRunning = steps.some((s) => s.status === "running");
  useEffect(() => {
    if (!hasRunning) return;
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, [hasRunning]);

  // Auto-scroll expanded log content
  useEffect(() => {
    if (expandedStep !== null && logRefs.current[expandedStep]) {
      const el = logRefs.current[expandedStep];
      if (el) {
        el.scrollTop = el.scrollHeight;
      }
    }
  }, [expandedStep, stepLogs]);

  const completedCount = steps.filter(
    (s) => s.status === "succeeded" || s.status === "failed" || s.status === "skipped",
  ).length;

  return (
    <div className="runner-card" style={{ marginBottom: 16, padding: 0, overflow: "hidden" }}>
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "12px 16px",
          borderBottom: "1px solid var(--border)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <h3 className="runner-card-label" style={{ margin: 0 }}>
            {jobName ? `Job: ${jobName}` : "Job Progress"}
          </h3>
          <span
            style={{
              fontSize: 12,
              padding: "2px 8px",
              borderRadius: 10,
              background: "var(--bg-tertiary)",
              color: "var(--text-secondary)",
              fontWeight: 500,
            }}
          >
            {completedCount}/{stepsDiscovered} steps
          </span>
        </div>
      </div>

      {/* Step list */}
      <div style={{ display: "flex", flexDirection: "column" }}>
        {steps.map((step) => {
          const isPending = step.status === "pending";
          const isRunning = step.status === "running";
          const isExpanded = expandedStep === step.number;
          const logs = stepLogs[step.number];

          return (
            <div key={step.number}>
              <div
                onClick={() => {
                  if (!isPending) onToggleStep(step.number);
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "8px 16px",
                  cursor: isPending ? "default" : "pointer",
                  opacity: isPending ? 0.5 : 1,
                  borderLeft: isRunning
                    ? "3px solid var(--accent-yellow)"
                    : "3px solid transparent",
                  background: isRunning ? "rgba(210, 153, 34, 0.06)" : "transparent",
                  transition: "background 0.15s",
                }}
                onMouseEnter={(e) => {
                  if (!isPending && !isRunning) {
                    e.currentTarget.style.background = "var(--bg-secondary)";
                  }
                }}
                onMouseLeave={(e) => {
                  if (!isPending && !isRunning) {
                    e.currentTarget.style.background = "transparent";
                  }
                }}
              >
                {/* Icon */}
                <div
                  style={{
                    width: 20,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flexShrink: 0,
                  }}
                >
                  <StepIcon status={step.status} />
                </div>

                {/* Name */}
                <span
                  style={{
                    flex: 1,
                    fontSize: 13,
                    fontWeight: isRunning ? 600 : 400,
                    color: isRunning
                      ? "var(--accent-yellow)"
                      : isPending
                        ? "var(--text-secondary)"
                        : "var(--text-primary)",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {step.name}
                </span>

                {/* Duration */}
                <span
                  className="font-mono"
                  style={{
                    fontSize: 12,
                    color: "var(--text-secondary)",
                    flexShrink: 0,
                  }}
                >
                  {formatDuration(step.started_at, step.completed_at)}
                </span>

                {/* Expand arrow */}
                {!isPending && (
                  <span
                    style={{
                      fontSize: 12,
                      color: "var(--text-secondary)",
                      width: 16,
                      textAlign: "center",
                      flexShrink: 0,
                    }}
                  >
                    {isExpanded ? "\u25BE" : "\u25B8"}
                  </span>
                )}
                {isPending && <span style={{ width: 16, flexShrink: 0 }} />}
              </div>

              {/* Expanded log view */}
              {isExpanded && (
                <div
                  ref={(el) => {
                    logRefs.current[step.number] = el;
                  }}
                  className="font-mono"
                  style={{
                    maxHeight: 200,
                    overflowY: "auto",
                    background: "var(--bg-primary)",
                    borderTop: "1px solid var(--border)",
                    borderBottom: "1px solid var(--border)",
                    padding: "8px 16px 8px 49px",
                    fontSize: 12,
                    lineHeight: 1.6,
                    color: "var(--text-secondary)",
                  }}
                >
                  {logs === undefined ? (
                    <span style={{ color: "var(--text-secondary)", fontStyle: "italic" }}>
                      Fetching logs...
                    </span>
                  ) : logs.length === 0 ? (
                    <span style={{ color: "var(--text-secondary)", fontStyle: "italic" }}>
                      {step.status === "running"
                        ? "Logs available after step completes."
                        : "No log output."}
                    </span>
                  ) : (
                    logs.map((line, i) => (
                      <div key={i} style={{ whiteSpace: "pre-wrap", wordBreak: "break-all" }}>
                        {line}
                      </div>
                    ))
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
