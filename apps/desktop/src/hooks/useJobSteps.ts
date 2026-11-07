import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "../api/commands";
import type { StepInfo, StepsResponse } from "../api/types";

interface UseJobStepsResult {
  steps: StepInfo[];
  stepsDiscovered: number;
  jobName: string | null;
  loading: boolean;
  expandedStep: number | null;
  stepLogs: Record<number, string[]>;
  toggleStep: (stepNumber: number) => void;
}

export function useJobSteps(runnerId: string | undefined, isBusy: boolean): UseJobStepsResult {
  const [stepsResponse, setStepsResponse] = useState<StepsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [expandedStep, setExpandedStep] = useState<number | null>(null);
  const [stepLogs, setStepLogs] = useState<Record<number, string[]>>({});
  const logCacheRef = useRef<Record<string, string[]>>({});
  const stepsGeneration = useRef(0);
  const logsGeneration = useRef(0);

  useEffect(() => {
    const generation = ++stepsGeneration.current;
    if (!isBusy || !runnerId) {
      setStepsResponse(null);
      setExpandedStep(null);
      setStepLogs({});
      logCacheRef.current = {};
      setLoading(false);
      return;
    }

    let cancelled = false;
    let timer: number | undefined;
    setLoading(true);

    const fetchSteps = async () => {
      try {
        const data = await api.getRunnerSteps(runnerId);
        if (!cancelled && generation === stepsGeneration.current) setStepsResponse(data);
      } catch {
        // Ignore polling errors while the runner/job transitions.
      } finally {
        if (!cancelled && generation === stepsGeneration.current) {
          setLoading(false);
          timer = window.setTimeout(() => void fetchSteps(), 1000);
        }
      }
    };

    void fetchSteps();
    return () => {
      cancelled = true;
      stepsGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [isBusy, runnerId]);

  const expandedStepInfo =
    expandedStep === null
      ? null
      : (stepsResponse?.steps.find((step) => step.number === expandedStep) ?? null);
  const expandedStepStatus = expandedStepInfo?.status ?? null;
  const expandedStepCacheKey =
    expandedStep === null || !runnerId || !expandedStepInfo
      ? null
      : JSON.stringify({
          runnerId,
          jobName: stepsResponse?.job_name ?? null,
          number: expandedStepInfo.number,
          name: expandedStepInfo.name,
          startedAt: expandedStepInfo.started_at,
          completedAt: expandedStepInfo.completed_at,
        });

  useEffect(() => {
    const generation = ++logsGeneration.current;
    if (expandedStep === null || !runnerId) return;

    const isTerminal =
      expandedStepStatus !== null &&
      expandedStepStatus !== "pending" &&
      expandedStepStatus !== "running";
    const cachedLines =
      expandedStepCacheKey === null ? undefined : logCacheRef.current[expandedStepCacheKey];
    if (cachedLines !== undefined && isTerminal) {
      setStepLogs((previous) => ({ ...previous, [expandedStep]: cachedLines }));
      return;
    }

    // Do not display a previous job's step logs while the current identity is
    // being resolved or fetched. Cache entries are scoped to a concrete job
    // execution rather than only the step number.
    setStepLogs((previous) => ({ ...previous, [expandedStep]: [] }));

    let cancelled = false;
    let timer: number | undefined;
    const fetchLogs = async () => {
      try {
        const data = await api.getStepLogs(runnerId, expandedStep);
        if (cancelled || generation !== logsGeneration.current) return;
        const lines = data.lines;
        // A null/pending status is not final. Caching at that point can freeze
        // an early partial response and prevent the completed log from ever
        // being fetched after the step transitions to a terminal state.
        if (isTerminal && expandedStepCacheKey !== null) {
          logCacheRef.current[expandedStepCacheKey] = lines;
        }
        setStepLogs((previous) => ({ ...previous, [expandedStep]: lines }));
      } catch {
        if (!cancelled && generation === logsGeneration.current && cachedLines === undefined) {
          setStepLogs((previous) => ({ ...previous, [expandedStep]: [] }));
        }
      } finally {
        if (!cancelled && generation === logsGeneration.current && !isTerminal) {
          timer = window.setTimeout(() => void fetchLogs(), 5000);
        }
      }
    };

    void fetchLogs();
    return () => {
      cancelled = true;
      logsGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [expandedStep, expandedStepCacheKey, expandedStepStatus, runnerId]);

  const toggleStep = useCallback((stepNumber: number) => {
    setExpandedStep((previous) => (previous === stepNumber ? null : stepNumber));
  }, []);

  return {
    steps: stepsResponse?.steps ?? [],
    stepsDiscovered: stepsResponse?.steps_discovered ?? 0,
    jobName: stepsResponse?.job_name ?? null,
    loading,
    expandedStep,
    stepLogs,
    toggleStep,
  };
}
