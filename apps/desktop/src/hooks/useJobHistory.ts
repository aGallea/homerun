import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/commands";
import type { JobHistoryEntry } from "../api/types";

export function useJobHistory(runnerId: string | undefined) {
  const [history, setHistory] = useState<JobHistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const refreshGeneration = useRef(0);

  const fetchHistory = useCallback(async () => {
    if (!runnerId) return;
    const generation = ++refreshGeneration.current;
    setLoading(true);
    try {
      const entries = await api.getRunnerHistory(runnerId);
      if (generation === refreshGeneration.current) setHistory(entries);
    } catch {
      // Ignore errors (the runner may not exist yet).
    } finally {
      if (generation === refreshGeneration.current) setLoading(false);
    }
  }, [runnerId]);

  useEffect(() => {
    refreshGeneration.current += 1;
    setHistory([]);
    if (!runnerId) {
      setLoading(false);
      return;
    }

    let cancelled = false;
    let timer: number | undefined;
    const poll = async () => {
      await fetchHistory();
      if (!cancelled) {
        timer = window.setTimeout(() => void poll(), 10_000);
      }
    };

    void poll();
    return () => {
      cancelled = true;
      refreshGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [runnerId, fetchHistory]);

  return { history, loading, refresh: fetchHistory };
}
