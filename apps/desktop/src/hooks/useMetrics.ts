import { useState, useEffect, useCallback } from "react";
import type { MetricsResponse } from "../api/types";
import { api } from "../api/commands";

export function useMetrics(pollInterval = 5000) {
  const [metrics, setMetrics] = useState<MetricsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const data = await api.getMetrics();
      setMetrics(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, pollInterval);
    return () => clearInterval(interval);
  }, [refresh, pollInterval]);

  return { metrics, loading, error, refresh };
}
