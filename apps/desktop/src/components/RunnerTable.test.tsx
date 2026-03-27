import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { RunnerTable } from "./RunnerTable";
import { makeRunner } from "../test/factories";

const noop = () => {};

function renderTable(props: Partial<Parameters<typeof RunnerTable>[0]> = {}) {
  const defaultProps = {
    runners: [],
    onStart: noop,
    onStop: noop,
    onRestart: noop,
    onDelete: noop,
    onStartGroup: noop,
    onStopGroup: noop,
    onRestartGroup: noop,
    onDeleteGroup: noop,
    onScaleGroup: noop,
    ...props,
  };
  return render(
    <MemoryRouter>
      <RunnerTable {...defaultProps} />
    </MemoryRouter>,
  );
}

describe("RunnerTable", () => {
  it("shows empty state when no runners", () => {
    renderTable({ runners: [] });
    expect(screen.getByText("No runners yet.")).toBeInTheDocument();
  });

  it("renders a solo runner row with name and status", () => {
    renderTable({
      runners: [makeRunner({ name: "my-runner", state: "online" })],
    });
    expect(screen.getByText("my-runner")).toBeInTheDocument();
    expect(screen.getByText("Online")).toBeInTheDocument();
  });

  it("renders multiple solo runners sorted by name", () => {
    renderTable({
      runners: [makeRunner({ name: "z-runner" }), makeRunner({ name: "a-runner" })],
    });
    const names = screen.getAllByText(/runner/).map((el) => el.textContent);
    expect(names.indexOf("a-runner")).toBeLessThan(names.indexOf("z-runner"));
  });

  it("groups runners with group_id by name prefix and repo", () => {
    const runners = [
      makeRunner({
        name: "batch-1",
        config: {
          id: "b1",
          name: "batch-1",
          repo_owner: "org",
          repo_name: "repo",
          labels: [],
          mode: "app",
          work_dir: "/tmp",
          group_id: "g1",
        },
      }),
      makeRunner({
        name: "batch-2",
        config: {
          id: "b2",
          name: "batch-2",
          repo_owner: "org",
          repo_name: "repo",
          labels: [],
          mode: "app",
          work_dir: "/tmp",
          group_id: "g1",
        },
      }),
    ];
    renderTable({ runners });
    // Group header shows prefix and count
    expect(screen.getByText("batch")).toBeInTheDocument();
    expect(screen.getByText("(2)")).toBeInTheDocument();
    // Individual runners hidden by default (group collapsed)
    expect(screen.queryByText("batch-1")).not.toBeInTheDocument();
  });

  it("expands group on click to show individual runners", () => {
    const runners = [
      makeRunner({
        name: "batch-1",
        config: {
          id: "b1",
          name: "batch-1",
          repo_owner: "org",
          repo_name: "repo",
          labels: [],
          mode: "app",
          work_dir: "/tmp",
          group_id: "g1",
        },
      }),
      makeRunner({
        name: "batch-2",
        config: {
          id: "b2",
          name: "batch-2",
          repo_owner: "org",
          repo_name: "repo",
          labels: [],
          mode: "app",
          work_dir: "/tmp",
          group_id: "g1",
        },
      }),
    ];
    renderTable({ runners });
    // Click the group row to expand
    fireEvent.click(screen.getByText("batch"));
    expect(screen.getByText("batch-1")).toBeInTheDocument();
    expect(screen.getByText("batch-2")).toBeInTheDocument();
  });

  it("hides actions in readOnly mode", () => {
    renderTable({
      runners: [makeRunner({ name: "r1", state: "offline" })],
      readOnly: true,
    });
    // RunnerActions returns null when readOnly, so no action buttons
    expect(screen.queryByTitle("Start")).not.toBeInTheDocument();
    expect(screen.queryByTitle("Delete")).not.toBeInTheDocument();
  });

  it("shows service badge for service mode runners", () => {
    renderTable({
      runners: [
        makeRunner({
          name: "svc-runner",
          config: {
            id: "svc1",
            name: "svc-runner",
            repo_owner: "org",
            repo_name: "repo",
            labels: [],
            mode: "service",
            work_dir: "/tmp",
          },
        }),
      ],
    });
    expect(screen.getByTitle("Service runner")).toBeInTheDocument();
  });

  it("applies loading state for pending actions", () => {
    const { container } = renderTable({
      runners: [makeRunner({ name: "r1" })],
      pendingActions: new Set(["r1"]),
    });
    const row = container.querySelector(".runner-row");
    expect(row).toHaveStyle({ opacity: "0.6" });
  });
});
