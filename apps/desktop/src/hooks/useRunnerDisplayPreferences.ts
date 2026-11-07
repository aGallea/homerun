import { useEffect, useState } from "react";
import { api } from "../api/commands";

export interface RunnerDisplayPreferences {
  hideOfflineRunnersInMiniView: boolean;
  sortRunnersByActivity: boolean;
}

const defaults: RunnerDisplayPreferences = {
  hideOfflineRunnersInMiniView: false,
  sortRunnersByActivity: false,
};

export function useRunnerDisplayPreferences(): RunnerDisplayPreferences {
  const [preferences, setPreferences] = useState(defaults);

  useEffect(() => {
    let cancelled = false;

    async function refresh() {
      try {
        const saved = await api.getPreferences();
        if (cancelled) return;
        setPreferences({
          hideOfflineRunnersInMiniView: saved.hide_offline_runners_in_mini_view ?? false,
          sortRunnersByActivity: saved.sort_runners_by_activity ?? false,
        });
      } catch {
        // Keep backward-compatible defaults while the daemon is unavailable.
      }
    }

    void refresh();
    const interval = window.setInterval(() => void refresh(), 2000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  return preferences;
}
