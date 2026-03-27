import { useEffect, useRef } from "react";
import type { RunnerInfo, TrayIconState } from "../api/types";
import { api } from "../api/commands";

function computeTrayState(runners: RunnerInfo[], daemonOk: boolean): TrayIconState {
  if (!daemonOk) return "offline";
  if (runners.some((r) => r.state === "error")) return "error";
  if (runners.some((r) => r.state === "busy")) return "active";
  return "idle";
}

export function useTrayIcon(runners: RunnerInfo[], daemonOk: boolean) {
  const lastState = useRef<TrayIconState | null>(null);

  useEffect(() => {
    const state = computeTrayState(runners, daemonOk);
    if (state !== lastState.current) {
      lastState.current = state;
      api.updateTrayIcon(state).catch(() => {});
    }
  }, [runners, daemonOk]);
}
