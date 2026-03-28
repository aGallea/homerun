import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import type { RunnerInfo, Preferences } from "../api/types";

type NotificationIcon = "active" | "error" | "offline";

interface TrackedRunner {
  state: string;
  lastJobKey: string | null;
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
        initial.set(r.config.id, { state: r.state, lastJobKey: jobKey(r) });
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

      // Job completion notifications
      if (
        preferences.notify_job_completions &&
        old &&
        currentJobKey &&
        currentJobKey !== old.lastJobKey &&
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

      next.set(id, { state: r.state, lastJobKey: currentJobKey });
    }

    prevRef.current = next;
  }, [runners, preferences]);
}
