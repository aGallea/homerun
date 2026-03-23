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
  const lastTimestampRef = useRef<string | null>(null);

  const fetchLogs = useCallback(async () => {
    try {
      const entries = await api.getDaemonLogsRecent(level, 2000, search || undefined);
      setLogs(entries);
      if (entries.length > 0) {
        lastTimestampRef.current = entries[entries.length - 1].timestamp;
      }
      setError(null);
    } catch (e) {
      setError(String(e));
      setLogs([]);
    } finally {
      setLoading(false);
    }
  }, [level, search]);

  useEffect(() => {
    fetchLogs();
    const interval = setInterval(fetchLogs, pollInterval);
    return () => clearInterval(interval);
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
