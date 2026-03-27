import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMetrics } from "./useMetrics";
import { api } from "../api/commands";
import { makeMetrics } from "../test/factories";

vi.mock("../api/commands", () => ({
  api: {
    getMetrics: vi.fn(),
  },
}));

const mockMetrics = makeMetrics({
  runners: [{ runner_id: "r1", cpu_percent: 42.5, memory_bytes: 100_000 }],
});

/** Flush pending microtasks/promises without advancing fake timers. */
async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  vi.mocked(api.getMetrics).mockResolvedValue(mockMetrics);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useMetrics", () => {
  it("returns loading true initially, then false after fetch", async () => {
    const { result } = renderHook(() => useMetrics());

    await flushPromises();

    expect(result.current.loading).toBe(false);
    expect(result.current.metrics).toEqual(mockMetrics);
  });

  it("polls at default 2-second interval", async () => {
    renderHook(() => useMetrics());

    await flushPromises();
    expect(api.getMetrics).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(api.getMetrics).toHaveBeenCalledTimes(2);
  });

  it("uses custom poll interval", async () => {
    renderHook(() => useMetrics(5000));

    await flushPromises();
    expect(api.getMetrics).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(api.getMetrics).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(api.getMetrics).toHaveBeenCalledTimes(2);
  });

  it("refresh manually triggers a fetch", async () => {
    const { result } = renderHook(() => useMetrics());

    await flushPromises();

    const callsBefore = vi.mocked(api.getMetrics).mock.calls.length;

    await act(async () => {
      await result.current.refresh();
    });

    expect(vi.mocked(api.getMetrics).mock.calls.length).toBe(callsBefore + 1);
  });

  it("sets error when getMetrics rejects", async () => {
    vi.mocked(api.getMetrics).mockRejectedValueOnce(new Error("timeout"));
    const { result } = renderHook(() => useMetrics());

    await flushPromises();

    expect(result.current.error).toBe("Error: timeout");
    expect(result.current.metrics).toBeNull();
  });
});
