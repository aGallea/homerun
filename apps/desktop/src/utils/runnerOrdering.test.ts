import { describe, expect, it } from "vitest";
import type { RunnerInfo, RunnerState } from "../api/types";
import { filterCompactRunners, sortRunnersByActivity } from "./runnerOrdering";

function runner(
  name: string,
  state: RunnerState,
  options: { completedAt?: string; jobStartedAt?: string; startedAt?: string } = {},
): RunnerInfo {
  return {
    config: {
      id: name,
      name,
      repo_owner: "owner",
      repo_name: "repo",
      labels: [],
      mode: "app",
      work_dir: `/tmp/${name}`,
    },
    state,
    pid: null,
    uptime_secs: null,
    started_at: options.startedAt ?? null,
    jobs_completed: options.completedAt ? 1 : 0,
    jobs_failed: 0,
    current_job: state === "busy" ? "build" : null,
    job_started_at: options.jobStartedAt ?? null,
    last_completed_job: options.completedAt
      ? {
          job_name: "build",
          succeeded: true,
          completed_at: options.completedAt,
          duration_secs: 5,
        }
      : null,
  };
}

describe("runner activity ordering", () => {
  it("puts active work first, then recent activity, and offline runners last", () => {
    const busy = runner("busy", "busy", { jobStartedAt: "2026-08-28T12:00:00Z" });
    const recent = runner("recent", "online", { completedAt: "2026-08-28T11:59:59Z" });
    const old = runner("old", "online", { completedAt: "2026-08-27T12:00:00Z" });
    const offline = runner("offline", "offline", { completedAt: "2026-08-28T12:01:00Z" });
    const error = runner("error", "error", { completedAt: "2026-08-28T12:02:00Z" });

    expect(
      sortRunnersByActivity([offline, error, old, recent, busy]).map((item) => item.config.name),
    ).toEqual(["busy", "recent", "old", "error", "offline"]);
  });

  it("uses runner start time when no job activity has been recorded yet", () => {
    const recentStart = runner("recent-start", "online", { startedAt: "2026-08-28T12:00:00Z" });
    const oldStart = runner("old-start", "online", { startedAt: "2026-08-28T11:00:00Z" });

    expect(sortRunnersByActivity([oldStart, recentStart]).map((item) => item.config.name)).toEqual([
      "recent-start",
      "old-start",
    ]);
  });

  it("hides only offline/error runners from compact lists", () => {
    const online = runner("online", "online");
    const busy = runner("busy", "busy");
    const offline = runner("offline", "offline");
    const error = runner("error", "error");
    expect(filterCompactRunners([online, offline, error, busy], true)).toEqual([online, busy]);
    expect(filterCompactRunners([online, offline], false)).toEqual([online, offline]);
  });
});
