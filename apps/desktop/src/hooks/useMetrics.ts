import { useState, useEffect, useCallback, useRef } from "react";
import type { MetricsResponse } from "../api/types";
import { api } from "../api/commands";

export function useMetrics(pollInterval = 5000) {
  const [metrics, setMetrics] = useState<MetricsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const initialFetch = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const data = await api.getMetrics();
      setMetrics(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      if (initialFetch.current) {
        initialFetch.current = false;
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, pollInterval);
    return () => clearInterval(interval);
  }, [refresh, pollInterval]);

  return { metrics, loading, error, refresh };
}
