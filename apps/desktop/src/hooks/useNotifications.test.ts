import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useNotifications } from "./useNotifications";
import { makeRunner } from "../test/factories";
import type { Preferences, RunnerInfo } from "../api/types";

const mockInvoke = vi.fn();
const mockResolveResource = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: (...args: unknown[]) => mockResolveResource(...args),
}));

function makePrefs(overrides?: Partial<Preferences>): Preferences {
  return {
    start_runners_on_launch: false,
    notify_status_changes: true,
    notify_job_completions: true,
    scan_labels: [],
    workspace_path: null,
    auto_scan: false,
    ...overrides,
  };
}

function runnerWithJob(
  name: string,
  state: string,
  succeeded: boolean,
  jobName: string,
  completedAt: string,
  durationSecs = 90,
): RunnerInfo {
  const base = makeRunner({ name, state: state as RunnerInfo["state"] });
  return {
    ...base,
    jobs_completed: succeeded ? base.jobs_completed + 1 : base.jobs_completed,
    jobs_failed: succeeded ? base.jobs_failed : base.jobs_failed + 1,
    last_completed_job: {
      job_name: jobName,
      succeeded,
      completed_at: completedAt,
      duration_secs: durationSecs,
    },
  };
}

beforeEach(() => {
  mockInvoke.mockClear();
  mockResolveResource.mockClear();
  mockInvoke.mockResolvedValue(undefined);
  mockResolveResource.mockImplementation((path: string) => Promise.resolve(`/resolved/${path}`));
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useNotifications", () => {
  it("does nothing when preferences is null", () => {
    const runners = [makeRunner({ name: "r1" })];
    renderHook(() => useNotifications(runners, null));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("does nothing when runners is empty", () => {
    renderHook(() => useNotifications([], makePrefs()));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("does not send notifications on first render (initialization)", () => {
    const runners = [makeRunner({ name: "r1", state: "online" })];
    renderHook(() => useNotifications(runners, makePrefs()));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("sends Runner Online notification when state changes to online", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "offline" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "online" })];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Runner Online",
        body: "r1 is now online and ready for jobs",
        icon_path: "/resolved/resources/notifications/active.png",
      });
    });
  });

  it("sends Runner Offline notification when state changes to offline", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "online" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "offline" })];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Runner Offline",
        body: "r1 went offline",
        icon_path: "/resolved/resources/notifications/offline.png",
      });
    });
  });

  it("sends Runner Error notification when state changes to error", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "online" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "error" })];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Runner Error",
        body: "r1 encountered an error",
        icon_path: "/resolved/resources/notifications/error.png",
      });
    });
  });

  it("does not notify online when transitioning from busy (job finished)", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "online" })];
    rerender({ runners: updated, prefs });

    // Give time for any async notification
    await new Promise((r) => setTimeout(r, 50));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("does not send status notifications when notify_status_changes is false", async () => {
    const prefs = makePrefs({ notify_status_changes: false });
    const initial = [makeRunner({ name: "r1", state: "online" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "offline" })];
    rerender({ runners: updated, prefs });

    await new Promise((r) => setTimeout(r, 50));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("sends Job Completed notification", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [runnerWithJob("r1", "online", true, "build", "2026-01-01T00:00:00Z")];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Job Completed",
        body: "build on r1 passed in 1m 30s",
        icon_path: "/resolved/resources/notifications/active.png",
      });
    });
  });

  it("sends Job Failed notification", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [runnerWithJob("r1", "online", false, "deploy", "2026-01-01T00:00:00Z")];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Job Failed",
        body: "deploy on r1 failed",
        icon_path: "/resolved/resources/notifications/error.png",
      });
    });
  });

  it("does not send job notifications when notify_job_completions is false", async () => {
    const prefs = makePrefs({ notify_job_completions: false });
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [runnerWithJob("r1", "online", true, "build", "2026-01-01T00:00:00Z")];
    rerender({ runners: updated, prefs });

    await new Promise((r) => setTimeout(r, 50));
    // Only the busy->online status notification should have fired, not job
    const jobCalls = mockInvoke.mock.calls.filter(
      (c: unknown[]) => (c[1] as Record<string, string>)?.title === "Job Completed",
    );
    expect(jobCalls).toHaveLength(0);
  });

  it("sends Job Completed when counter increases even if job key was briefly null", async () => {
    const prefs = makePrefs();
    // Runner is busy with jobs_completed=0
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    // Job completed (counter increased) and runner is back online with last_completed_job
    const completed = runnerWithJob("r1", "online", true, "test", "2026-02-01T00:00:00Z", 30);
    rerender({ runners: [completed], prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Job Completed",
        body: "test on r1 passed in 30s",
        icon_path: "/resolved/resources/notifications/active.png",
      });
    });
  });

  it("does not double-notify when counter stays the same across polls", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    // First completion
    const completed = runnerWithJob("r1", "online", true, "test", "2026-02-01T00:00:00Z", 30);
    rerender({ runners: [completed], prefs });
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledTimes(1);
    });

    // Same data on next poll — should NOT notify again
    mockInvoke.mockClear();
    mockResolveResource.mockImplementation((path: string) => Promise.resolve(`/resolved/${path}`));
    rerender({ runners: [completed], prefs });
    await new Promise((r) => setTimeout(r, 50));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("sends Runner Deleted notification when runner disappears", async () => {
    const prefs = makePrefs();
    const initial = [
      makeRunner({ name: "r1", state: "online" }),
      makeRunner({ name: "r2", state: "offline" }),
    ];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    // Remove r2
    const updated = [makeRunner({ name: "r1", state: "online" })];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("send_notification", {
        title: "Runner Deleted",
        body: "r2 was removed",
        icon_path: "/resolved/resources/notifications/offline.png",
      });
    });
  });

  it("does not send deleted notification when notify_status_changes is false", async () => {
    const prefs = makePrefs({ notify_status_changes: false });
    const initial = [makeRunner({ name: "r1" }), makeRunner({ name: "r2" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    rerender({ runners: [makeRunner({ name: "r1" })], prefs });

    await new Promise((r) => setTimeout(r, 50));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("handles notification send failure gracefully", async () => {
    mockResolveResource.mockRejectedValue(new Error("resource not found"));
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "offline" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const updated = [makeRunner({ name: "r1", state: "online" })];
    rerender({ runners: updated, prefs });

    await vi.waitFor(() => {
      expect(consoleSpy).toHaveBeenCalled();
    });
    consoleSpy.mockRestore();
  });
});

describe("formatDuration (via Job Completed body)", () => {
  it("formats seconds only", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const runner = runnerWithJob("r1", "online", true, "test", "2026-01-01T00:00:00Z", 45);
    rerender({ runners: [runner], prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "send_notification",
        expect.objectContaining({ body: "test on r1 passed in 45s" }),
      );
    });
  });

  it("formats exact minutes", async () => {
    const prefs = makePrefs();
    const initial = [makeRunner({ name: "r1", state: "busy" })];
    const { rerender } = renderHook(({ runners, prefs }) => useNotifications(runners, prefs), {
      initialProps: { runners: initial, prefs },
    });

    const runner = runnerWithJob("r1", "online", true, "test", "2026-01-01T00:00:00Z", 120);
    rerender({ runners: [runner], prefs });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "send_notification",
        expect.objectContaining({ body: "test on r1 passed in 2m" }),
      );
    });
  });
});
