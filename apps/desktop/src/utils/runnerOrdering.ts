import type { RunnerInfo } from "../api/types";

export function isOfflineRunner(runner: RunnerInfo): boolean {
  return runner.state === "offline" || runner.state === "error";
}

function parseTimestamp(value: string | null | undefined): number {
  if (!value) return Number.NEGATIVE_INFINITY;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : Number.NEGATIVE_INFINITY;
}

export function runnerActivityTimestamp(runner: RunnerInfo): number {
  return Math.max(
    parseTimestamp(runner.job_started_at),
    parseTimestamp(runner.last_completed_job?.completed_at),
    parseTimestamp(runner.started_at),
  );
}

export function compareRunnersByActivity(a: RunnerInfo, b: RunnerInfo): number {
  const aOffline = isOfflineRunner(a);
  const bOffline = isOfflineRunner(b);
  if (aOffline !== bOffline) return aOffline ? 1 : -1;

  const aBusy = a.state === "busy";
  const bBusy = b.state === "busy";
  if (aBusy !== bBusy) return aBusy ? -1 : 1;

  const activityDifference = runnerActivityTimestamp(b) - runnerActivityTimestamp(a);
  if (Number.isFinite(activityDifference) && activityDifference !== 0) return activityDifference;

  const aName = a.config.display_name ?? a.config.name;
  const bName = b.config.display_name ?? b.config.name;
  return aName.localeCompare(bName, undefined, { numeric: true });
}

export function sortRunnersByActivity(runners: RunnerInfo[]): RunnerInfo[] {
  return [...runners].sort(compareRunnersByActivity);
}

export function filterCompactRunners(runners: RunnerInfo[], hideOffline: boolean): RunnerInfo[] {
  return hideOffline ? runners.filter((runner) => !isOfflineRunner(runner)) : runners;
}
