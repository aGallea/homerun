import { useState, useEffect, useCallback, useRef } from "react";
import type { RepoInfo } from "../api/types";
import { api } from "../api/commands";
import { useAuth } from "./AuthContext";

export function useRepos(enabled = true) {
  const [repos, setRepos] = useState<RepoInfo[]>([]);
  const [loading, setLoading] = useState(enabled);
  const [error, setError] = useState<string | null>(null);
  const { handleUnauthorized } = useAuth();
  const refreshGeneration = useRef(0);

  const refresh = useCallback(async () => {
    if (!enabled) {
      refreshGeneration.current += 1;
      setRepos([]);
      setError(null);
      setLoading(false);
      return;
    }
    const generation = ++refreshGeneration.current;
    try {
      const data = await api.listRepos();
      if (generation !== refreshGeneration.current) return;
      setError(null);
      setRepos(data);
    } catch (e) {
      if (generation !== refreshGeneration.current) return;
      const msg = String(e);
      if (msg.includes("401") || msg.includes("UNAUTHORIZED")) {
        handleUnauthorized();
      }
      setError(msg);
    } finally {
      if (generation === refreshGeneration.current) setLoading(false);
    }
  }, [enabled, handleUnauthorized]);

  useEffect(() => {
    refreshGeneration.current += 1;
    if (!enabled) {
      setRepos([]);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    let timer: number | undefined;
    setLoading(true);

    const poll = async () => {
      await refresh();
      if (!cancelled) {
        timer = window.setTimeout(() => void poll(), 5000);
      }
    };

    void poll();
    return () => {
      cancelled = true;
      refreshGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [enabled, refresh]);

  return { repos, loading, error, refresh };
}
