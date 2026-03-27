import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";
import { AuthProvider, useAuth } from "./AuthContext";
import { api } from "../api/commands";

vi.mock("../api/commands", () => ({
  api: {
    getAuthStatus: vi.fn(),
    loginWithToken: vi.fn(),
    logout: vi.fn(),
  },
}));

function wrapper({ children }: { children: ReactNode }) {
  return <AuthProvider>{children}</AuthProvider>;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  vi.mocked(api.getAuthStatus).mockResolvedValue({
    authenticated: false,
    user: null,
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useAuth", () => {
  it("throws if used outside AuthProvider", () => {
    expect(() => {
      renderHook(() => useAuth());
    }).toThrow("useAuth must be used within AuthProvider");
  });

  it("returns loading true initially, then false after fetch", async () => {
    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    expect(result.current.loading).toBe(false);
    expect(result.current.auth.authenticated).toBe(false);
  });

  it("populates auth from getAuthStatus response", async () => {
    vi.mocked(api.getAuthStatus).mockResolvedValue({
      authenticated: true,
      user: { login: "octocat", avatar_url: "https://example.com/avatar.png" },
    });

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    expect(result.current.auth.authenticated).toBe(true);
    expect(result.current.auth.user?.login).toBe("octocat");
  });

  it("loginWithToken calls api and updates auth state", async () => {
    const authResult = {
      authenticated: true,
      user: { login: "newuser", avatar_url: "" },
    };
    vi.mocked(api.loginWithToken).mockResolvedValue(authResult);

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    let returned;
    await act(async () => {
      returned = await result.current.loginWithToken("ghp_token123");
    });

    expect(api.loginWithToken).toHaveBeenCalledWith("ghp_token123");
    expect(returned).toEqual(authResult);
    expect(result.current.auth.authenticated).toBe(true);
    expect(result.current.auth.user?.login).toBe("newuser");
  });

  it("logout calls api and clears auth state", async () => {
    vi.mocked(api.getAuthStatus).mockResolvedValue({
      authenticated: true,
      user: { login: "octocat", avatar_url: "" },
    });
    vi.mocked(api.logout).mockResolvedValue(undefined);

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });
    expect(result.current.auth.authenticated).toBe(true);

    await act(async () => {
      await result.current.logout();
    });

    expect(api.logout).toHaveBeenCalled();
    expect(result.current.auth.authenticated).toBe(false);
    expect(result.current.auth.user).toBeNull();
  });

  it("polls getAuthStatus every 5 seconds", async () => {
    renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    // Reset call count after initial fetch(es) so we can assert the poll
    vi.mocked(api.getAuthStatus).mockClear();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(api.getAuthStatus).toHaveBeenCalledTimes(1);
  });

  it("handleUnauthorized triggers a refresh", async () => {
    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    const callsBefore = vi.mocked(api.getAuthStatus).mock.calls.length;

    await act(async () => {
      result.current.handleUnauthorized();
    });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    expect(vi.mocked(api.getAuthStatus).mock.calls.length).toBeGreaterThan(callsBefore);
  });

  it("sets error when loginWithToken fails", async () => {
    vi.mocked(api.loginWithToken).mockRejectedValue(new Error("bad token"));

    const { result } = renderHook(() => useAuth(), { wrapper });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });

    await act(async () => {
      try {
        await result.current.loginWithToken("bad");
      } catch {
        // expected
      }
    });

    expect(result.current.error).toContain("bad token");
  });
});
