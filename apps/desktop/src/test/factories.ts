import type { RunnerInfo, RunnerState, RepoInfo, MetricsResponse, AuthStatus } from "../api/types";

export function makeRunner(overrides: Partial<RunnerInfo> & { name: string }): RunnerInfo {
  return {
    config: {
      id: overrides.config?.id ?? overrides.name,
      name: overrides.name,
      repo_owner: overrides.config?.repo_owner ?? "org",
      repo_name: overrides.config?.repo_name ?? "repo",
      labels: overrides.config?.labels ?? ["self-hosted"],
      mode: overrides.config?.mode ?? "app",
      work_dir: overrides.config?.work_dir ?? "/tmp",
      group_id: overrides.config?.group_id,
    },
    state: (overrides.state as RunnerState) ?? "online",
    pid: overrides.pid ?? 1234,
    uptime_secs: overrides.uptime_secs ?? 100,
    jobs_completed: overrides.jobs_completed ?? 0,
    jobs_failed: overrides.jobs_failed ?? 0,
    current_job: overrides.current_job ?? null,
    job_started_at: overrides.job_started_at ?? null,
    estimated_job_duration_secs: overrides.estimated_job_duration_secs ?? null,
  };
}

export function makeRepo(overrides: Partial<RepoInfo> & { full_name: string }): RepoInfo {
  const parts = overrides.full_name.split("/");
  return {
    id: overrides.id ?? Math.floor(Math.random() * 100000),
    full_name: overrides.full_name,
    name: overrides.name ?? parts[1] ?? overrides.full_name,
    owner: overrides.owner ?? parts[0] ?? "org",
    private: overrides.private ?? false,
    html_url: overrides.html_url ?? `https://github.com/${overrides.full_name}`,
    is_org: overrides.is_org ?? false,
  };
}

export function makeMetrics(overrides?: Partial<MetricsResponse>): MetricsResponse {
  return {
    system: overrides?.system ?? {
      cpu_percent: 25.0,
      memory_used_bytes: 4_000_000_000,
      memory_total_bytes: 16_000_000_000,
      disk_used_bytes: 100_000_000_000,
      disk_total_bytes: 500_000_000_000,
    },
    runners: overrides?.runners ?? [],
  };
}

export function makeAuth(overrides?: Partial<AuthStatus>): AuthStatus {
  return {
    authenticated: overrides?.authenticated ?? false,
    user: overrides?.user ?? null,
  };
}
