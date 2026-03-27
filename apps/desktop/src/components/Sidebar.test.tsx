import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { AuthProvider } from "../hooks/AuthContext";
import { api } from "../api/commands";

vi.mock("../api/commands", () => ({
  api: {
    getAuthStatus: vi.fn(),
  },
}));

function renderSidebar(
  props: { collapsed?: boolean; runners?: Parameters<typeof Sidebar>[0]["runners"] } = {},
) {
  return render(
    <MemoryRouter>
      <AuthProvider>
        <Sidebar collapsed={props.collapsed ?? false} runners={props.runners ?? []} />
      </AuthProvider>
    </MemoryRouter>,
  );
}

describe("Sidebar", () => {
  beforeEach(() => {
    vi.mocked(api.getAuthStatus).mockResolvedValue({
      authenticated: false,
      user: null,
    });
  });

  it("renders all 4 navigation links", () => {
    renderSidebar();
    expect(screen.getByText("Runners")).toBeInTheDocument();
    expect(screen.getByText("Repositories")).toBeInTheDocument();
    expect(screen.getByText("Daemon")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("shows 'Sign in' button when unauthenticated", () => {
    renderSidebar();
    expect(screen.getByText("Sign in")).toBeInTheDocument();
  });

  it("shows username and avatar when authenticated", async () => {
    vi.mocked(api.getAuthStatus).mockResolvedValue({
      authenticated: true,
      user: { login: "testuser", avatar_url: "https://example.com/avatar.png" },
    });
    renderSidebar();
    // Wait for auth to load
    expect(await screen.findByText("testuser")).toBeInTheDocument();
    const avatar = screen.getByRole("img", { name: "testuser" }) as HTMLImageElement;
    expect(avatar.src).toBe("https://example.com/avatar.png");
  });

  it("hides labels when collapsed", () => {
    renderSidebar({ collapsed: true });
    expect(screen.queryByText("Runners")).not.toBeInTheDocument();
    expect(screen.queryByText("Repositories")).not.toBeInTheDocument();
    // Icons should still be present
    const icons = document.querySelectorAll(".sidebar-icon");
    expect(icons.length).toBe(4);
  });

  it("hides 'Sign in' text when collapsed (shows icon only)", () => {
    renderSidebar({ collapsed: true });
    expect(screen.queryByText("Sign in")).not.toBeInTheDocument();
  });

  it("shows HomeRun title when expanded", () => {
    renderSidebar({ collapsed: false });
    expect(screen.getByText("HomeRun")).toBeInTheDocument();
  });

  it("hides HomeRun title when collapsed", () => {
    renderSidebar({ collapsed: true });
    expect(screen.queryByText("HomeRun")).not.toBeInTheDocument();
  });
});
