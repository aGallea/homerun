import { describe, it, expect } from "vitest";
import { formatElapsed } from "./formatElapsed";

describe("formatElapsed", () => {
  it("returns '< 1m' for durations under 60 seconds", () => {
    expect(formatElapsed(0)).toBe("< 1m");
    expect(formatElapsed(30)).toBe("< 1m");
    expect(formatElapsed(59)).toBe("< 1m");
  });

  it("returns 'Xm' for durations under 60 minutes", () => {
    expect(formatElapsed(60)).toBe("1m");
    expect(formatElapsed(90)).toBe("1m");
    expect(formatElapsed(300)).toBe("5m");
    expect(formatElapsed(3540)).toBe("59m");
  });

  it("returns 'XhYm' for durations of 60 minutes or more", () => {
    expect(formatElapsed(3600)).toBe("1h0m");
    expect(formatElapsed(3660)).toBe("1h1m");
    expect(formatElapsed(7380)).toBe("2h3m");
  });

  it("returns '...' for null or negative input", () => {
    expect(formatElapsed(null)).toBe("...");
    expect(formatElapsed(undefined)).toBe("...");
    expect(formatElapsed(-1)).toBe("...");
  });
});
