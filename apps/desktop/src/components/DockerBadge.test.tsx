import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DockerBadge } from "./DockerBadge";

describe("DockerBadge", () => {
  it("renders the Docker label", () => {
    render(<DockerBadge />);
    expect(screen.getByText("Docker")).toBeInTheDocument();
  });

  it("has a descriptive title", () => {
    render(<DockerBadge />);
    expect(screen.getByTitle("Runs in a Docker container")).toBeInTheDocument();
  });
});
