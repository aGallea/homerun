import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MiniView } from "./MiniView";

const mocks = vi.hoisted(() => ({
  useRunners: vi.fn(),
  useTrayIcon: vi.fn(),
  setSize: vi.fn().mockResolvedValue(undefined),
  saveMiniPosition: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../hooks/useRunners", () => ({ useRunners: mocks.useRunners }));
vi.mock("../hooks/useTrayIcon", () => ({ useTrayIcon: mocks.useTrayIcon }));
vi.mock("../api/commands", () => ({
  api: { saveMiniPosition: mocks.saveMiniPosition },
}));
vi.mock("@tauri-apps/api/window", () => ({
  LogicalSize: class LogicalSize {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
  getCurrentWindow: () => ({
    setSize: mocks.setSize,
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
    scaleFactor: vi.fn().mockResolvedValue(1),
  }),
}));

describe("MiniView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useRunners.mockReturnValue({
      runners: [],
      loading: true,
      error: null,
    });
  });

  it("does not report the daemon online before the first runner request completes", () => {
    const { container } = render(<MiniView />);
    expect(container.querySelector(".mini-health-dot")).toHaveClass("offline");
    expect(mocks.useTrayIcon).toHaveBeenCalledWith([], false);
  });
});
