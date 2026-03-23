import { invoke } from "@tauri-apps/api/core";
import type {
  AuthStatus,
  BatchCreateResponse,
  CreateBatchRequest,
  DaemonLogEntry,
  DeviceFlowResponse,
  GroupActionResponse,
  JobHistoryEntry,
  LogEntry,
  MetricsResponse,
  Preferences,
  RepoInfo,
  RunnerInfo,
  CreateRunnerRequest,
  ScaleGroupResponse,
  StepsResponse,
  StepLogsResponse,
} from "./types";

export const api = {
  // Auth
  getAuthStatus: () => invoke<AuthStatus>("auth_status"),
  loginWithToken: (token: string) => invoke<AuthStatus>("login_with_token", { token }),
  logout: () => invoke<void>("logout"),
  startDeviceFlow: () => invoke<DeviceFlowResponse>("start_device_flow"),
  pollDeviceFlow: (deviceCode: string, interval: number) =>
    invoke<AuthStatus>("poll_device_flow", {
      device_code: deviceCode,
      interval,
    }),

  // Runners
  listRunners: () => invoke<RunnerInfo[]>("list_runners"),
  createRunner: (req: CreateRunnerRequest) => invoke<RunnerInfo>("create_runner", { req }),
  deleteRunner: (id: string) => invoke<void>("delete_runner", { id }),
  startRunner: (id: string) => invoke<void>("start_runner", { id }),
  stopRunner: (id: string) => invoke<void>("stop_runner", { id }),
  restartRunner: (id: string) => invoke<void>("restart_runner", { id }),

  // Repos
  listRepos: () => invoke<RepoInfo[]>("list_repos"),

  // Metrics
  getMetrics: () => invoke<MetricsResponse>("get_metrics"),

  // Logs
  getRunnerLogs: (runnerId: string) =>
    invoke<LogEntry[]>("get_runner_logs", { runner_id: runnerId }),
  getDaemonLogsRecent: (level?: string, limit?: number, search?: string) =>
    invoke<DaemonLogEntry[]>("get_daemon_logs_recent", { level, limit, search }),

  // Steps
  getRunnerSteps: (runnerId: string) =>
    invoke<StepsResponse>("get_runner_steps", { runner_id: runnerId }),
  getStepLogs: (runnerId: string, stepNumber: number) =>
    invoke<StepLogsResponse>("get_step_logs", { runner_id: runnerId, step_number: stepNumber }),

  // History
  getRunnerHistory: (runnerId: string) =>
    invoke<JobHistoryEntry[]>("get_runner_history", { runner_id: runnerId }),
  rerunWorkflow: (runnerId: string, runUrl: string) =>
    invoke<void>("rerun_workflow", { runner_id: runnerId, run_url: runUrl }),

  // Health
  healthCheck: () => invoke<boolean>("health_check"),
  daemonAvailable: () => invoke<boolean>("daemon_available"),

  // Batch / Groups
  createBatch: (req: CreateBatchRequest) => invoke<BatchCreateResponse>("create_batch", { req }),
  startGroup: (groupId: string) =>
    invoke<GroupActionResponse>("start_group", { group_id: groupId }),
  stopGroup: (groupId: string) => invoke<GroupActionResponse>("stop_group", { group_id: groupId }),
  restartGroup: (groupId: string) =>
    invoke<GroupActionResponse>("restart_group", { group_id: groupId }),
  deleteGroup: (groupId: string) =>
    invoke<GroupActionResponse>("delete_group", { group_id: groupId }),
  scaleGroup: (groupId: string, count: number) =>
    invoke<ScaleGroupResponse>("scale_group", { group_id: groupId, count }),

  // Preferences
  getPreferences: () => invoke<Preferences>("get_preferences"),
  updatePreferences: (prefs: Preferences) => invoke<Preferences>("update_preferences", { prefs }),
};
