import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBadge } from "./StatusBadge";
import type { RunnerState } from "../api/types";

describe("StatusBadge", () => {
  const allStates: { state: RunnerState; label: string }[] = [
    { state: "creating", label: "Creating" },
    { state: "registering", label: "Registering" },
    { state: "online", label: "Online" },
    { state: "busy", label: "Busy" },
    { state: "stopping", label: "Stopping" },
    { state: "offline", label: "Offline" },
    { state: "error", label: "Error" },
    { state: "deleting", label: "Deleting" },
  ];

  it.each(allStates)("renders '$label' for state '$state'", ({ state, label }) => {
    render(<StatusBadge state={state} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  const transientStates: RunnerState[] = [
    "creating",
    "registering",
    "stopping",
    "deleting",
    "busy",
  ];

  it.each(transientStates)("renders spinner SVG for transient state '%s'", (state) => {
    const { container } = render(<StatusBadge state={state} />);
    expect(container.querySelector("svg")).toBeInTheDocument();
    expect(container.querySelector(".status-dot")).not.toBeInTheDocument();
  });

  const staticStates: RunnerState[] = ["online", "offline", "error"];

  it.each(staticStates)("renders dot (not spinner) for static state '%s'", (state) => {
    const { container } = render(<StatusBadge state={state} />);
    expect(container.querySelector(".status-dot")).toBeInTheDocument();
    expect(container.querySelector("svg")).not.toBeInTheDocument();
  });

  it("shows 'Busy: <job>' when busy with a currentJob", () => {
    render(<StatusBadge state="busy" currentJob="build-and-test" />);
    expect(screen.getByText("Busy: build-and-test")).toBeInTheDocument();
  });

  it("shows 'Busy' when busy without currentJob", () => {
    render(<StatusBadge state="busy" />);
    expect(screen.getByText("Busy")).toBeInTheDocument();
  });
});
