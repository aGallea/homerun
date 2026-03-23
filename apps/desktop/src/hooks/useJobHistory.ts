import { useState, useEffect } from "react";
import { api } from "../api/commands";
import type { JobHistoryEntry } from "../api/types";

export function useJobHistory(runnerId: string | undefined) {
  const [history, setHistory] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!runnerId) return;

    let cancelled = false;

    async function fetchHistory() {
      setLoading(true);
      try {
        const entries = await api.getRunnerHistory(runnerId!);
        if (!cancelled) setHistory(entries);
      } catch {
        // ignore errors (runner may not exist yet)
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchHistory();
    // Refresh every 10 seconds (history doesn't change frequently)
    const timer = setInterval(fetchHistory, 10000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [runnerId]);

  return { history, loading };
}
