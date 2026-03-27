import { vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock Tauri invoke — the hard boundary that can't exist outside Tauri runtime
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
