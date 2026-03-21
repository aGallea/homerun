import { invoke } from "@tauri-apps/api/core";
import type {
  AuthStatus,
  DeviceFlowResponse,
  LogEntry,
  RunnerInfo,
  RepoInfo,
  MetricsResponse,
  CreateRunnerRequest,
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

  // Health
  healthCheck: () => invoke<boolean>("health_check"),
  daemonAvailable: () => invoke<boolean>("daemon_available"),
};
