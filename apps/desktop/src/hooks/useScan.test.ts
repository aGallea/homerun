import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useScan } from "./useScan";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../api/commands", () => ({
  api: {
    startScan: vi.fn().mockResolvedValue(undefined),
    cancelScan: vi.fn().mockResolvedValue(undefined),
    getScanResults: vi.fn().mockResolvedValue(null),
    scanLocal: vi.fn(),
    scanRemote: vi.fn(),
  },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

describe("useScan", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(result.current.discoveredRepos).toHaveLength(1);
    expect(result.current.lastScanAt).toBe("2026-03-28T13:00:00Z");
  });

  it("sets scanning state when runScan is called", async () => {
    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: true });
    });

    expect(result.current.scanning).toBe(true);
    expect(mockedApi.startScan).toHaveBeenCalledWith("/workspace", true);
  });

  it("sets error when neither workspace nor auth available", async () => {
    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: null, authenticated: false });
    });

    expect(result.current.scanning).toBe(false);
    expect(result.current.scanError).toBeTruthy();
  });
});
