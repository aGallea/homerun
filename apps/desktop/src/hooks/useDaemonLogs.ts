import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/commands";
import type { DaemonLogEntry } from "../api/types";

export function useDaemonLogs(pollInterval = 2000) {
  const [logs, setLogs] = useState<DaemonLogEntry[]>([]);
  const [level, setLevel] = useState<string>("INFO");
  const [search, setSearch] = useState<string>("");
  const [follow, setFollow] = useState(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshGeneration = useRef(0);

  const fetchLogs = useCallback(async () => {
    const generation = ++refreshGeneration.current;
    try {
      const entries = await api.getDaemonLogsRecent(level, 2000, search || undefined);
      if (generation !== refreshGeneration.current) return;
      setLogs(entries);
      setError(null);
    } catch (e) {
      if (generation === refreshGeneration.current) setError(String(e));
    } finally {
      if (generation === refreshGeneration.current) setLoading(false);
    }
  }, [level, search]);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    const poll = async () => {
      await fetchLogs();
      if (!cancelled) {
        timer = window.setTimeout(() => void poll(), pollInterval);
      }
    };

    void poll();
    return () => {
      cancelled = true;
      refreshGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [fetchLogs, pollInterval]);

  return {
    logs,
    level,
    setLevel,
    search,
    setSearch,
    follow,
    setFollow,
    loading,
    error,
    refresh: fetchLogs,
  };
}
