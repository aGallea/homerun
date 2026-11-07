import { useState, useMemo, useEffect, useRef } from "react";
import { useNavigate, useOutletContext } from "react-router";
import { useRepos } from "../hooks/useRepos";
import { useScan } from "../hooks/useScan";
import type { RunnersContextType } from "../hooks/useRunners";
import type { Preferences, DiscoveredRepo, RepoInfo } from "../api/types";
import { useAuth } from "../hooks/useAuth";
import { NewRunnerWizard } from "../components/NewRunnerWizard";
import { api } from "../api/commands";
import { isLaunchableRepository } from "../utils/repository";

export function Repositories() {
  const { auth, loading: authLoading } = useAuth();
  const navigate = useNavigate();
  const { repos, loading: reposLoading, error: reposError } = useRepos(auth.authenticated);
  const { runners, createRunner, createBatch } = useOutletContext<RunnersContextType>();
  const { discoveredRepos, scanning, lastScanAt, scanError, progressText, runScan, cancelScan } =
    useScan();
  const [search, setSearch] = useState("");
  const [showEnriched, setShowEnriched] = useState(true);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardRepo, setWizardRepo] = useState<RepoInfo | null>(null);
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [labelFilter, setLabelFilter] = useState<string | null>(null);
  const [sourceFilter, setSourceFilter] = useState<string | null>(null);
  const autoScanAttempted = useRef(false);

  // Load preferences on mount
  useEffect(() => {
    api
      .getPreferences()
      .then(setPreferences)
      .catch(() => {});
  }, []);

  // Auto-scan at most once for this page mount. Depending directly on the
  // scanning state without this latch would start a new scan after every finish.
  useEffect(() => {
    if (!preferences || authLoading || autoScanAttempted.current) return;
    if (scanning) {
      autoScanAttempted.current = true;
      return;
    }

    autoScanAttempted.current = true;
    if (preferences.auto_scan && (preferences.workspace_path || auth.authenticated)) {
      void runScan({
        workspacePath: preferences.workspace_path,
        authenticated: auth.authenticated,
      });
    }
  }, [scanning, preferences, auth.authenticated, authLoading, runScan]);

  // Count runners per repo full_name
  const runnerCountByRepo = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of runners) {
      const key = `${r.config.repo_owner}/${r.config.repo_name}`;
      map.set(key, (map.get(key) ?? 0) + 1);
    }
    return map;
  }, [runners]);

  // Map discovered repos by full_name for O(1) lookup
  const discoveryMap = useMemo(() => {
    const map = new Map<string, DiscoveredRepo>();
    for (const d of discoveredRepos) {
      map.set(d.full_name, d);
    }
    return map;
  }, [discoveredRepos]);

  // Unique labels from scan results
  const availableLabels = useMemo(() => {
    const labels = new Set<string>();
    for (const d of discoveredRepos) {
      for (const l of d.matched_labels) {
        labels.add(l);
      }
    }
    return Array.from(labels).sort();
  }, [discoveredRepos]);

  // Merge GitHub repos with local-only discovered repos
  const allRepos = useMemo(() => {
    const repoNames = new Set(repos.map((r) => r.full_name));
    const localOnly = discoveredRepos
      .filter((d) => !repoNames.has(d.full_name))
      .map((d) => ({
        id: 0,
        full_name: d.full_name,
        name: d.full_name.split("/")[1] ?? d.full_name,
        owner: d.full_name.split("/")[0] ?? "",
        private: false,
        html_url: "",
        is_org: false,
      }));
    return [...repos, ...localOnly];
  }, [repos, discoveredRepos]);

  // Scan summary stats
  const scanSummary = useMemo(() => {
    if (!lastScanAt || discoveredRepos.length === 0) return null;
    let local = 0;
    let remote = 0;
    let both = 0;
    for (const d of discoveredRepos) {
      if (d.source === "local") local++;
      else if (d.source === "remote") remote++;
      else if (d.source === "both") both++;
    }
    return { total: discoveredRepos.length, local, remote, both };
  }, [discoveredRepos, lastScanAt]);

  const hasScanned = lastScanAt !== null;
  const needsConfig = !preferences?.workspace_path;

  // Filter repos by search, label, and source
  const filteredRepos = useMemo(() => {
    const q = search.toLowerCase();
    return allRepos.filter((r) => {
      if (!r.full_name.toLowerCase().includes(q)) return false;

      const discovered = discoveryMap.get(r.full_name);

      if (labelFilter) {
        if (!discovered || !discovered.matched_labels.includes(labelFilter)) return false;
      }

      if (sourceFilter) {
        if (!discovered) return false;
        if (
          sourceFilter === "local" &&
          discovered.source !== "local" &&
          discovered.source !== "both"
        )
          return false;
        if (
          sourceFilter === "remote" &&
          discovered.source !== "remote" &&
          discovered.source !== "both"
        )
          return false;
      }

      return true;
    });
  }, [allRepos, search, discoveryMap, labelFilter, sourceFilter]);

  function handleScan() {
    runScan({
      workspacePath: preferences?.workspace_path ?? null,
      authenticated: auth.authenticated,
    });
  }

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Repositories</h1>
        <div style={{ display: "flex", gap: 8 }}>
          {discoveredRepos.length > 0 && (
            <button
              className="btn btn-secondary"
              onClick={() => setShowEnriched((v) => !v)}
              style={{ fontSize: 12 }}
            >
              {showEnriched ? "Plain view" : "Enriched view"}
            </button>
          )}
          {scanning ? (
            <button className="btn btn-secondary" onClick={() => void cancelScan()}>
              Cancel scan
            </button>
          ) : (
            <button className="btn btn-secondary" onClick={handleScan}>
              Scan
            </button>
          )}
          {auth.authenticated ? (
            <button
              className="btn btn-primary"
              onClick={() => {
                setWizardRepo(null);
                setWizardOpen(true);
              }}
            >
              + Add Runner
            </button>
          ) : (
            <button className="btn btn-primary" onClick={() => navigate("/settings")}>
              Sign in for remote repos
            </button>
          )}
        </div>
      </div>

      {!auth.authenticated && (
        <div className="card" style={{ marginBottom: 16, padding: "14px 18px" }}>
          <strong style={{ fontSize: 13 }}>Local discovery is available without signing in.</strong>
          <div className="text-muted" style={{ fontSize: 12, marginTop: 4 }}>
            Configure a workspace path and scan local repositories. Sign in only to browse GitHub
            repositories or create runners.
          </div>
        </div>
      )}

      {/* Search */}
      <div style={{ marginBottom: 20 }}>
        <input
          type="text"
          placeholder="Search repositories..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          style={{ width: "100%", maxWidth: 400 }}
        />
      </div>

      {/* Progress text */}
      {scanning && progressText && (
        <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 12 }}>
          {progressText}
        </div>
      )}

      {/* Filter bar (shown after scan) */}
      {hasScanned && showEnriched && (availableLabels.length > 0 || discoveredRepos.length > 0) && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: 8,
            alignItems: "center",
            marginBottom: 16,
          }}
        >
          <span style={{ fontSize: 12, color: "var(--text-secondary)", marginRight: 4 }}>
            Labels:
          </span>
          <FilterPill
            label="All"
            active={labelFilter === null}
            onClick={() => setLabelFilter(null)}
          />
          {availableLabels.map((l) => (
            <FilterPill
              key={l}
              label={l}
              active={labelFilter === l}
              onClick={() => setLabelFilter(labelFilter === l ? null : l)}
            />
          ))}

          <span
            style={{ fontSize: 12, color: "var(--text-secondary)", marginLeft: 12, marginRight: 4 }}
          >
            Source:
          </span>
          <FilterPill
            label="All"
            active={sourceFilter === null}
            onClick={() => setSourceFilter(null)}
          />
          <FilterPill
            label="Local"
            active={sourceFilter === "local"}
            onClick={() => setSourceFilter(sourceFilter === "local" ? null : "local")}
          />
          <FilterPill
            label="Remote"
            active={sourceFilter === "remote"}
            onClick={() => setSourceFilter(sourceFilter === "remote" ? null : "remote")}
          />
        </div>
      )}

      {/* Settings hint when workspace path not configured */}
      {!hasScanned && needsConfig && (
        <div style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 16 }}>
          Scan for repos with self-hosted workflows.{" "}
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              navigate("/settings");
            }}
            style={{ color: "var(--accent-blue)" }}
          >
            Configure labels and workspace path in Settings.
          </a>
        </div>
      )}

      {/* Scan status bar */}
      {hasScanned &&
        (showEnriched && scanSummary ? (
          <div
            style={{
              background: "rgba(63, 185, 80, 0.1)",
              border: "1px solid rgba(63, 185, 80, 0.3)",
              borderRadius: 6,
              padding: "8px 14px",
              marginBottom: 16,
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
            }}
          >
            <span style={{ fontSize: 12, color: "var(--accent-green)" }}>
              Found {scanSummary.total} repo{scanSummary.total !== 1 ? "s" : ""} with matching
              workflows ({scanSummary.local} local, {scanSummary.remote} remote, {scanSummary.both}{" "}
              both)
            </span>
            <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
              Last scan: {new Date(lastScanAt!).toLocaleString()}
            </span>
          </div>
        ) : (
          <div style={{ fontSize: 11, color: "var(--text-secondary)", marginBottom: 12 }}>
            Last scan: {new Date(lastScanAt!).toLocaleString()}
          </div>
        ))}

      {/* Scan error */}
      {scanError && (
        <div className="error-banner" style={{ marginBottom: 16 }}>
          {scanError}
        </div>
      )}

      {auth.authenticated && reposError && (
        <div className="error-banner" style={{ marginBottom: 16 }}>
          {reposError}
        </div>
      )}

      {auth.authenticated && reposLoading ? (
        <p className="text-muted">Loading repositories...</p>
      ) : filteredRepos.length === 0 ? (
        <div className="card" style={{ textAlign: "center", padding: "40px" }}>
          <p className="text-muted">
            {search ? "No repositories match your search." : "No repositories found."}
          </p>
          <p className="text-muted" style={{ fontSize: 12, marginTop: 8 }}>
            {auth.authenticated
              ? "Check repository access or run discovery again."
              : preferences?.workspace_path
                ? "Run a local scan, or sign in to include GitHub repositories."
                : "Configure a workspace path in Settings to discover local repositories."}
          </p>
        </div>
      ) : (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))",
            gap: 16,
          }}
        >
          {filteredRepos.map((repo) => {
            const count = runnerCountByRepo.get(repo.full_name) ?? 0;
            const discovered = discoveryMap.get(repo.full_name);
            const isDimmed = hasScanned && showEnriched && !discovered;
            const canCreateRunner = isLaunchableRepository(repo.full_name);
            return (
              <div
                key={repo.id || repo.full_name}
                className="card"
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
                  opacity: isDimmed ? 0.5 : 1,
                  transition: "opacity 0.2s",
                }}
              >
                {/* Header */}
                <div className="flex items-center justify-between">
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div className="flex items-center gap-8" style={{ marginBottom: 4 }}>
                      <span
                        style={{
                          fontWeight: 600,
                          fontSize: 14,
                          color: "var(--text-primary)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {repo.full_name}
                      </span>
                    </div>
                    <div className="flex items-center gap-8">
                      <span
                        style={{
                          fontSize: 11,
                          padding: "2px 8px",
                          borderRadius: 10,
                          background:
                            repo.id === 0
                              ? "rgba(31, 111, 235, 0.2)"
                              : repo.private
                                ? "rgba(210, 153, 34, 0.2)"
                                : "rgba(63, 185, 80, 0.2)",
                          color:
                            repo.id === 0
                              ? "var(--accent-blue)"
                              : repo.private
                                ? "var(--accent-yellow)"
                                : "var(--accent-green)",
                        }}
                      >
                        {repo.id === 0 ? "Local" : repo.private ? "Private" : "Public"}
                      </span>
                      {repo.is_org && (
                        <span
                          style={{
                            fontSize: 11,
                            padding: "2px 8px",
                            borderRadius: 10,
                            background: "rgba(31, 111, 235, 0.2)",
                            color: "var(--accent-blue)",
                          }}
                        >
                          Org
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                {/* Discovery badges */}
                {showEnriched && discovered && (
                  <div className="flex items-center gap-8" style={{ flexWrap: "wrap" }}>
                    {discovered.matched_labels.map((label) => (
                      <span
                        key={label}
                        style={{
                          fontSize: 10,
                          padding: "2px 7px",
                          borderRadius: 10,
                          background: "rgba(163, 113, 247, 0.2)",
                          color: "#a371f7",
                        }}
                      >
                        {label}
                      </span>
                    ))}
                    <span
                      style={{
                        fontSize: 10,
                        padding: "2px 7px",
                        borderRadius: 10,
                        background:
                          discovered.source === "both"
                            ? "rgba(31, 111, 235, 0.2)"
                            : discovered.source === "local"
                              ? "rgba(210, 153, 34, 0.2)"
                              : "rgba(63, 185, 80, 0.2)",
                        color:
                          discovered.source === "both"
                            ? "var(--accent-blue)"
                            : discovered.source === "local"
                              ? "var(--accent-yellow)"
                              : "var(--accent-green)",
                      }}
                    >
                      {discovered.source}
                    </span>
                  </div>
                )}

                {/* Workflow files */}
                {showEnriched && discovered && discovered.workflow_files.length > 0 && (
                  <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                    Workflows: {discovered.workflow_files.join(", ")}
                  </div>
                )}

                {/* Runner count */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-8">
                    <span
                      style={{
                        fontSize: 22,
                        fontWeight: 600,
                        color: count > 0 ? "var(--accent-green)" : "var(--text-secondary)",
                      }}
                    >
                      {count}
                    </span>
                    <span className="text-muted" style={{ fontSize: 13 }}>
                      {count === 1 ? "runner" : "runners"}
                    </span>
                  </div>

                  <button
                    className="btn btn-primary"
                    style={{ fontSize: 12, padding: "4px 12px" }}
                    disabled={auth.authenticated && !canCreateRunner}
                    title={
                      auth.authenticated && !canCreateRunner
                        ? "Add a GitHub remote in owner/repository form before creating a runner"
                        : undefined
                    }
                    onClick={() => {
                      if (auth.authenticated && canCreateRunner) {
                        setWizardRepo(repo);
                        setWizardOpen(true);
                      } else if (!auth.authenticated) {
                        navigate("/settings");
                      }
                    }}
                  >
                    {auth.authenticated
                      ? canCreateRunner
                        ? "+ Add Runner"
                        : "GitHub remote required"
                      : "Sign in to add"}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {wizardOpen && (
        <NewRunnerWizard
          onClose={() => {
            setWizardOpen(false);
            setWizardRepo(null);
          }}
          onCreate={createRunner}
          onCreateBatch={createBatch}
          preselectedRepo={wizardRepo?.full_name}
          preselectedRepoDetails={wizardRepo ?? undefined}
        />
      )}
    </div>
  );
}

function FilterPill({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      style={{
        fontSize: 12,
        padding: "3px 10px",
        borderRadius: 12,
        background: active ? "rgba(31, 111, 235, 0.25)" : "var(--bg-tertiary)",
        color: active ? "var(--accent-blue)" : "var(--text-secondary)",
        cursor: "pointer",
        border: `1px solid ${active ? "rgba(31, 111, 235, 0.4)" : "var(--border)"}`,
        fontFamily: "inherit",
      }}
    >
      {label}
    </button>
  );
}
