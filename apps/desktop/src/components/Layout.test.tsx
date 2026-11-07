import { render, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Layout } from "./Layout";

const mocks = vi.hoisted(() => ({
  healthCheck: vi.fn(),
  getPreferences: vi.fn(),
  useTrayIcon: vi.fn(),
  useNotifications: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("../api/commands", () => ({
  api: {
    healthCheck: mocks.healthCheck,
    getPreferences: mocks.getPreferences,
    startDaemon: vi.fn(),
  },
}));
vi.mock("../hooks/useRunners", () => ({
  useRunners: () => ({ runners: [] }),
}));
vi.mock("../hooks/useTrayIcon", () => ({ useTrayIcon: mocks.useTrayIcon }));
vi.mock("../hooks/useNotifications", () => ({ useNotifications: mocks.useNotifications }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(mocks.unlisten),
}));
vi.mock("./Sidebar", () => ({ Sidebar: () => <aside /> }));

describe("Layout", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getPreferences.mockResolvedValue(null);
  });

  it("does not report the daemon online before the first health check completes", async () => {
    let resolveHealth: ((value: boolean) => void) | undefined;
    mocks.healthCheck.mockReturnValue(
      new Promise((resolve) => {
        resolveHealth = resolve;
      }),
    );

    const view = render(
      <MemoryRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<div>content</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(mocks.useTrayIcon).toHaveBeenLastCalledWith([], false);
    expect(view.queryByText("Unable to connect to the HomeRun daemon.")).not.toBeInTheDocument();

    resolveHealth?.(true);
    await waitFor(() => expect(mocks.useTrayIcon).toHaveBeenLastCalledWith([], true));
  });
});
