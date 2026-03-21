export interface RunnerConfig {
  id: string;
  name: string;
  repo_owner: string;
  repo_name: string;
  labels: string[];
  mode: string;
  work_dir: string;
}

export type RunnerState =
  | "creating"
  | "registering"
  | "online"
  | "busy"
  | "stopping"
  | "offline"
  | "error"
  | "deleting";

export interface RunnerInfo {
  config: RunnerConfig;
  state: RunnerState;
  pid: number | null;
  uptime_secs: number | null;
  jobs_completed: number;
  jobs_failed: number;
  current_job?: string | null;
}

export interface LogEntry {
  runner_id: string;
  timestamp: string;
  line: string;
  stream: string;
}

export interface GitHubUser {
  login: string;
  avatar_url: string;
}

export interface AuthStatus {
  authenticated: boolean;
  user: GitHubUser | null;
}

export interface DeviceFlowResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface SystemMetrics {
  cpu_percent: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  disk_used_bytes: number;
  disk_total_bytes: number;
}

export interface RunnerMetrics {
  runner_id: string;
  cpu_percent: number;
  memory_bytes: number;
}

export interface MetricsResponse {
  system: SystemMetrics;
  runners: RunnerMetrics[];
}

export interface RepoInfo {
  id: number;
  full_name: string;
  name: string;
  owner: string;
  private: boolean;
  html_url: string;
  is_org: boolean;
}

export interface CreateRunnerRequest {
  repo_full_name: string;
  name?: string;
  labels?: string[];
  mode?: string;
}

export interface RunnerEvent {
  runner_id: string;
  event_type: string;
  data: unknown;
  timestamp: string;
}
