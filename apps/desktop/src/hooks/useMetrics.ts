import { useState, useEffect, useCallback, useRef } from "react";
import type { MetricsResponse } from "../api/types";
import { api } from "../api/commands";

export function useMetrics(pollInterval = 2000) {
  const [metrics, setMetrics] = useState<MetricsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const initialFetch = useRef(true);
  const inFlight = useRef(false);

  const refresh = useCallback(async () => {
    // Skip if a request is already outstanding — the metrics endpoint can take
    // ~1s when container runners exist (it waits a Docker stats interval), and
    // the poll fires every ~2s, so without this a slow response would pile up
    // overlapping requests.
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const data = await api.getMetrics();
      setMetrics(data);
      setError(null);
    } catch (e) {
      setError(String(e));
      setMetrics(null);
    } finally {
      inFlight.current = false;
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
