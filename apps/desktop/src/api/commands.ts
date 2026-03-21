import { invoke } from "@tauri-apps/api/core";
import type {
  AuthStatus,
  RunnerInfo,
  RepoInfo,
  MetricsResponse,
  CreateRunnerRequest,
} from "./types";

export const api = {
  // Auth
  getAuthStatus: () => invoke<AuthStatus>("auth_status"),
  loginWithToken: (token: string) =>
    invoke<AuthStatus>("login_with_token", { token }),
  logout: () => invoke<void>("logout"),

  // Runners
  listRunners: () => invoke<RunnerInfo[]>("list_runners"),
  createRunner: (req: CreateRunnerRequest) =>
    invoke<RunnerInfo>("create_runner", { req }),
  deleteRunner: (id: string) => invoke<void>("delete_runner", { id }),
  startRunner: (id: string) => invoke<void>("start_runner", { id }),
  stopRunner: (id: string) => invoke<void>("stop_runner", { id }),
  restartRunner: (id: string) => invoke<void>("restart_runner", { id }),

  // Repos
  listRepos: () => invoke<RepoInfo[]>("list_repos"),

  // Metrics
  getMetrics: () => invoke<MetricsResponse>("get_metrics"),

  // Health
  healthCheck: () => invoke<boolean>("health_check"),
  daemonAvailable: () => invoke<boolean>("daemon_available"),
};
