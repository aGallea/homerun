import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useJobHistory } from "./useJobHistory";

vi.mock("../api/commands", () => ({
  api: { getRunnerHistory: vi.fn() },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

describe("useJobHistory", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.useRealTimers());

  it("does not overlap a slow history request", async () => {
    vi.useFakeTimers();
    let resolveFirst:
      ((value: Awaited<ReturnType<typeof api.getRunnerHistory>>) => void) | undefined;
    mockedApi.getRunnerHistory
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValue([]);

    const { unmount } = renderHook(() => useJobHistory("runner-1"));
    await act(async () => Promise.resolve());
    expect(mockedApi.getRunnerHistory).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(30_000));
    expect(mockedApi.getRunnerHistory).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst?.([]);
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(mockedApi.getRunnerHistory).toHaveBeenCalledTimes(2);
    unmount();
  });
});
