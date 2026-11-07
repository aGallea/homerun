import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StepsResponse } from "../api/types";
import { useJobSteps } from "./useJobSteps";

vi.mock("../api/commands", () => ({
  api: {
    getRunnerSteps: vi.fn(),
    getStepLogs: vi.fn(),
  },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

describe("useJobSteps", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.useRealTimers());

  it("refetches final logs when a step was expanded before its status was known", async () => {
    let resolveSteps: ((value: Awaited<ReturnType<typeof api.getRunnerSteps>>) => void) | undefined;
    mockedApi.getRunnerSteps.mockReturnValue(
      new Promise((resolve) => {
        resolveSteps = resolve;
      }),
    );
    mockedApi.getStepLogs
      .mockResolvedValueOnce({ step_number: 1, step_name: "Build", lines: ["partial"] })
      .mockResolvedValueOnce({ step_number: 1, step_name: "Build", lines: ["final"] });

    const { result, unmount } = renderHook(() => useJobSteps("runner-1", true));

    act(() => result.current.toggleStep(1));
    await waitFor(() => expect(result.current.stepLogs[1]).toEqual(["partial"]));

    await act(async () => {
      resolveSteps?.({
        job_name: "CI",
        steps_discovered: 1,
        steps: [
          {
            number: 1,
            name: "Build",
            status: "succeeded",
            started_at: "2026-07-31T10:00:00Z",
            completed_at: "2026-07-31T10:00:05Z",
          },
        ],
      });
      await Promise.resolve();
    });

    await waitFor(() => expect(result.current.stepLogs[1]).toEqual(["final"]));
    expect(mockedApi.getStepLogs).toHaveBeenCalledTimes(2);
    unmount();
  });

  it("does not reuse terminal logs for the same step number in a later job", async () => {
    vi.useFakeTimers();
    const firstJob: StepsResponse = {
      job_name: "CI",
      steps_discovered: 1,
      steps: [
        {
          number: 1,
          name: "Build",
          status: "succeeded",
          started_at: "2026-07-31T10:00:00Z",
          completed_at: "2026-07-31T10:00:05Z",
        },
      ],
    };
    const secondJob: StepsResponse = {
      ...firstJob,
      steps: [
        {
          ...firstJob.steps[0],
          started_at: "2026-07-31T11:00:00Z",
          completed_at: "2026-07-31T11:00:05Z",
        },
      ],
    };
    mockedApi.getRunnerSteps.mockResolvedValueOnce(firstJob).mockResolvedValue(secondJob);
    mockedApi.getStepLogs
      .mockResolvedValueOnce({ step_number: 1, step_name: "Build", lines: ["first job"] })
      .mockResolvedValueOnce({ step_number: 1, step_name: "Build", lines: ["second job"] });

    const { result, unmount } = renderHook(() => useJobSteps("runner-1", true));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    act(() => result.current.toggleStep(1));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.stepLogs[1]).toEqual(["first job"]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
      await Promise.resolve();
    });
    expect(result.current.stepLogs[1]).toEqual(["second job"]);
    expect(mockedApi.getStepLogs).toHaveBeenCalledTimes(2);
    unmount();
  });
});
