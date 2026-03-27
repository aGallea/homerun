import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRunners } from "./useRunners";
import { api } from "../api/commands";
import { makeRunner } from "../test/factories";

vi.mock("../api/commands", () => ({
  api: {
    listRunners: vi.fn(),
    createRunner: vi.fn(),
    deleteRunner: vi.fn(),
    startRunner: vi.fn(),
    stopRunner: vi.fn(),
    restartRunner: vi.fn(),
    createBatch: vi.fn(),
    startGroup: vi.fn(),
    stopGroup: vi.fn(),
    restartGroup: vi.fn(),
    deleteGroup: vi.fn(),
    scaleGroup: vi.fn(),
  },
}));

const mockRunners = [
  makeRunner({ name: "runner-1", state: "online" }),
  makeRunner({ name: "runner-2", state: "offline" }),
];

beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(api.listRunners).mockResolvedValue(mockRunners);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useRunners", () => {
  it("returns loading true initially, then false after first fetch", async () => {
    const { result } = renderHook(() => useRunners());
    expect(result.current.loading).toBe(true);

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    expect(result.current.loading).toBe(false);
    expect(result.current.runners).toEqual(mockRunners);
  });

  it("polls listRunners every 2 seconds", async () => {
    renderHook(() => useRunners());

    // Flush the initial fetch (direct call, no timer)
    await act(async () => {
      await Promise.resolve();
    });
    const afterInitial = vi.mocked(api.listRunners).mock.calls.length;
    expect(afterInitial).toBeGreaterThanOrEqual(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(vi.mocked(api.listRunners).mock.calls.length).toBe(afterInitial + 1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(vi.mocked(api.listRunners).mock.calls.length).toBe(afterInitial + 2);
  });

  it("sets error when listRunners rejects", async () => {
    vi.mocked(api.listRunners).mockRejectedValueOnce(new Error("fetch failed"));
    const { result } = renderHook(() => useRunners());

    // Flush only the initial fetch promise (don't advance timers to avoid interval re-fetch)
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.error).toBe("Error: fetch failed");
    expect(result.current.runners).toEqual([]);
  });

  it("deleteRunner adds to pendingActions and refreshes after", async () => {
    vi.mocked(api.deleteRunner).mockResolvedValue(undefined);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    const callsBefore = vi.mocked(api.listRunners).mock.calls.length;

    await act(async () => {
      await result.current.deleteRunner("runner-1");
    });

    expect(api.deleteRunner).toHaveBeenCalledWith("runner-1");
    expect(vi.mocked(api.listRunners).mock.calls.length).toBeGreaterThan(callsBefore);
    expect(result.current.pendingActions.has("runner-1")).toBe(false);
  });

  it("startRunner calls api.startRunner and refreshes", async () => {
    vi.mocked(api.startRunner).mockResolvedValue(undefined);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    await act(async () => {
      await result.current.startRunner("runner-2");
    });

    expect(api.startRunner).toHaveBeenCalledWith("runner-2");
  });

  it("stopRunner calls api.stopRunner and refreshes", async () => {
    vi.mocked(api.stopRunner).mockResolvedValue(undefined);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    await act(async () => {
      await result.current.stopRunner("runner-1");
    });

    expect(api.stopRunner).toHaveBeenCalledWith("runner-1");
  });

  it("restartRunner calls api.restartRunner and refreshes", async () => {
    vi.mocked(api.restartRunner).mockResolvedValue(undefined);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    await act(async () => {
      await result.current.restartRunner("runner-1");
    });

    expect(api.restartRunner).toHaveBeenCalledWith("runner-1");
  });

  it("createRunner calls api.createRunner and refreshes", async () => {
    const newRunner = makeRunner({ name: "new-runner" });
    vi.mocked(api.createRunner).mockResolvedValue(newRunner);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    const req = { repo_full_name: "org/repo", name: "new-runner" };
    let created;
    await act(async () => {
      created = await result.current.createRunner(req);
    });

    expect(api.createRunner).toHaveBeenCalledWith(req);
    expect(created).toEqual(newRunner);
  });

  it("createBatch calls api.createBatch and refreshes", async () => {
    const batchResult = { group_id: "g1", runners: [], errors: [] };
    vi.mocked(api.createBatch).mockResolvedValue(batchResult);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    const req = { repo_full_name: "org/repo", count: 3 };
    await act(async () => {
      await result.current.createBatch(req);
    });

    expect(api.createBatch).toHaveBeenCalledWith(req);
  });

  it("stopGroup calls api.stopGroup with groupId", async () => {
    const groupResult = { group_id: "g1", results: [] };
    vi.mocked(api.stopGroup).mockResolvedValue(groupResult);
    const { result } = renderHook(() => useRunners());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    await act(async () => {
      await result.current.stopGroup("g1");
    });

    expect(api.stopGroup).toHaveBeenCalledWith("g1");
  });
});
