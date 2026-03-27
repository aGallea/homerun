export interface RunnerConfig {
  id: string;
  name: string;
  repo_owner: string;
  repo_name: string;
  labels: string[];
  mode: string;
  work_dir: string;
  group_id?: string;
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

export type StepStatus = "pending" | "running" | "succeeded" | "failed" | "skipped" | "cancelled";

export interface StepInfo {
  number: number;
  name: string;
  status: StepStatus;
  started_at: string | null;
  completed_at: string | null;
}

export interface RunAttempt {
  attempt: number;
  succeeded: boolean;
  runner_name: string;
  completed_at: string;
  run_url?: string | null;
}

export interface CompletedJob {
  job_name: string;
  succeeded: boolean;
  completed_at: string;
  duration_secs: number;
  branch?: string | null;
  pr_number?: number | null;
  run_url?: string | null;
  error_message?: string | null;
  latest_attempt?: RunAttempt | null;
}

export interface JobHistoryEntry {
  job_name: string;
  started_at: string;
  completed_at: string;
  succeeded: boolean;
  branch?: string | null;
  pr_number?: number | null;
  run_url?: string | null;
  error_message?: string | null;
  steps: StepInfo[];
  latest_attempt?: RunAttempt | null;
  job_number: number;
}

export interface RunnerInfo {
  config: RunnerConfig;
  state: RunnerState;
  pid: number | null;
  uptime_secs: number | null;
  jobs_completed: number;
  jobs_failed: number;
  current_job?: string | null;
  job_context?: JobContext | null;
  error_message?: string | null;
  job_started_at?: string | null;
  last_completed_job?: CompletedJob | null;
  estimated_job_duration_secs?: number | null;
}

export interface JobContext {
  branch: string;
  pr_number: number | null;
  pr_url: string | null;
  run_url: string;
  job_id?: number | null;
}

export interface LogEntry {
  runner_id: string;
  timestamp: string;
  line: string;
  stream: string;
}

export interface StepsResponse {
  job_name: string;
  steps: StepInfo[];
  steps_discovered: number;
}

export interface StepLogsResponse {
  step_number: number;
  step_name: string;
  lines: string[];
}

export interface RunStatusResponse {
  status: string;
  conclusion: string | null;
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
  daemon?: DaemonMetrics;
}

export interface DaemonLogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export interface DaemonMetrics {
  pid: number;
  uptime_seconds: number;
  cpu_percent: number;
  memory_bytes: number;
  child_processes: ChildProcessInfo[];
}

export interface ChildProcessInfo {
  pid: number;
  runner_id: string;
  runner_name: string;
  cpu_percent: number;
  memory_bytes: number;
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

export interface CreateBatchRequest {
  repo_full_name: string;
  count: number;
  labels?: string[];
  mode?: string;
}

export interface BatchCreateResponse {
  group_id: string;
  runners: RunnerInfo[];
  errors: { index: number; error: string }[];
}

export interface GroupActionResult {
  runner_id: string;
  success: boolean;
  error?: string;
}

export interface GroupActionResponse {
  group_id: string;
  results: GroupActionResult[];
}

export interface ScaleGroupResponse {
  group_id: string;
  previous_count: number;
  target_count: number;
  actual_count: number;
  added: RunnerInfo[];
  removed: string[];
  skipped_busy: string[];
}

export interface Preferences {
  start_runners_on_launch: boolean;
  notify_status_changes: boolean;
  notify_job_completions: boolean;
}

export type TrayIconState = "idle" | "active" | "error" | "offline";
