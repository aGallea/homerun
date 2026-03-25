import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ActiveRunners } from "./ActiveRunners";
import type { RunnerInfo } from "../api/types";

const FIXED_NOW = new Date("2026-03-25T12:00:00Z").getTime();

function makeRunner(overrides: Partial<RunnerInfo> & { name: string }): RunnerInfo {
  return {
    config: {
      id: overrides.name,
      name: overrides.name,
      repo_owner: "org",
      repo_name: "repo",
      labels: [],
      mode: "service",
      work_dir: "/tmp",
    },
    state: overrides.state ?? "online",
    pid: 1234,
    uptime_secs: 100,
    jobs_completed: 0,
    jobs_failed: 0,
    current_job: overrides.current_job ?? null,
    job_started_at: overrides.job_started_at ?? null,
    ...overrides,
  };
}

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ActiveRunners", () => {
  it("renders nothing when no runners are busy", () => {
    const { container } = renderWithRouter(
      <ActiveRunners runners={[makeRunner({ name: "r1", state: "online" })]} collapsed={false} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders busy runners with name, job, and elapsed time", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const threeMinAgo = new Date(FIXED_NOW - 180_000).toISOString();
    renderWithRouter(
      <ActiveRunners
        runners={[
          makeRunner({
            name: "runner-1",
            state: "busy",
            current_job: "build-and-test",
            job_started_at: threeMinAgo,
          }),
        ]}
        collapsed={false}
      />,
    );
    expect(screen.getByText("runner-1")).toBeTruthy();
    expect(screen.getByText("build-and-test")).toBeTruthy();
    expect(screen.getByText("3m")).toBeTruthy();
  });

  it("shows 'Starting...' when current_job is null", () => {
    renderWithRouter(
      <ActiveRunners
        runners={[makeRunner({ name: "r1", state: "busy", current_job: null })]}
        collapsed={false}
      />,
    );
    expect(screen.getByText("Starting...")).toBeTruthy();
  });

  it("caps at 3 runners and shows overflow link", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const runners = Array.from({ length: 5 }, (_, i) =>
      makeRunner({
        name: `runner-${i}`,
        state: "busy",
        current_job: `job-${i}`,
        job_started_at: new Date(FIXED_NOW - i * 60_000).toISOString(),
      }),
    );
    renderWithRouter(<ActiveRunners runners={runners} collapsed={false} />);
    expect(screen.getByText("runner-0")).toBeTruthy();
    expect(screen.getByText("runner-1")).toBeTruthy();
    expect(screen.getByText("runner-2")).toBeTruthy();
    expect(screen.queryByText("runner-3")).toBeNull();
    expect(screen.getByText("+2 more runners")).toBeTruthy();
  });

  it("sorts by job_started_at descending (most recent first)", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const runners = [
      makeRunner({
        name: "old-runner",
        state: "busy",
        current_job: "j1",
        job_started_at: new Date(FIXED_NOW - 300_000).toISOString(),
      }),
      makeRunner({
        name: "new-runner",
        state: "busy",
        current_job: "j2",
        job_started_at: new Date(FIXED_NOW - 60_000).toISOString(),
      }),
    ];
    renderWithRouter(<ActiveRunners runners={runners} collapsed={false} />);
    const entries = document.querySelectorAll(".sidebar-active-entry");
    expect(entries[0].textContent).toContain("new-runner");
    expect(entries[1].textContent).toContain("old-runner");
  });

  it("sorts runners with null job_started_at last", () => {
    vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
    const runners = [
      makeRunner({
        name: "no-time",
        state: "busy",
        current_job: "j1",
        job_started_at: null,
      }),
      makeRunner({
        name: "has-time",
        state: "busy",
        current_job: "j2",
        job_started_at: new Date(FIXED_NOW - 60_000).toISOString(),
      }),
    ];
    renderWithRouter(<ActiveRunners runners={runners} collapsed={false} />);
    const entries = document.querySelectorAll(".sidebar-active-entry");
    expect(entries[0].textContent).toContain("has-time");
    expect(entries[1].textContent).toContain("no-time");
  });

  it("shows only badge with count when collapsed", () => {
    renderWithRouter(
      <ActiveRunners
        runners={[
          makeRunner({ name: "r1", state: "busy", current_job: "j1" }),
          makeRunner({ name: "r2", state: "busy", current_job: "j2" }),
        ]}
        collapsed={true}
      />,
    );
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.queryByText("r1")).toBeNull();
    expect(screen.queryByText("r2")).toBeNull();
  });
});
