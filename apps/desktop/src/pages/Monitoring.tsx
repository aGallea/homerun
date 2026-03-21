import { useMetrics } from "../hooks/useMetrics";
import { useRunners } from "../hooks/useRunners";
import { StatusBadge } from "../components/StatusBadge";

function formatBytes(bytes: number): string {
  const gb = bytes / 1024 / 1024 / 1024;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / 1024 / 1024;
  return `${mb.toFixed(0)} MB`;
}

function metricColor(pct: number): string {
  if (pct >= 80) return "var(--accent-red)";
  if (pct >= 60) return "var(--accent-yellow)";
  return "var(--accent-green)";
}

interface ResourceBarProps {
  label: string;
  value: number; // 0–100
  sublabel?: string;
}

function ResourceBar({ label, value, sublabel }: ResourceBarProps) {
  const color = metricColor(value);
  return (
    <div className="card" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <div className="flex items-center justify-between">
        <span
          style={{
            fontSize: 11,
            fontWeight: 500,
            color: "var(--text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
          }}
        >
          {label}
        </span>
        <span
          className="font-mono"
          style={{ fontSize: 20, fontWeight: 600, color }}
        >
          {value.toFixed(1)}%
        </span>
      </div>
      <div
        style={{
          height: 8,
          background: "var(--bg-tertiary)",
          borderRadius: 4,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            height: "100%",
            width: `${Math.min(value, 100)}%`,
            background: color,
            borderRadius: 4,
            transition: "width 0.5s ease",
          }}
        />
      </div>
      {sublabel && (
        <span className="text-muted" style={{ fontSize: 12 }}>
          {sublabel}
        </span>
      )}
    </div>
  );
}

export function Monitoring() {
  const { metrics, loading: metricsLoading, error: metricsError } = useMetrics(3000);
  const { runners } = useRunners();

  const sys = metrics?.system;

  const memPct =
    sys && sys.memory_total_bytes > 0
      ? (sys.memory_used_bytes / sys.memory_total_bytes) * 100
      : 0;

  const diskPct =
    sys && sys.disk_total_bytes > 0
      ? (sys.disk_used_bytes / sys.disk_total_bytes) * 100
      : 0;

  // Build per-runner metrics map
  const metricsMap = new Map<string, { cpu: number; memBytes: number }>();
  metrics?.runners.forEach((m) =>
    metricsMap.set(m.runner_id, {
      cpu: m.cpu_percent,
      memBytes: m.memory_bytes,
    }),
  );

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Monitoring</h1>
        <span className="text-muted" style={{ fontSize: 12 }}>
          Auto-refreshes every 3s
        </span>
      </div>

      {metricsError && (
        <div className="error-banner" style={{ marginBottom: 20 }}>
          {metricsError}
        </div>
      )}

      {/* System metrics */}
      <section style={{ marginBottom: 32 }}>
        <h2
          style={{
            fontSize: 13,
            fontWeight: 500,
            color: "var(--text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
            marginBottom: 16,
          }}
        >
          System
        </h2>

        {metricsLoading && !metrics ? (
          <p className="text-muted">Loading metrics...</p>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
              gap: 16,
            }}
          >
            <ResourceBar
              label="CPU"
              value={sys?.cpu_percent ?? 0}
            />
            <ResourceBar
              label="Memory"
              value={memPct}
              sublabel={
                sys
                  ? `${formatBytes(sys.memory_used_bytes)} / ${formatBytes(sys.memory_total_bytes)}`
                  : undefined
              }
            />
            <ResourceBar
              label="Disk"
              value={diskPct}
              sublabel={
                sys
                  ? `${formatBytes(sys.disk_used_bytes)} / ${formatBytes(sys.disk_total_bytes)}`
                  : undefined
              }
            />
          </div>
        )}
      </section>

      {/* Per-runner table */}
      <section>
        <h2
          style={{
            fontSize: 13,
            fontWeight: 500,
            color: "var(--text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.5px",
            marginBottom: 16,
          }}
        >
          Runners
        </h2>

        {runners.length === 0 ? (
          <div className="card" style={{ textAlign: "center", padding: 32 }}>
            <p className="text-muted">No runners to monitor.</p>
          </div>
        ) : (
          <div className="card" style={{ padding: 0, overflow: "hidden" }}>
            <table className="table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Status</th>
                  <th>CPU</th>
                  <th>Memory</th>
                </tr>
              </thead>
              <tbody>
                {runners.map((runner) => {
                  const rm = metricsMap.get(runner.config.id);
                  const cpuPct = rm?.cpu ?? 0;
                  const cpuColor = metricColor(cpuPct);
                  const hasMetrics = rm != null;

                  return (
                    <tr key={runner.config.id}>
                      <td>
                        <span className="font-mono" style={{ fontSize: 13 }}>
                          {runner.config.name}
                        </span>
                        <div
                          className="text-muted"
                          style={{ fontSize: 11, marginTop: 2 }}
                        >
                          {runner.config.repo_owner}/{runner.config.repo_name}
                        </div>
                      </td>
                      <td>
                        <StatusBadge state={runner.state} />
                      </td>
                      <td>
                        {hasMetrics ? (
                          <div>
                            <div
                              className="font-mono"
                              style={{
                                fontSize: 13,
                                color: cpuColor,
                                marginBottom: 4,
                              }}
                            >
                              {cpuPct.toFixed(1)}%
                            </div>
                            <div
                              style={{
                                height: 4,
                                width: 80,
                                background: "var(--bg-tertiary)",
                                borderRadius: 2,
                                overflow: "hidden",
                              }}
                            >
                              <div
                                style={{
                                  height: "100%",
                                  width: `${Math.min(cpuPct, 100)}%`,
                                  background: cpuColor,
                                  borderRadius: 2,
                                  transition: "width 0.5s ease",
                                }}
                              />
                            </div>
                          </div>
                        ) : (
                          <span className="text-muted">--</span>
                        )}
                      </td>
                      <td>
                        {hasMetrics ? (
                          <span className="font-mono" style={{ fontSize: 13 }}>
                            {formatBytes(rm!.memBytes)}
                          </span>
                        ) : (
                          <span className="text-muted">--</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
