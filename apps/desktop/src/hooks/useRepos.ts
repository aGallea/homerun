import { useState, useEffect, useCallback } from "react";
import type { RepoInfo } from "../api/types";
import { api } from "../api/commands";
import { useAuth } from "./AuthContext";

export function useRepos() {
  const [repos, setRepos] = useState<RepoInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { handleUnauthorized } = useAuth();

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const data = await api.listRepos();
      setRepos(data);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("401")) {
        handleUnauthorized();
      }
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, [handleUnauthorized]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { repos, loading, error, refresh };
}
