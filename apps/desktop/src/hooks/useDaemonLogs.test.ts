import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDaemonLogs } from "./useDaemonLogs";

vi.mock("../api/commands", () => ({
  api: { getDaemonLogsRecent: vi.fn() },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

describe("useDaemonLogs", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.useRealTimers());

  it("waits for a slow request before scheduling the next poll", async () => {
    vi.useFakeTimers();
    let resolveFirst:
      ((value: Awaited<ReturnType<typeof api.getDaemonLogsRecent>>) => void) | undefined;
    mockedApi.getDaemonLogsRecent
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValue([]);

    const { unmount } = renderHook(() => useDaemonLogs(1000));
    await act(async () => Promise.resolve());
    expect(mockedApi.getDaemonLogsRecent).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(5000));
    expect(mockedApi.getDaemonLogsRecent).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst?.([]);
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(mockedApi.getDaemonLogsRecent).toHaveBeenCalledTimes(2);
    unmount();
  });
});
