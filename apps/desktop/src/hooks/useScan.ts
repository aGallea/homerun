import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DiscoveredRepo, ScanProgressEvent } from "../api/types";
import { api } from "../api/commands";

interface ScanOptions {
  workspacePath: string | null;
  authenticated: boolean;
}

export function useScan() {
  const [discoveredRepos, setDiscoveredRepos] = useState<DiscoveredRepo[]>([]);
  const [scanning, setScanning] = useState(false);
  const [lastScanAt, setLastScanAt] = useState<string | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [progressText, setProgressText] = useState<string | null>(null);

  // Load persisted results on mount
  useEffect(() => {
    api
      .getScanResults()
      .then((results) => {
        if (results) {
          setDiscoveredRepos(results.merged_results);
          setLastScanAt(results.last_scan_at);
        }
      })
      .catch(() => {});
  }, []);

  // Listen for scan progress events
  useEffect(() => {
    let doneCount = 0;
    let expectedDone = 0;

    const unlisten = listen<string>("scan-progress", (event) => {
      try {
        const data: ScanProgressEvent = JSON.parse(event.payload);

        switch (data.type) {
          case "started":
            expectedDone++;
            break;
          case "checking":
            setProgressText(`Scanning ${data.repo} (${data.index}/${data.total})...`);
            break;
          case "found":
            break;
          case "done":
            doneCount++;
            if (doneCount >= expectedDone) {
              api
                .getScanResults()
                .then((results) => {
                  if (results) {
                    setDiscoveredRepos(results.merged_results);
                    setLastScanAt(results.last_scan_at);
                  }
                  setScanning(false);
                  setProgressText(null);
                })
                .catch(() => {
                  setScanning(false);
                  setProgressText(null);
                });
            }
            break;
          case "cancelled":
            doneCount++;
            if (doneCount >= expectedDone) {
              setScanning(false);
              setProgressText(null);
            }
            break;
        }
      } catch {
        // Ignore malformed events
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const runScan = useCallback(async (options: ScanOptions) => {
    const { workspacePath, authenticated } = options;

    if (!workspacePath && !authenticated) {
      setScanError("Configure workspace path or sign in to scan.");
      return;
    }

    setScanning(true);
    setScanError(null);
    setProgressText("Starting scan...");

    try {
      await api.startScan(workspacePath, authenticated);
    } catch (e) {
      setScanError(String(e));
      setScanning(false);
      setProgressText(null);
    }
  }, []);

  const clearResults = useCallback(() => {
    setDiscoveredRepos([]);
    setLastScanAt(null);
    setScanError(null);
    setProgressText(null);
  }, []);

  return {
    discoveredRepos,
    scanning,
    lastScanAt,
    scanError,
    progressText,
    runScan,
    clearResults,
  };
}
