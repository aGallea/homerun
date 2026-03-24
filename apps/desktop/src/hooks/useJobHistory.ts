import { useState, useEffect, useCallback } from "react";
import { api } from "../api/commands";
import type { JobHistoryEntry } from "../api/types";

export function useJobHistory(runnerId: string | undefined) {
  const [history, setHistory] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchHistory = useCallback(async () => {
    if (!runnerId) return;
    setLoading(true);
    try {
      const entries = await api.getRunnerHistory(runnerId);
      setHistory(entries);
    } catch {
      // ignore errors (runner may not exist yet)
    } finally {
      setLoading(false);
    }
  }, [runnerId]);

  useEffect(() => {
    if (!runnerId) return;
    fetchHistory();
    const timer = setInterval(fetchHistory, 10000);
    return () => clearInterval(timer);
  }, [runnerId, fetchHistory]);

  return { history, loading, refresh: fetchHistory };
}
