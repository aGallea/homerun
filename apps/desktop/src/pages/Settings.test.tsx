import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Settings } from "./Settings";
import type { Preferences } from "../api/types";

const mocks = vi.hoisted(() => ({
  getPreferences: vi.fn(),
  updatePreferences: vi.fn(),
  invoke: vi.fn(),
  openExternal: vi.fn(),
}));

vi.mock("../hooks/useAuth", () => ({
  useAuth: () => ({
    auth: {
      authenticated: true,
      user: { login: "octocat", avatar_url: "https://example.com/avatar.png" },
    },
    loading: false,
    error: null,
    loginWithToken: vi.fn(),
    logout: vi.fn(),
    refresh: vi.fn(),
  }),
}));

vi.mock("../api/commands", () => ({
  api: {
    getPreferences: mocks.getPreferences,
    updatePreferences: mocks.updatePreferences,
    startDeviceFlow: vi.fn(),
    pollDeviceFlow: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn().mockResolvedValue("0.9.1") }));
vi.mock("../utils/openExternal", () => ({ openExternal: mocks.openExternal }));

const preferences: Preferences = {
  start_runners_on_launch: false,
  notify_status_changes: true,
  notify_job_completions: true,
  scan_labels: ["self-hosted"],
  workspace_path: null,
  auto_scan: false,
  hide_offline_runners_in_mini_view: false,
  sort_runners_by_activity: false,
};

describe("Settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.invoke.mockResolvedValue(false);
    mocks.getPreferences.mockResolvedValue(preferences);
    mocks.updatePreferences.mockImplementation(async (value: Preferences) => value);
  });

  it("serializes rapid preference changes without losing either update", async () => {
    let resolveFirst: ((value: Preferences) => void) | undefined;
    mocks.updatePreferences
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockImplementation(async (value: Preferences) => value);

    render(<Settings />);
    const restore = await screen.findByRole("switch", { name: "Restore runners on launch" });
    const completions = screen.getByRole("switch", { name: "Job completions" });
    await waitFor(() => expect(restore).not.toBeDisabled());

    act(() => {
      restore.click();
      completions.click();
    });
    expect(mocks.updatePreferences).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst?.({ ...preferences, start_runners_on_launch: true });
      await Promise.resolve();
    });

    await waitFor(() => expect(mocks.updatePreferences).toHaveBeenCalledTimes(2));
    expect(mocks.updatePreferences).toHaveBeenLastCalledWith(
      expect.objectContaining({
        start_runners_on_launch: true,
        notify_job_completions: false,
      }),
    );
  });

  it("persists the newest queued snapshot after the screen unmounts", async () => {
    let resolveFirst: ((value: Preferences) => void) | undefined;
    mocks.updatePreferences
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockImplementation(async (value: Preferences) => value);

    const view = render(<Settings />);
    const restore = await screen.findByRole("switch", { name: "Restore runners on launch" });
    const completions = screen.getByRole("switch", { name: "Job completions" });
    await waitFor(() => expect(restore).not.toBeDisabled());

    act(() => {
      restore.click();
      completions.click();
    });
    expect(mocks.updatePreferences).toHaveBeenCalledTimes(1);
    view.unmount();

    await act(async () => {
      resolveFirst?.({ ...preferences, start_runners_on_launch: true });
      await Promise.resolve();
    });

    await waitFor(() => expect(mocks.updatePreferences).toHaveBeenCalledTimes(2));
    expect(mocks.updatePreferences).toHaveBeenLastCalledWith(
      expect.objectContaining({
        start_runners_on_launch: true,
        notify_job_completions: false,
      }),
    );
  });

  it("persists runner list display preferences", async () => {
    render(<Settings />);
    const hideOffline = await screen.findByRole("switch", {
      name: "Hide offline runners in Mini View",
    });
    await waitFor(() => expect(hideOffline).not.toBeDisabled());
    fireEvent.click(hideOffline);
    await waitFor(() =>
      expect(mocks.updatePreferences).toHaveBeenCalledWith(
        expect.objectContaining({ hide_offline_runners_in_mini_view: true }),
      ),
    );
  });

  it("opens About links through the Tauri shell bridge", async () => {
    render(<Settings />);
    const repository = await screen.findByRole("link", { name: "Repository" });
    fireEvent.click(repository);
    expect(mocks.openExternal).toHaveBeenCalledWith("https://github.com/lgg/homerun");
  });

  it("keeps preference controls disabled until saved preferences are loaded", async () => {
    let resolvePreferences: ((value: Preferences) => void) | undefined;
    mocks.getPreferences.mockReturnValue(
      new Promise((resolve) => {
        resolvePreferences = resolve;
      }),
    );

    render(<Settings />);
    const restoreToggle = screen.getByRole("switch", { name: "Restore runners on launch" });
    expect(restoreToggle).toBeDisabled();
    fireEvent.click(restoreToggle);
    expect(mocks.updatePreferences).not.toHaveBeenCalled();

    await act(async () => {
      resolvePreferences?.(preferences);
      await Promise.resolve();
    });

    await waitFor(() => expect(restoreToggle).not.toBeDisabled());
  });
});
