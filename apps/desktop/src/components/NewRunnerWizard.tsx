import { useState, useMemo, useEffect } from "react";
import { useRepos } from "../hooks/useRepos";
import { api } from "../api/commands";
import { openExternal } from "../utils/openExternal";
import type {
  BatchCreateResponse,
  ContainerConfig,
  CreateBatchRequest,
  CreateRunnerRequest,
  RepoInfo,
  RunnerInfo,
} from "../api/types";

const DEFAULT_CONTAINER_IMAGE = "ghcr.io/agallea/homerun-runner:ubuntu-24.04";
const RUST_CONTAINER_IMAGE = "ghcr.io/agallea/homerun-runner:rust";
type RunnerModeChoice = "app" | "service" | "container";
type ContainerPreset = "base" | "rust" | "custom";

interface NewRunnerWizardProps {
  onClose: () => void;
  onCreate: (req: CreateRunnerRequest) => Promise<RunnerInfo>;
  onCreateBatch: (req: CreateBatchRequest) => Promise<BatchCreateResponse>;
  preselectedRepo?: string;
  preselectedRepoDetails?: RepoInfo;
}

// Empty default — the daemon sets platform-appropriate labels (e.g. self-hosted, Windows, X64)
const DEFAULT_LABELS: string[] = [];
const STEPS = ["Select Repository", "Configure", "Launch"];

function generateName(repoName: string): string {
  const slug = repoName.toLowerCase().replace(/[^a-z0-9]/g, "-");
  const rand = Math.floor(Math.random() * 9000) + 1000;
  return `${slug}-runner-${rand}`;
}

interface BatchResult {
  name: string;
  success: boolean;
  error?: string;
}

export function NewRunnerWizard({
  onClose,
  onCreate,
  onCreateBatch,
  preselectedRepo,
  preselectedRepoDetails,
}: NewRunnerWizardProps) {
  const { repos, loading: reposLoading } = useRepos();
  const [step, setStep] = useState<0 | 1 | 2>(preselectedRepo ? 1 : 0);
  const [search, setSearch] = useState("");
  const [selectedRepo, setSelectedRepo] = useState<RepoInfo | null>(() => {
    return null; // will be resolved once repos load if preselectedRepo is set
  });
  const [resolvedPreselectFor, setResolvedPreselectFor] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [labelsInput, setLabelsInput] = useState(DEFAULT_LABELS.join(", "));

  // Resolve a repository passed from the repositories page after loading completes.
  // This must be an effect (not useMemo): setting state while rendering can cause
  // re-entrant renders and an intermittently disappearing wizard.
  useEffect(() => {
    if (!preselectedRepo) {
      if (resolvedPreselectFor !== null) {
        setSelectedRepo(null);
        setName("");
        setStep(0);
      }
      setResolvedPreselectFor(null);
      return;
    }
    if (reposLoading) return;

    const provided =
      preselectedRepoDetails?.full_name === preselectedRepo ? preselectedRepoDetails : null;
    const found = repos.find((r) => r.full_name === preselectedRepo) ?? provided;
    if (resolvedPreselectFor === preselectedRepo) {
      if (selectedRepo?.full_name === preselectedRepo) {
        if (found) return;
        // The repository disappeared after it had been selected.
        setSelectedRepo(null);
        setName("");
        setStep(0);
        return;
      }
      // Do not override a repository the user selected manually after a stale
      // preselection, but retry an unresolved preselection when polling finds it.
      if (selectedRepo || !found) return;
    }

    setSelectedRepo(found);
    setResolvedPreselectFor(preselectedRepo);

    if (found) {
      setName(generateName(found.name));
      setStep(1);
    } else {
      // A stale/deleted repository must not leave the wizard stuck on an
      // unresolvable "Loading repository..." screen.
      setName("");
      setStep(0);
    }
  }, [
    preselectedRepo,
    preselectedRepoDetails,
    repos,
    reposLoading,
    resolvedPreselectFor,
    selectedRepo,
  ]);

  const [mode, setMode] = useState<RunnerModeChoice>("app");
  const [containerImage, setContainerImage] = useState(DEFAULT_CONTAINER_IMAGE);
  const [preset, setPreset] = useState<ContainerPreset>("base");
  const [dockerAvailable, setDockerAvailable] = useState<boolean | null>(null);
  const [count, setCount] = useState(1);
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launched, setLaunched] = useState(false);
  const [batchResults, setBatchResults] = useState<BatchResult[]>([]);

  // Preflight — only offer "Container" mode if Docker is actually reachable.
  useEffect(() => {
    let cancelled = false;
    api
      .getDockerStatus()
      .then((res) => {
        if (!cancelled) setDockerAvailable(res.available);
      })
      .catch(() => {
        if (!cancelled) setDockerAvailable(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const applyPreset = (p: ContainerPreset) => {
    setPreset(p);
    if (p === "base") {
      setContainerImage(DEFAULT_CONTAINER_IMAGE);
      setLabelsInput("self-hosted, docker");
    } else if (p === "rust") {
      setContainerImage(RUST_CONTAINER_IMAGE);
      setLabelsInput("self-hosted, docker, rust");
    }
    // "custom": leave the current image/labels for the user to edit.
  };

  useEffect(() => {
    if (mode === "container") applyPreset(preset);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  const filteredRepos = useMemo(() => {
    const q = search.toLowerCase();
    return repos.filter((r) => r.full_name.toLowerCase().includes(q));
  }, [repos, search]);

  function handleSelectRepo(repo: RepoInfo) {
    setSelectedRepo(repo);
    setName(generateName(repo.name));
    setStep(1);
  }

  function handleBack() {
    if (step === 1) setStep(0);
    else if (step === 2) setStep(1);
  }

  function handleNext() {
    if (step === 1) setStep(2);
  }

  async function handleLaunch() {
    if (!selectedRepo) return;
    setLaunching(true);
    setLaunchError(null);
    const labels = labelsInput
      .split(",")
      .map((l) => l.trim())
      .filter(Boolean);
    const container: ContainerConfig | undefined =
      mode === "container" ? { image: containerImage.trim() } : undefined;

    if (count === 1) {
      try {
        await onCreate({
          repo_full_name: selectedRepo.full_name,
          name: name.trim() || undefined,
          labels,
          mode,
          container,
        });
        setLaunched(true);
      } catch (e) {
        setLaunchError(String(e));
      } finally {
        setLaunching(false);
      }
    } else {
      // Batch creation via server endpoint
      try {
        const result = await onCreateBatch({
          repo_full_name: selectedRepo.full_name,
          count,
          labels,
          mode,
          container,
        });
        const results: BatchResult[] = result.runners.map((r) => ({
          name: r.config.name,
          success: true,
        }));
        for (const err of result.errors) {
          results.push({
            name: `runner-${err.index + 1}`,
            success: false,
            error: err.error,
          });
        }
        setBatchResults(results);
        setLaunching(false);
        setLaunched(true);
      } catch (e) {
        setLaunchError(String(e));
        setLaunching(false);
      }
    }
  }

  const labels = labelsInput
    .split(",")
    .map((l) => l.trim())
    .filter(Boolean);

  const isNextDisabled =
    (count === 1 ? !name.trim() : false) || (mode === "container" && !containerImage.trim());

  return (
    <div className="dialog-overlay" role="presentation">
      <div className="wizard" role="dialog" aria-modal="true" aria-label="Create runner">
        {/* Step indicators */}
        <div className="wizard-progress">
          {STEPS.map((label, i) => (
            <div
              key={i}
              className={`wizard-step ${
                i === step ? "wizard-step-active" : i < step ? "wizard-step-done" : ""
              }`}
            >
              <span className="wizard-step-num">{i < step ? "✓" : i + 1}</span>
              <span className="wizard-step-label">{label}</span>
            </div>
          ))}
        </div>

        {/* Body */}
        <div className="wizard-body">
          {step === 0 && (
            <StepSelectRepo
              repos={filteredRepos}
              loading={reposLoading}
              search={search}
              onSearch={setSearch}
              selected={selectedRepo}
              onSelect={handleSelectRepo}
            />
          )}
          {step === 1 && !selectedRepo && (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 10,
                padding: "48px 0",
                color: "var(--text-secondary)",
                fontSize: 13,
              }}
            >
              <span
                style={{
                  display: "inline-block",
                  width: 16,
                  height: 16,
                  border: "2px solid var(--border)",
                  borderTopColor: "var(--text-primary)",
                  borderRadius: "50%",
                  animation: "spin 0.6s linear infinite",
                }}
              />
              Loading repository...
            </div>
          )}
          {step === 1 && selectedRepo && (
            <StepConfigure
              repo={selectedRepo}
              name={name}
              onName={setName}
              labelsInput={labelsInput}
              onLabelsInput={setLabelsInput}
              mode={mode}
              onMode={setMode}
              dockerAvailable={dockerAvailable}
              containerImage={containerImage}
              onContainerImage={setContainerImage}
              preset={preset}
              onPreset={applyPreset}
              count={count}
              onCount={setCount}
            />
          )}
          {step === 2 && selectedRepo && !launched && (
            <StepLaunch
              repo={selectedRepo}
              name={name}
              labels={labels}
              mode={mode}
              containerImage={containerImage}
              count={count}
              error={launchError}
            />
          )}
          {step === 2 && launched && count === 1 && (
            <div style={{ textAlign: "center", padding: "24px 0" }}>
              <div
                style={{
                  fontSize: 48,
                  marginBottom: 12,
                  color: "var(--accent-green)",
                }}
              >
                ✓
              </div>
              <h3 style={{ margin: "0 0 8px", color: "var(--text-primary)" }}>Runner launched!</h3>
              <p className="text-muted">
                <strong className="text-primary">{name}</strong> is being created for{" "}
                <strong className="text-primary">{selectedRepo?.full_name}</strong>.
              </p>
            </div>
          )}
          {step === 2 && launched && count > 1 && (
            <BatchSummary results={batchResults} repo={selectedRepo!} />
          )}
        </div>

        {/* Footer */}
        {!launched && (
          <div className="wizard-footer">
            {step === 0 ? (
              <button className="btn" onClick={onClose}>
                Cancel
              </button>
            ) : (
              <button className="btn" onClick={handleBack} disabled={launching}>
                Back
              </button>
            )}
            {step === 0 && (
              <button
                className="btn btn-primary"
                disabled={!selectedRepo}
                onClick={() => setStep(1)}
              >
                Next
              </button>
            )}
            {step === 1 && (
              <button
                className="btn btn-primary"
                disabled={isNextDisabled || !selectedRepo}
                onClick={handleNext}
              >
                Next
              </button>
            )}
            {step === 2 && (
              <button className="btn btn-primary" disabled={launching} onClick={handleLaunch}>
                {launching
                  ? "Launching..."
                  : count > 1
                    ? `Launch ${count} Runners`
                    : "Launch Runner"}
              </button>
            )}
          </div>
        )}
        {launched && (
          <div className="wizard-footer">
            <button className="btn btn-primary" onClick={onClose}>
              Done
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// --- Step sub-components ---

interface StepSelectRepoProps {
  repos: RepoInfo[];
  loading: boolean;
  search: string;
  onSearch: (v: string) => void;
  selected: RepoInfo | null;
  onSelect: (r: RepoInfo) => void;
}

function StepSelectRepo({
  repos,
  loading,
  search,
  onSearch,
  selected,
  onSelect,
}: StepSelectRepoProps) {
  return (
    <div>
      <div className="form-group">
        <input
          type="text"
          placeholder="Search repositories..."
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          style={{ width: "100%" }}
          autoFocus
        />
      </div>
      {loading ? (
        <p className="text-muted">Loading repositories...</p>
      ) : repos.length === 0 ? (
        <p className="text-muted" style={{ padding: "16px 0" }}>
          No repositories found.
        </p>
      ) : (
        <div className="repo-list">
          {repos.map((repo) => (
            <button
              key={repo.id}
              className={`repo-item ${selected?.id === repo.id ? "repo-item-selected" : ""}`}
              onClick={() => onSelect(repo)}
            >
              <span>{repo.full_name}</span>
              <span
                style={{
                  fontSize: 11,
                  padding: "2px 6px",
                  borderRadius: 10,
                  background: repo.private ? "rgba(210, 153, 34, 0.2)" : "rgba(63, 185, 80, 0.2)",
                  color: repo.private ? "var(--accent-yellow)" : "var(--accent-green)",
                }}
              >
                {repo.private ? "Private" : "Public"}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

interface StepConfigureProps {
  repo: RepoInfo;
  name: string;
  onName: (v: string) => void;
  labelsInput: string;
  onLabelsInput: (v: string) => void;
  mode: RunnerModeChoice;
  onMode: (v: RunnerModeChoice) => void;
  dockerAvailable: boolean | null;
  containerImage: string;
  onContainerImage: (v: string) => void;
  preset: ContainerPreset;
  onPreset: (p: ContainerPreset) => void;
  count: number;
  onCount: (v: number) => void;
}

function StepConfigure({
  repo,
  name,
  onName,
  labelsInput,
  onLabelsInput,
  mode,
  onMode,
  dockerAvailable,
  containerImage,
  onContainerImage,
  preset,
  onPreset,
  count,
  onCount,
}: StepConfigureProps) {
  return (
    <div>
      <div className="form-group">
        <label className="form-label">Repository</label>
        <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>{repo.full_name}</div>
      </div>

      <div className="form-group">
        <label className="form-label">Runners</label>
        <div style={{ display: "flex", gap: 4 }}>
          {Array.from({ length: 10 }, (_, i) => i + 1).map((n) => (
            <button
              key={n}
              onClick={() => onCount(n)}
              style={{
                width: 32,
                height: 32,
                padding: 0,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 13,
                fontWeight: n === count ? 600 : 400,
                borderRadius: 6,
                border: `1px solid ${n === count ? "var(--accent-blue)" : "var(--border)"}`,
                background: n === count ? "rgba(59, 130, 246, 0.15)" : "transparent",
                color: n === count ? "var(--accent-blue)" : "var(--text-secondary)",
                cursor: "pointer",
              }}
            >
              {n}
            </button>
          ))}
        </div>
        <p className="form-hint">
          {count === 1
            ? "Single runner instance."
            : `${count} parallel runners for concurrent jobs.`}
        </p>
      </div>

      <div className="form-group">
        <label className="form-label">Mode</label>
        <div style={{ display: "flex", gap: 10 }}>
          {(["app", "service", "container"] as const).map((m) => {
            const selected = mode === m;
            const disabled = m === "container" && dockerAvailable === false;
            const iconColor = disabled
              ? "var(--text-secondary)"
              : selected
                ? "var(--accent-blue)"
                : "var(--text-secondary)";
            return (
              <button
                key={m}
                onClick={() => !disabled && onMode(m)}
                disabled={disabled}
                title={
                  disabled
                    ? "Docker isn't reachable — start Docker Desktop to use this mode"
                    : undefined
                }
                style={{
                  flex: 1,
                  padding: "12px",
                  background: selected ? "rgba(59, 130, 246, 0.08)" : "var(--bg-tertiary)",
                  border: `1.5px solid ${selected ? "var(--accent-blue)" : "var(--border)"}`,
                  borderRadius: 10,
                  cursor: disabled ? "not-allowed" : "pointer",
                  textAlign: "left",
                  opacity: disabled ? 0.5 : 1,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
                  <div
                    style={{
                      width: 28,
                      height: 28,
                      borderRadius: 7,
                      background: selected ? "rgba(59, 130, 246, 0.2)" : "rgba(255,255,255,0.06)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      flexShrink: 0,
                    }}
                  >
                    {m === "app" ? (
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke={iconColor}
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <rect x="2" y="3" width="20" height="14" rx="2" />
                        <line x1="8" y1="21" x2="16" y2="21" />
                        <line x1="12" y1="17" x2="12" y2="21" />
                      </svg>
                    ) : m === "service" ? (
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke={iconColor}
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <rect x="2" y="2" width="20" height="8" rx="2" />
                        <rect x="2" y="14" width="20" height="8" rx="2" />
                        <line x1="6" y1="6" x2="6.01" y2="6" />
                        <line x1="6" y1="18" x2="6.01" y2="18" />
                      </svg>
                    ) : (
                      <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke={iconColor}
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <ellipse cx="12" cy="6" rx="9" ry="3" />
                        <path d="M3 6v12c0 1.66 4.03 3 9 3s9-1.34 9-3V6" />
                        <path d="M3 12c0 1.66 4.03 3 9 3s9-1.34 9-3" />
                      </svg>
                    )}
                  </div>
                  <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                    {m === "app" ? "App" : m === "service" ? "Service" : "Container"}
                  </span>
                </div>
                <div style={{ fontSize: 11, color: "var(--text-secondary)", lineHeight: 1.4 }}>
                  {m === "app"
                    ? "Runs as a foreground process. Stops with daemon."
                    : m === "service"
                      ? "Runs persistently and can be reattached after daemon crashes."
                      : disabled
                        ? "Docker isn't reachable right now."
                        : "Runs inside a Docker container for isolation."}
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {mode === "container" && (
        <div className="form-group">
          <label className="form-label">Image preset</label>
          <div style={{ display: "flex", gap: 8 }}>
            {(["base", "rust", "custom"] as const).map((p) => (
              <button
                key={p}
                type="button"
                onClick={() => onPreset(p)}
                style={{
                  flex: 1,
                  padding: "8px 10px",
                  fontSize: 12,
                  fontWeight: 600,
                  textTransform: "capitalize",
                  background: preset === p ? "rgba(59, 130, 246, 0.08)" : "var(--bg-tertiary)",
                  border: `1.5px solid ${preset === p ? "var(--accent-blue)" : "var(--border)"}`,
                  borderRadius: 8,
                  color: "var(--text-primary)",
                  cursor: "pointer",
                }}
              >
                {p.charAt(0).toUpperCase() + p.slice(1)}
              </button>
            ))}
          </div>
        </div>
      )}

      {mode === "container" && (
        <div className="form-group">
          <label className="form-label" htmlFor="runner-image">
            Image
          </label>
          <input
            id="runner-image"
            type="text"
            value={containerImage}
            onChange={(e) => onContainerImage(e.target.value)}
            style={{ width: "100%" }}
            placeholder={DEFAULT_CONTAINER_IMAGE}
            className="font-mono"
          />
          <p className="form-hint">
            Defaults to HomeRun's base image. Any image with glibc and bash works — see{" "}
            <a
              href="https://github.com/lgg/homerun/blob/master/docs/DOCKER_RUNNERS.md"
              target="_blank"
              rel="noreferrer"
              onClick={(e) => {
                e.preventDefault();
                void openExternal(
                  "https://github.com/lgg/homerun/blob/master/docs/DOCKER_RUNNERS.md",
                );
              }}
            >
              Docker Runners
            </a>
            .
          </p>
        </div>
      )}

      <div className="form-group" style={{ marginTop: 8 }}>
        <label className="form-label" htmlFor="runner-name">
          Name
        </label>
        <input
          id="runner-name"
          type="text"
          value={count > 1 ? "" : name}
          onChange={(e) => onName(e.target.value)}
          style={{ width: "100%" }}
          placeholder={count > 1 ? "Auto-generated" : "e.g. my-runner-1"}
          disabled={count > 1}
        />
      </div>

      <div className="form-group">
        <label className="form-label" htmlFor="runner-labels">
          Labels
        </label>
        <input
          id="runner-labels"
          type="text"
          value={labelsInput}
          onChange={(e) => onLabelsInput(e.target.value)}
          style={{ width: "100%" }}
          placeholder="e.g. self-hosted, gpu (leave empty for defaults)"
        />
      </div>
    </div>
  );
}

interface StepLaunchProps {
  repo: RepoInfo;
  name: string;
  labels: string[];
  mode: string;
  containerImage: string;
  count: number;
  error: string | null;
}

function StepLaunch({ repo, name, labels, mode, containerImage, count, error }: StepLaunchProps) {
  const slug = repo.name.toLowerCase().replace(/[^a-z0-9]/g, "-");

  return (
    <div>
      <p className="text-muted" style={{ marginBottom: 16 }}>
        Review the configuration before launching.
      </p>

      {error && <div className="error-banner">{error}</div>}

      <div className="launch-summary">
        <div className="launch-summary-row">
          <span className="launch-summary-key">Repository</span>
          <span className="launch-summary-value">{repo.full_name}</span>
        </div>
        <div className="launch-summary-row">
          <span className="launch-summary-key">Name</span>
          <span className="launch-summary-value font-mono">
            {count > 1 ? `${slug}-runner-1 ... ${slug}-runner-${count}` : name}
          </span>
        </div>
        <div className="launch-summary-row">
          <span className="launch-summary-key">Count</span>
          <span className="launch-summary-value">{count}</span>
        </div>
        <div className="launch-summary-row">
          <span className="launch-summary-key">Labels</span>
          <span className="launch-summary-value">{labels.join(", ")}</span>
        </div>
        <div className="launch-summary-row">
          <span className="launch-summary-key">Mode</span>
          <span className="launch-summary-value" style={{ textTransform: "capitalize" }}>
            {mode}
          </span>
        </div>
        {mode === "container" && (
          <div className="launch-summary-row">
            <span className="launch-summary-key">Image</span>
            <span className="launch-summary-value font-mono">{containerImage}</span>
          </div>
        )}
      </div>
    </div>
  );
}

interface BatchSummaryProps {
  results: BatchResult[];
  repo: RepoInfo;
}

function BatchSummary({ results, repo }: BatchSummaryProps) {
  const successCount = results.filter((r) => r.success).length;
  const failCount = results.length - successCount;
  const allSuccess = failCount === 0;

  return (
    <div style={{ padding: "8px 0" }}>
      <div style={{ textAlign: "center", marginBottom: 16 }}>
        <div
          style={{
            fontSize: 48,
            marginBottom: 12,
            color: allSuccess ? "var(--accent-green)" : "var(--accent-yellow)",
          }}
        >
          {allSuccess ? "✓" : "!"}
        </div>
        <h3 style={{ margin: "0 0 8px", color: "var(--text-primary)" }}>
          {allSuccess
            ? `${successCount} runner${successCount !== 1 ? "s" : ""} created successfully`
            : `${successCount} of ${results.length} runners created`}
        </h3>
        <p className="text-muted">
          For <strong className="text-primary">{repo.full_name}</strong>
        </p>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {results.map((r) => (
          <div
            key={r.name}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              padding: "6px 10px",
              borderRadius: 6,
              background: r.success ? "rgba(63, 185, 80, 0.08)" : "rgba(248, 81, 73, 0.08)",
              border: `1px solid ${r.success ? "rgba(63, 185, 80, 0.2)" : "rgba(248, 81, 73, 0.2)"}`,
              fontSize: 13,
            }}
          >
            <span style={{ color: r.success ? "var(--accent-green)" : "var(--accent-red)" }}>
              {r.success ? "✓" : "✗"}
            </span>
            <span className="font-mono" style={{ color: "var(--text-primary)", flex: 1 }}>
              {r.name}
            </span>
            {r.error && <span style={{ color: "var(--text-muted)", fontSize: 11 }}>{r.error}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}
