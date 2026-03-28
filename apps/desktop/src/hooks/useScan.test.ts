import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useScan } from "./useScan";

vi.mock("../api/commands", () => ({
  api: {
    scanLocal: vi.fn(),
    scanRemote: vi.fn(),
  },
}));

import { api } from "../api/commands";

const mockedApi = vi.mocked(api);

describe("useScan", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts with no results and not scanning", () => {
    const { result } = renderHook(() => useScan());
    expect(result.current.discoveredRepos).toEqual([]);
    expect(result.current.scanning).toBe(false);
    expect(result.current.lastScanTime).toBeNull();
  });

  it("runs remote-only scan when no workspace path", async () => {
    mockedApi.scanRemote.mockResolvedValue([
      {
        full_name: "acme/api",
        source: "remote",
        workflow_files: ["ci.yml"],
        local_path: null,
        matched_labels: ["self-hosted"],
      },
    ]);

    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: null, authenticated: true });
    });

    expect(mockedApi.scanRemote).toHaveBeenCalled();
    expect(mockedApi.scanLocal).not.toHaveBeenCalled();
    expect(result.current.discoveredRepos).toHaveLength(1);
    expect(result.current.scanning).toBe(false);
    expect(result.current.lastScanTime).not.toBeNull();
  });

  it("runs both scans and merges results with source=both", async () => {
    mockedApi.scanLocal.mockResolvedValue([
      {
        full_name: "acme/api",
        source: "local",
        workflow_files: ["ci.yml"],
        local_path: "/workspace/api",
        matched_labels: ["self-hosted"],
      },
    ]);
    mockedApi.scanRemote.mockResolvedValue([
      {
        full_name: "acme/api",
        source: "remote",
        workflow_files: ["ci.yml"],
        local_path: null,
        matched_labels: ["self-hosted"],
      },
    ]);

    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: true });
    });

    expect(result.current.discoveredRepos).toHaveLength(1);
    expect(result.current.discoveredRepos[0].source).toBe("both");
  });

  it("runs local-only scan when not authenticated", async () => {
    mockedApi.scanLocal.mockResolvedValue([]);

    const { result } = renderHook(() => useScan());

    await act(async () => {
      await result.current.runScan({ workspacePath: "/workspace", authenticated: false });
    });

    expect(mockedApi.scanLocal).toHaveBeenCalledWith("/workspace");
    expect(mockedApi.scanRemote).not.toHaveBeenCalled();
  });
});
