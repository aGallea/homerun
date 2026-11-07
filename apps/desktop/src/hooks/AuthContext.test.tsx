import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthProvider, useAuth } from "./AuthContext";

vi.mock("../api/commands", () => ({
  api: {
    getAuthStatus: vi.fn(),
    loginWithToken: vi.fn(),
    logout: vi.fn(),
  },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

function Probe() {
  const { loading } = useAuth();
  return <div>{loading ? "loading" : "ready"}</div>;
}

describe("AuthProvider", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => vi.useRealTimers());

  it("waits for a slow auth request before scheduling the next poll", async () => {
    vi.useFakeTimers();
    let resolveFirst: ((value: Awaited<ReturnType<typeof api.getAuthStatus>>) => void) | undefined;
    mockedApi.getAuthStatus
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValue({ authenticated: false, user: null });

    const { unmount } = render(
      <AuthProvider>
        <Probe />
      </AuthProvider>,
    );

    await act(async () => Promise.resolve());
    expect(mockedApi.getAuthStatus).toHaveBeenCalledTimes(1);
    expect(screen.getByText("loading")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(15_000));
    expect(mockedApi.getAuthStatus).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst?.({ authenticated: false, user: null });
      await Promise.resolve();
    });
    expect(screen.getByText("ready")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(mockedApi.getAuthStatus).toHaveBeenCalledTimes(2);
    unmount();
  });
});
