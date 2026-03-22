import { useState, useMemo } from "react";
import { useRepos } from "../hooks/useRepos";
import type {
  BatchCreateResponse,
  CreateBatchRequest,
  CreateRunnerRequest,
  RepoInfo,
  RunnerInfo,
} from "../api/types";

interface NewRunnerWizardProps {
  onClose: () => void;
  onCreate: (req: CreateRunnerRequest) => Promise<RunnerInfo>;
  onCreateBatch: (req: CreateBatchRequest) => Promise<BatchCreateResponse>;
  preselectedRepo?: string;
}

const DEFAULT_LABELS = ["self-hosted", "macOS", "ARM64"];
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
}: NewRunnerWizardProps) {
  const { repos, loading: reposLoading } = useRepos();
  const [step, setStep] = useState<0 | 1 | 2>(preselectedRepo ? 1 : 0);
  const [search, setSearch] = useState("");
  const [selectedRepo, setSelectedRepo] = useState<RepoInfo | null>(() => {
    return null; // will be resolved once repos load if preselectedRepo is set
  });
  const [resolvedPreselect, setResolvedPreselect] = useState(false);

  // Resolve preselected repo once repos load
  useMemo(() => {
    if (preselectedRepo && !resolvedPreselect && repos.length > 0) {
      const found = repos.find((r) => r.full_name === preselectedRepo) ?? null;
      setSelectedRepo(found);
      setResolvedPreselect(true);
    }
  }, [preselectedRepo, repos, resolvedPreselect]);

  const [name, setName] = useState("");
  const [labelsInput, setLabelsInput] = useState(DEFAULT_LABELS.join(", "));
  const [mode, setMode] = useState<"app" | "service">("app");
  const [count, setCount] = useState(1);
  const [launching, setLaunching] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launched, setLaunched] = useState(false);
  const [batchResults, setBatchResults] = useState<BatchResult[]>([]);

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

    if (count === 1) {
      try {
        await onCreate({
          repo_full_name: selectedRepo.full_name,
          name: name.trim() || undefined,
          labels,
          mode,
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

  const isNextDisabled = count === 1 ? !name.trim() : false;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="wizard" onClick={(e) => e.stopPropagation()}>
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
          {step === 1 && selectedRepo && (
            <StepConfigure
              repo={selectedRepo}
              name={name}
              onName={setName}
              labelsInput={labelsInput}
              onLabelsInput={setLabelsInput}
              mode={mode}
              onMode={setMode}
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
              <button className="btn btn-primary" disabled={isNextDisabled} onClick={handleNext}>
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
  mode: "app" | "service";
  onMode: (v: "app" | "service") => void;
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
  count,
  onCount,
}: StepConfigureProps) {
  return (
    <div>
      <div className="form-group">
        <label className="form-label">Repository</label>
        <div
          style={{
            padding: "8px 12px",
            background: "var(--bg-tertiary)",
            border: "1px solid var(--border)",
            borderRadius: 6,
            fontSize: 13,
            color: "var(--text-secondary)",
          }}
        >
          {repo.full_name}
        </div>
      </div>

      <div className="form-group">
        <label className="form-label" htmlFor="runner-name">
          Runner Name
        </label>
        <input
          id="runner-name"
          type="text"
          value={count > 1 ? "" : name}
          onChange={(e) => onName(e.target.value)}
          style={{ width: "100%" }}
          placeholder={count > 1 ? "Names auto-generated by server" : "e.g. my-repo-runner-1234"}
          disabled={count > 1}
        />
        {count > 1 ? (
          <p className="form-hint">Names auto-generated by server.</p>
        ) : (
          <p className="form-hint">Unique name for this runner instance (auto-generated).</p>
        )}
      </div>

      <div className="form-group">
        <label className="form-label">Number of Runners</label>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button
            className="btn"
            onClick={() => onCount(Math.max(1, count - 1))}
            disabled={count <= 1}
            style={{
              width: 32,
              height: 32,
              padding: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 16,
              lineHeight: 1,
            }}
          >
            −
          </button>
          <span
            style={{
              minWidth: 32,
              textAlign: "center",
              fontSize: 15,
              fontWeight: 600,
              color: "var(--text-primary)",
            }}
          >
            {count}
          </span>
          <button
            className="btn"
            onClick={() => onCount(Math.min(10, count + 1))}
            disabled={count >= 10}
            style={{
              width: 32,
              height: 32,
              padding: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 16,
              lineHeight: 1,
            }}
          >
            +
          </button>
        </div>
        <p className="form-hint">Number of runner instances to create (1–10).</p>
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
          placeholder="self-hosted, macOS, ARM64"
        />
        <p className="form-hint">Comma-separated labels for job routing.</p>
      </div>

      <div className="form-group">
        <label className="form-label">Mode</label>
        <div className="mode-options">
          <button
            className={`mode-option ${mode === "app" ? "mode-option-selected" : ""}`}
            onClick={() => onMode("app")}
          >
            <div className="mode-option-title">App</div>
            <div className="mode-option-desc">
              Runs as a foreground process. Stops when the app quits.
            </div>
          </button>
          <button
            className={`mode-option ${mode === "service" ? "mode-option-selected" : ""}`}
            onClick={() => onMode("service")}
          >
            <div className="mode-option-title">Service</div>
            <div className="mode-option-desc">Runs as a launchd service. Survives reboots.</div>
          </button>
        </div>
      </div>
    </div>
  );
}

interface StepLaunchProps {
  repo: RepoInfo;
  name: string;
  labels: string[];
  mode: string;
  count: number;
  error: string | null;
}

function StepLaunch({ repo, name, labels, mode, count, error }: StepLaunchProps) {
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
