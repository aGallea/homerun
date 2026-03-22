import { useState, useEffect, useCallback, useRef } from "react";
import type {
  BatchCreateResponse,
  CreateBatchRequest,
  CreateRunnerRequest,
  GroupActionResponse,
  RunnerInfo,
  ScaleGroupResponse,
} from "../api/types";
import { api } from "../api/commands";

export function useRunners() {
  const [runners, setRunners] = useState<RunnerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const initialFetch = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const data = await api.listRunners();
      setRunners(data);
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

  const createBatch = useCallback(
    async (req: CreateBatchRequest): Promise<BatchCreateResponse> => {
      const result = await api.createBatch(req);
      await refresh();
      return result;
    },
    [refresh],
  );

  const startGroup = useCallback(
    async (groupId: string): Promise<GroupActionResponse> => {
      const result = await api.startGroup(groupId);
      await refresh();
      return result;
    },
    [refresh],
  );

  const stopGroup = useCallback(
    async (groupId: string): Promise<GroupActionResponse> => {
      const result = await api.stopGroup(groupId);
      await refresh();
      return result;
    },
    [refresh],
  );

  const restartGroup = useCallback(
    async (groupId: string): Promise<GroupActionResponse> => {
      const result = await api.restartGroup(groupId);
      await refresh();
      return result;
    },
    [refresh],
  );

  const deleteGroup = useCallback(
    async (groupId: string): Promise<GroupActionResponse> => {
      const result = await api.deleteGroup(groupId);
      await refresh();
      return result;
    },
    [refresh],
  );

  const scaleGroup = useCallback(
    async (groupId: string, count: number): Promise<ScaleGroupResponse> => {
      const result = await api.scaleGroup(groupId, count);
      await refresh();
      return result;
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
    createBatch,
    startGroup,
    stopGroup,
    restartGroup,
    deleteGroup,
    scaleGroup,
  };
}
