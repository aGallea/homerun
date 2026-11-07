import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { Event } from "@tauri-apps/api/event";
import { useScan } from "./useScan";

let progressListener: ((event: Event<string>) => void) | undefined;
let delayListener = false;
let resolveListener: (() => void) | undefined;

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockImplementation((_name: string, listener: (event: Event<string>) => void) => {
    progressListener = listener;
    if (delayListener) {
      return new Promise<() => void>((resolve) => {
        resolveListener = () => resolve(() => {});
      });
    }
    return Promise.resolve(() => {});
  }),
}));

vi.mock("../api/commands", () => ({
  api: {
    startScan: vi.fn().mockResolvedValue(["local-1", "remote-1"]),
    cancelScan: vi.fn().mockResolvedValue({ cancelled: true }),
    getScanResults: vi.fn().mockResolvedValue(null),
  },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

function emitProgress(payload: object) {
  progressListener?.({ payload: JSON.stringify(payload) } as Event<string>);
}

describe("useScan", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    progressListener = undefined;
    delayListener = false;
    resolveListener = undefined;
    mockedApi.startScan.mockResolvedValue(["local-1", "remote-1"]);
    mockedApi.getScanResults.mockResolvedValue(null);
  });

  it("starts with no results and not scanning", () => {
    const { result } = renderHook(() => useScan());
    expect(result.current.discoveredRepos).toEqual([]);
    expect(result.current.scanning).toBe(false);
    expect(result.current.lastScanAt).toBeNull();
    expect(result.current.progressText).toBeNull();
  });

  it("loads persisted results on mount", async () => {
    mockedApi.getScanResults.mockResolvedValue({
      last_scan_at: "2026-03-28T13:00:00Z",
      local_results: [],
      remote_results: [],
      merged_results: [
        {
          full_name: "acme/api",
          source: "local",
          workflow_files: ["ci.yml"],
          local_path: null,
          matched_labels: ["self-hosted"],
        },
      ],
    });

    const { result } = renderHook(() => useScan());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.discoveredRepos).toHaveLength(1);
    expect(result.current.lastScanAt).toBe("2026-03-28T13:00:00Z");
  });

  it("waits for the progress listener before starting a daemon scan", async () => {
    delayListener = true;
    const { result, unmount } = renderHook(() => useScan());
    let runPromise!: Promise<void>;

    await act(async () => {
      runPromise = result.current.runScan({ workspacePath: "/workspace", authenticated: true });
      await Promise.resolve();
    });

    expect(mockedApi.startScan).not.toHaveBeenCalled();

    await act(async () => {
      resolveListener?.();
      await runPromise;
    });

    expect(mockedApi.startScan).toHaveBeenCalledTimes(1);
    unmount();
  });

  it("tracks exact scan IDs and finishes only after every source is terminal", async () => {
    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: true });
    });
    expect(result.current.scanning).toBe(true);

    act(() => {
      emitProgress({
        type: "done",
        scan_id: "local-1",
        scan_type: "local",
        total_found: 1,
        total_checked: 1,
      });
    });
    expect(result.current.scanning).toBe(true);

    await act(async () => {
      emitProgress({
        type: "done",
        scan_id: "remote-1",
        scan_type: "remote",
        total_found: 2,
        total_checked: 2,
      });
      await Promise.resolve();
    });
    expect(result.current.scanning).toBe(false);
  });

  it("ignores stale terminal events from an unknown scan", async () => {
    const { result } = renderHook(() => useScan());
    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: true });
    });

    act(() => {
      emitProgress({
        type: "done",
        scan_id: "old-scan",
        scan_type: "local",
        total_found: 0,
        total_checked: 0,
      });
    });
    expect(result.current.scanning).toBe(true);
  });

  it("exposes cancellation for every active scan ID", async () => {
    const { result } = renderHook(() => useScan());
    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: true });
      await result.current.cancelScan();
    });

    expect(mockedApi.cancelScan).toHaveBeenCalledWith("local-1");
    expect(mockedApi.cancelScan).toHaveBeenCalledWith("remote-1");
  });

  it("surfaces terminal scan failures and releases scanning state", async () => {
    mockedApi.startScan.mockResolvedValue(["remote-1"]);
    const { result } = renderHook(() => useScan());
    await act(async () => {
      await result.current.runScan({ workspacePath: null, authenticated: true });
    });

    await act(async () => {
      emitProgress({
        type: "failed",
        scan_id: "remote-1",
        scan_type: "remote",
        message: "GitHub unavailable",
      });
      await Promise.resolve();
    });

    expect(result.current.scanning).toBe(false);
    expect(result.current.scanError).toContain("GitHub unavailable");
  });

  it("cancels IDs returned after cancellation was requested", async () => {
    let resolveStart: ((ids: string[]) => void) | undefined;
    mockedApi.startScan.mockReturnValue(
      new Promise<string[]>((resolve) => {
        resolveStart = resolve;
      }),
    );
    const { result } = renderHook(() => useScan());

    let runPromise: Promise<void> | undefined;
    await act(async () => {
      runPromise = result.current.runScan({ workspacePath: "/workspace", authenticated: true });
      await Promise.resolve();
      await result.current.cancelScan();
    });

    await act(async () => {
      resolveStart?.(["late-1"]);
      await runPromise;
    });
    expect(mockedApi.cancelScan).toHaveBeenCalledWith("late-1");

    await act(async () => {
      emitProgress({ type: "cancelled", scan_id: "late-1", scan_type: "local" });
      await Promise.resolve();
    });
    expect(result.current.scanning).toBe(false);
  });

  it("does not re-add a scan that finished before startScan returned", async () => {
    let resolveStart: ((ids: string[]) => void) | undefined;
    mockedApi.startScan.mockReturnValue(
      new Promise<string[]>((resolve) => {
        resolveStart = resolve;
      }),
    );
    const { result } = renderHook(() => useScan());

    let runPromise: Promise<void> | undefined;
    await act(async () => {
      runPromise = result.current.runScan({ workspacePath: "/workspace", authenticated: true });
      await Promise.resolve();
    });
    act(() => {
      emitProgress({ type: "started", scan_id: "fast-1", scan_type: "local", total: 1 });
      emitProgress({
        type: "done",
        scan_id: "fast-1",
        scan_type: "local",
        total_found: 0,
        total_checked: 1,
      });
    });

    await act(async () => {
      resolveStart?.(["fast-1"]);
      await runPromise;
    });
    expect(result.current.scanning).toBe(false);
  });

  it("retains a failure emitted before startScan returns without a started event", async () => {
    let resolveStart: ((ids: string[]) => void) | undefined;
    mockedApi.startScan.mockReturnValue(
      new Promise<string[]>((resolve) => {
        resolveStart = resolve;
      }),
    );
    const { result } = renderHook(() => useScan());

    let runPromise: Promise<void> | undefined;
    await act(async () => {
      runPromise = result.current.runScan({ workspacePath: null, authenticated: true });
      await Promise.resolve();
    });
    act(() => {
      emitProgress({
        type: "failed",
        scan_id: "fast-failure",
        scan_type: "remote",
        message: "Connection refused",
      });
    });

    await act(async () => {
      resolveStart?.(["fast-failure"]);
      await runPromise;
    });

    expect(result.current.scanning).toBe(false);
    expect(result.current.scanError).toContain("Connection refused");
  });

  it("sets error when neither workspace nor auth is available", async () => {
    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: null, authenticated: false });
    });

    expect(result.current.scanning).toBe(false);
    expect(result.current.scanError).toBeTruthy();
  });
});
