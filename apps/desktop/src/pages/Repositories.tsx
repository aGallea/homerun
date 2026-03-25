import { useState, useMemo } from "react";
import { useNavigate, useOutletContext } from "react-router-dom";
import { useRepos } from "../hooks/useRepos";
import type { RunnersContextType } from "../hooks/useRunners";
import { useAuth } from "../hooks/useAuth";
import { NewRunnerWizard } from "../components/NewRunnerWizard";

export function Repositories() {
  const { auth } = useAuth();
  const navigate = useNavigate();
  const { repos, loading: reposLoading, error: reposError } = useRepos();
  const { runners, createRunner, createBatch } = useOutletContext<RunnersContextType>();
  const [search, setSearch] = useState("");
  const [wizardRepo, setWizardRepo] = useState<string | null>(null);

  // Count runners per repo full_name
  const runnerCountByRepo = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of runners) {
      const key = `${r.config.repo_owner}/${r.config.repo_name}`;
      map.set(key, (map.get(key) ?? 0) + 1);
    }
    return map;
  }, [runners]);

  const filteredRepos = useMemo(() => {
    const q = search.toLowerCase();
    return repos.filter((r) => r.full_name.toLowerCase().includes(q));
  }, [repos, search]);

  if (!auth.authenticated) {
    return (
      <div className="page">
        <div className="page-header">
          <h1 className="page-title">Repositories</h1>
        </div>
        <div className="card" style={{ textAlign: "center", padding: "60px 40px" }}>
          <p style={{ fontSize: 15, color: "var(--text-primary)", marginBottom: 8 }}>
            Sign in to view your repositories
          </p>
          <p className="text-muted" style={{ fontSize: 13, marginBottom: 20 }}>
            Connect your GitHub account to browse repositories and add runners.
          </p>
          <button className="btn btn-primary" onClick={() => navigate("/settings")}>
            Sign in with GitHub
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Repositories</h1>
        <button className="btn btn-primary" onClick={() => setWizardRepo("")}>
          + Add Runner
        </button>
      </div>

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

      {reposError && (
        <div className="error-banner" style={{ marginBottom: 16 }}>
          {reposError}
        </div>
      )}

      {reposLoading ? (
        <p className="text-muted">Loading repositories...</p>
      ) : filteredRepos.length === 0 ? (
        <div className="card" style={{ textAlign: "center", padding: "40px" }}>
          <p className="text-muted">
            {search ? "No repositories match your search." : "No repositories found."}
          </p>
          <p className="text-muted" style={{ fontSize: 12, marginTop: 8 }}>
            Make sure you are authenticated with a GitHub token that has repo access.
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
            return (
              <div
                key={repo.id}
                className="card"
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
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
                          background: repo.private
                            ? "rgba(210, 153, 34, 0.2)"
                            : "rgba(63, 185, 80, 0.2)",
                          color: repo.private ? "var(--accent-yellow)" : "var(--accent-green)",
                        }}
                      >
                        {repo.private ? "Private" : "Public"}
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
                    onClick={() => setWizardRepo(repo.full_name)}
                  >
                    + Add Runner
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {wizardRepo !== null && (
        <NewRunnerWizard
          onClose={() => setWizardRepo(null)}
          onCreate={createRunner}
          onCreateBatch={createBatch}
          preselectedRepo={wizardRepo || undefined}
        />
      )}
    </div>
  );
}
