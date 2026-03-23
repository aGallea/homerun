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
  const logCacheRef = useRef<Record<number, string[]>>({});

  // Poll steps every 1s when busy
  useEffect(() => {
    if (!isBusy || !runnerId) {
      setStepsResponse(null);
      setExpandedStep(null);
      setStepLogs({});
      logCacheRef.current = {};
      setLoading(false);
      return;
    }

    setLoading(true);

    const fetchSteps = async () => {
      try {
        const data = await api.getRunnerSteps(runnerId);
        setStepsResponse(data);
      } catch {
        // ignore errors during polling
      } finally {
        setLoading(false);
      }
    };

    fetchSteps();
    const interval = setInterval(fetchSteps, 1000);
    return () => clearInterval(interval);
  }, [isBusy, runnerId]);

  // Fetch step logs when a step is expanded, re-fetch every 5s if running
  useEffect(() => {
    if (expandedStep === null || !runnerId) return;

    const currentStep = stepsResponse?.steps.find((s) => s.number === expandedStep);
    const isRunning = currentStep?.status === "running";
    const isCached = logCacheRef.current[expandedStep] !== undefined;

    // Skip if cached and not running
    if (isCached && !isRunning) return;

    const fetchLogs = async () => {
      try {
        const data = await api.getStepLogs(runnerId, expandedStep);
        const lines = data.lines;

        // Cache only if step is completed (not running)
        const step = stepsResponse?.steps.find((s) => s.number === expandedStep);
        if (step && step.status !== "running") {
          logCacheRef.current[expandedStep] = lines;
        }

        setStepLogs((prev) => ({ ...prev, [expandedStep]: lines }));
      } catch {
        // GitHub API only returns logs for completed steps.
        // Set empty array so UI shows "No log output" instead of "Fetching logs..." forever.
        if (!logCacheRef.current[expandedStep]) {
          setStepLogs((prev) => ({ ...prev, [expandedStep]: [] }));
        }
      }
    };

    fetchLogs();

    if (!isRunning) return;

    const interval = setInterval(fetchLogs, 5000);
    return () => clearInterval(interval);
  }, [expandedStep, runnerId, stepsResponse]);

  const toggleStep = useCallback((stepNumber: number) => {
    setExpandedStep((prev) => (prev === stepNumber ? null : stepNumber));
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
