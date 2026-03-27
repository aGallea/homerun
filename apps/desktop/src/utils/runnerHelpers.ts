export function elapsedSeconds(jobStartedAt: string | null | undefined): number | null {
  if (!jobStartedAt) return null;
  const started = new Date(jobStartedAt).getTime();
  if (isNaN(started)) return null;
  return Math.floor((Date.now() - started) / 1000);
}

export function jobProgress(
  jobStartedAt: string | null | undefined,
  estimatedDuration: number | null | undefined,
): number | null {
  if (!jobStartedAt || !estimatedDuration || estimatedDuration <= 0) return null;
  const elapsed = elapsedSeconds(jobStartedAt);
  if (elapsed == null) return null;
  return Math.min(elapsed / estimatedDuration, 1);
}

export function formatJobElapsed(jobStartedAt: string | null | undefined): string {
  const secs = elapsedSeconds(jobStartedAt);
  if (secs == null) return "";
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}m ${rem.toString().padStart(2, "0")}s`;
}
