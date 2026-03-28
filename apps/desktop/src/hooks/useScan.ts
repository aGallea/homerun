import { useState, useCallback } from "react";
import type { DiscoveredRepo } from "../api/types";
import { api } from "../api/commands";

interface ScanOptions {
  workspacePath: string | null;
  authenticated: boolean;
}

function mergeResults(local: DiscoveredRepo[], remote: DiscoveredRepo[]): DiscoveredRepo[] {
  const byName = new Map<string, DiscoveredRepo>();

  for (const repo of local) {
    byName.set(repo.full_name, repo);
  }

  for (const repo of remote) {
    const existing = byName.get(repo.full_name);
    if (existing) {
      existing.source = "both";
      for (const wf of repo.workflow_files) {
        if (!existing.workflow_files.includes(wf)) {
          existing.workflow_files.push(wf);
        }
      }
      for (const label of repo.matched_labels) {
        if (!existing.matched_labels.includes(label)) {
          existing.matched_labels.push(label);
        }
      }
    } else {
      byName.set(repo.full_name, repo);
    }
  }

  return Array.from(byName.values()).sort((a, b) => a.full_name.localeCompare(b.full_name));
}

export function useScan() {
  const [discoveredRepos, setDiscoveredRepos] = useState<DiscoveredRepo[]>([]);
  const [scanning, setScanning] = useState(false);
  const [lastScanTime, setLastScanTime] = useState<Date | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);

  const runScan = useCallback(async (options: ScanOptions) => {
    const { workspacePath, authenticated } = options;

    if (!workspacePath && !authenticated) {
      setScanError("Configure workspace path or sign in to scan.");
      return;
    }

    setScanning(true);
    setScanError(null);

    try {
      const promises: Promise<DiscoveredRepo[]>[] = [];

      if (workspacePath) {
        promises.push(api.scanLocal(workspacePath).catch(() => []));
      }
      if (authenticated) {
        promises.push(api.scanRemote().catch(() => []));
      }

      const results = await Promise.all(promises);

      let local: DiscoveredRepo[] = [];
      let remote: DiscoveredRepo[] = [];

      if (workspacePath && authenticated) {
        local = results[0];
        remote = results[1];
      } else if (workspacePath) {
        local = results[0];
      } else {
        remote = results[0];
      }

      setDiscoveredRepos(mergeResults(local, remote));
      setLastScanTime(new Date());
    } catch (e) {
      setScanError(String(e));
    } finally {
      setScanning(false);
    }
  }, []);

  const clearResults = useCallback(() => {
    setDiscoveredRepos([]);
    setLastScanTime(null);
    setScanError(null);
  }, []);

  return { discoveredRepos, scanning, lastScanTime, scanError, runScan, clearResults };
}
