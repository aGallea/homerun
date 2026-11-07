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
import { useEvents } from "./useEvents";

export function useRunners() {
  const [runners, setRunners] = useState<RunnerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pendingActions, setPendingActions] = useState<Set<string>>(new Set());
  const initialFetch = useRef(true);
  const refreshGeneration = useRef(0);
  const refreshPromise = useRef<Promise<void> | null>(null);
  const refreshQueued = useRef(false);
  const actionPromises = useRef(new Map<string, Promise<unknown>>());

  const refresh = useCallback((): Promise<void> => {
    if (refreshPromise.current) {
      refreshQueued.current = true;
      return refreshPromise.current;
    }

    const promise = (async () => {
      do {
        refreshQueued.current = false;
        const generation = ++refreshGeneration.current;
        try {
          const data = await api.listRunners();
          if (generation !== refreshGeneration.current) continue;
          setRunners(data);
          setError(null);
        } catch (cause) {
          if (generation === refreshGeneration.current) setError(String(cause));
        } finally {
          if (generation === refreshGeneration.current && initialFetch.current) {
            initialFetch.current = false;
            setLoading(false);
          }
        }
      } while (refreshQueued.current);
    })();

    refreshPromise.current = promise;
    void promise.finally(() => {
      if (refreshPromise.current === promise) refreshPromise.current = null;
    });
    return promise;
  }, []);

  useEffect(() => {
    void refresh();
    const interval = setInterval(() => void refresh(), 2000);
    return () => {
      clearInterval(interval);
      refreshGeneration.current += 1;
    };
  }, [refresh]);

  useEvents(refresh);

  const runPending = useCallback(function runPending<T>(
    id: string,
    operation: () => Promise<T>,
  ): Promise<T> {
    const existing = actionPromises.current.get(id) as Promise<T> | undefined;
    if (existing) return existing;

    setPendingActions((previous) => new Set(previous).add(id));
    const promise = operation().finally(() => {
      actionPromises.current.delete(id);
      setPendingActions((previous) => {
        const next = new Set(previous);
        next.delete(id);
        return next;
      });
    });
    actionPromises.current.set(id, promise);
    return promise;
  }, []);

  const createRunner = useCallback(
    async (request: CreateRunnerRequest) => {
      const runner = await api.createRunner(request);
      await refresh();
      return runner;
    },
    [refresh],
  );

  const deleteRunner = useCallback(
    (id: string) =>
      runPending(id, async () => {
        await api.deleteRunner(id);
        await refresh();
      }),
    [refresh, runPending],
  );

  const startRunner = useCallback(
    (id: string) =>
      runPending(id, async () => {
        await api.startRunner(id);
        await refresh();
      }),
    [refresh, runPending],
  );

  const stopRunner = useCallback(
    (id: string) =>
      runPending(id, async () => {
        await api.stopRunner(id);
        await refresh();
      }),
    [refresh, runPending],
  );

  const restartRunner = useCallback(
    (id: string) =>
      runPending(id, async () => {
        await api.restartRunner(id);
        await refresh();
      }),
    [refresh, runPending],
  );

  const createBatch = useCallback(
    async (request: CreateBatchRequest): Promise<BatchCreateResponse> => {
      const result = await api.createBatch(request);
      await refresh();
      return result;
    },
    [refresh],
  );

  const startGroup = useCallback(
    (groupId: string): Promise<GroupActionResponse> =>
      runPending(groupId, async () => {
        const result = await api.startGroup(groupId);
        await refresh();
        return result;
      }),
    [refresh, runPending],
  );

  const stopGroup = useCallback(
    (groupId: string): Promise<GroupActionResponse> =>
      runPending(groupId, async () => {
        const result = await api.stopGroup(groupId);
        await refresh();
        return result;
      }),
    [refresh, runPending],
  );

  const restartGroup = useCallback(
    (groupId: string): Promise<GroupActionResponse> =>
      runPending(groupId, async () => {
        const result = await api.restartGroup(groupId);
        await refresh();
        return result;
      }),
    [refresh, runPending],
  );

  const deleteGroup = useCallback(
    (groupId: string): Promise<GroupActionResponse> =>
      runPending(groupId, async () => {
        const result = await api.deleteGroup(groupId);
        await refresh();
        return result;
      }),
    [refresh, runPending],
  );

  const scaleGroup = useCallback(
    (groupId: string, count: number): Promise<ScaleGroupResponse> =>
      runPending(groupId, async () => {
        const result = await api.scaleGroup(groupId, count);
        await refresh();
        return result;
      }),
    [refresh, runPending],
  );

  return {
    runners,
    loading,
    error,
    refresh,
    pendingActions,
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

export type RunnersContextType = ReturnType<typeof useRunners> & {
  daemonStarting: boolean;
  handleStartDaemon: () => Promise<void>;
};
