import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import type { RunnerInfo, Preferences } from "../api/types";

type NotificationIcon = "active" | "error" | "offline";

interface TrackedRunner {
  name: string;
  state: string;
  lastJobKey: string | null;
  totalJobs: number;
}

function jobKey(runner: RunnerInfo): string | null {
  const job = runner.last_completed_job;
  if (!job) return null;
  return `${job.job_name}@${job.completed_at}`;
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s === 0 ? `${m}m` : `${m}m ${s}s`;
}

async function notify(title: string, body: string, icon: NotificationIcon) {
  try {
    const iconPath = await resolveResource(`resources/notifications/${icon}.png`);
    await invoke("send_notification", {
      title,
      body,
      icon_path: iconPath,
    });
  } catch (e) {
    console.error("Failed to send notification:", e);
  }
}

export function useNotifications(runners: RunnerInfo[], preferences: Preferences | null) {
  const prevRef = useRef<Map<string, TrackedRunner>>(new Map());
  const initialized = useRef(false);

  useEffect(() => {
    if (!preferences || runners.length === 0) return;

    const prev = prevRef.current;

    // On first render, just capture state without sending notifications
    if (!initialized.current) {
      const initial = new Map<string, TrackedRunner>();
      for (const r of runners) {
        initial.set(r.config.id, {
          name: r.config.name,
          state: r.state,
          lastJobKey: jobKey(r),
          totalJobs: r.jobs_completed + r.jobs_failed,
        });
      }
      prevRef.current = initial;
      initialized.current = true;
      return;
    }

    const next = new Map<string, TrackedRunner>();

    for (const r of runners) {
      const id = r.config.id;
      const name = r.config.name;
      const old = prev.get(id);
      const currentJobKey = jobKey(r);

      // Runner status change notifications
      if (preferences.notify_status_changes && old && old.state !== r.state) {
        if (r.state === "online" && old.state !== "busy") {
          notify("Runner Online", `${name} is now online and ready for jobs`, "active");
        } else if (r.state === "offline") {
          notify("Runner Offline", `${name} went offline`, "offline");
        } else if (r.state === "error") {
          notify("Runner Error", `${name} encountered an error`, "error");
        }
      }

      // Job completion notifications — use the monotonic job counter as the
      // primary trigger so we never miss a completion even when last_completed_job
      // is briefly cleared between consecutive jobs.
      const currentTotalJobs = r.jobs_completed + r.jobs_failed;
      if (
        preferences.notify_job_completions &&
        old &&
        currentTotalJobs > old.totalJobs &&
        r.last_completed_job
      ) {
        const job = r.last_completed_job;
        if (job.succeeded) {
          notify(
            "Job Completed",
            `${job.job_name} on ${name} passed in ${formatDuration(job.duration_secs)}`,
            "active",
          );
        } else {
          notify("Job Failed", `${job.job_name} on ${name} failed`, "error");
        }
      }

      next.set(id, {
        name,
        state: r.state,
        lastJobKey: currentJobKey,
        totalJobs: currentTotalJobs,
      });
    }

    // Detect deleted runners (were in prev, no longer in current)
    if (preferences.notify_status_changes) {
      for (const [id, old] of prev) {
        if (!next.has(id)) {
          notify("Runner Deleted", `${old.name} was removed`, "offline");
        }
      }
    }

    prevRef.current = next;
  }, [runners, preferences]);
}
