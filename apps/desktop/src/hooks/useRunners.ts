import { useState, useEffect, useCallback } from "react";
import type { RunnerInfo, CreateRunnerRequest } from "../api/types";
import { api } from "../api/commands";

export function useRunners() {
  const [runners, setRunners] = useState<RunnerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const data = await api.listRunners();
      setRunners(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);
    return () => clearInterval(interval);
  }, [refresh]);

  const createRunner = useCallback(
    async (req: CreateRunnerRequest) => {
      const runner = await api.createRunner(req);
      await refresh();
      return runner;
    },
    [refresh],
  );

  const deleteRunner = useCallback(
    async (id: string) => {
      await api.deleteRunner(id);
      await refresh();
    },
    [refresh],
  );

  const startRunner = useCallback(
    async (id: string) => {
      await api.startRunner(id);
      await refresh();
    },
    [refresh],
  );

  const stopRunner = useCallback(
    async (id: string) => {
      await api.stopRunner(id);
      await refresh();
    },
    [refresh],
  );

  const restartRunner = useCallback(
    async (id: string) => {
      await api.restartRunner(id);
      await refresh();
    },
    [refresh],
  );

  return {
    runners,
    loading,
    error,
    refresh,
    createRunner,
    deleteRunner,
    startRunner,
    stopRunner,
    restartRunner,
  };
}
