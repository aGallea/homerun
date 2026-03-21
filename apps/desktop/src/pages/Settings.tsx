import { useState } from "react";
import { useAuth } from "../hooks/useAuth";

export function Settings() {
  const { auth, loading, loginWithToken, logout } = useAuth();
  const [token, setToken] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [tokenSuccess, setTokenSuccess] = useState(false);

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault();
    if (!token.trim()) return;
    setSubmitting(true);
    setTokenError(null);
    setTokenSuccess(false);
    try {
      await loginWithToken(token.trim());
      setToken("");
      setTokenSuccess(true);
    } catch (e) {
      setTokenError(String(e));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleLogout() {
    await logout();
    setTokenSuccess(false);
  }

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Settings</h1>
      </div>

      {/* Auth section */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Authentication" />

        <div className="card">
          {loading ? (
            <p className="text-muted">Loading...</p>
          ) : auth.authenticated && auth.user ? (
            /* Logged in state */
            <div>
              <div className="flex items-center gap-12" style={{ marginBottom: 16 }}>
                <img
                  src={auth.user.avatar_url}
                  alt={auth.user.login}
                  style={{
                    width: 40,
                    height: 40,
                    borderRadius: "50%",
                    border: "2px solid var(--border)",
                  }}
                />
                <div>
                  <div style={{ fontWeight: 600, marginBottom: 2 }}>
                    @{auth.user.login}
                  </div>
                  <div
                    style={{
                      fontSize: 12,
                      padding: "2px 8px",
                      display: "inline-block",
                      borderRadius: 10,
                      background: "rgba(63, 185, 80, 0.2)",
                      color: "var(--accent-green)",
                    }}
                  >
                    Authenticated
                  </div>
                </div>
              </div>
              <button className="btn btn-danger" onClick={handleLogout}>
                Logout
              </button>
            </div>
          ) : (
            /* Login form */
            <div>
              <p className="text-muted" style={{ marginBottom: 16 }}>
                Enter a GitHub Personal Access Token (PAT) with{" "}
                <code
                  style={{
                    background: "var(--bg-tertiary)",
                    padding: "1px 6px",
                    borderRadius: 4,
                    fontSize: 12,
                  }}
                >
                  repo
                </code>{" "}
                and{" "}
                <code
                  style={{
                    background: "var(--bg-tertiary)",
                    padding: "1px 6px",
                    borderRadius: 4,
                    fontSize: 12,
                  }}
                >
                  admin:org
                </code>{" "}
                scopes to authenticate.
              </p>

              {tokenError && (
                <div className="error-banner" style={{ marginBottom: 16 }}>
                  {tokenError}
                </div>
              )}

              {tokenSuccess && (
                <div
                  style={{
                    background: "rgba(63, 185, 80, 0.15)",
                    border: "1px solid var(--accent-green)",
                    borderRadius: 6,
                    padding: "10px 14px",
                    marginBottom: 16,
                    fontSize: 13,
                    color: "var(--accent-green)",
                  }}
                >
                  Authenticated successfully!
                </div>
              )}

              <form onSubmit={handleLogin}>
                <div className="form-group" style={{ marginBottom: 12 }}>
                  <label className="form-label" htmlFor="pat-input">
                    Personal Access Token
                  </label>
                  <input
                    id="pat-input"
                    type="password"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    placeholder="ghp_..."
                    style={{ width: "100%", maxWidth: 400 }}
                    autoComplete="off"
                  />
                  <p className="form-hint">
                    Your token is stored securely in the daemon and never
                    transmitted in plaintext.
                  </p>
                </div>
                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={submitting || !token.trim()}
                >
                  {submitting ? "Authenticating..." : "Save Token"}
                </button>
              </form>
            </div>
          )}
        </div>
      </section>

      {/* Auto-start (placeholder) */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Startup" />
        <div className="card">
          <PlaceholderSetting
            label="Launch at login"
            description="Automatically start the HomeRun daemon when you log in to macOS."
          />
          <Divider />
          <PlaceholderSetting
            label="Start runners on launch"
            description="Resume all runners that were running when the app was last closed."
          />
        </div>
      </section>

      {/* Notifications (placeholder) */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Notifications" />
        <div className="card">
          <PlaceholderSetting
            label="Runner status changes"
            description="Notify when a runner goes online, offline, or encounters an error."
          />
          <Divider />
          <PlaceholderSetting
            label="Job completions"
            description="Notify when a job completes or fails on a self-hosted runner."
          />
        </div>
      </section>

      {/* About */}
      <section>
        <SectionHeader title="About" />
        <div className="card">
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 8,
              fontSize: 13,
              color: "var(--text-secondary)",
            }}
          >
            <div className="flex items-center justify-between">
              <span>HomeRun</span>
              <span className="font-mono">desktop</span>
            </div>
            <div className="flex items-center justify-between">
              <span>Daemon connection</span>
              <span style={{ color: "var(--accent-green)" }}>Unix socket</span>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

// Helper components

function SectionHeader({ title }: { title: string }) {
  return (
    <h2
      style={{
        fontSize: 13,
        fontWeight: 500,
        color: "var(--text-secondary)",
        textTransform: "uppercase",
        letterSpacing: "0.5px",
        marginBottom: 12,
      }}
    >
      {title}
    </h2>
  );
}

function PlaceholderSetting({
  label,
  description,
}: {
  label: string;
  description: string;
}) {
  return (
    <div className="flex items-center justify-between" style={{ padding: "8px 0" }}>
      <div style={{ flex: 1, marginRight: 24 }}>
        <div style={{ fontWeight: 500, marginBottom: 2, fontSize: 14 }}>
          {label}
        </div>
        <p className="text-muted" style={{ margin: 0, fontSize: 12 }}>
          {description}
        </p>
      </div>
      <div
        style={{
          width: 36,
          height: 20,
          background: "var(--bg-tertiary)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          opacity: 0.5,
          cursor: "not-allowed",
          flexShrink: 0,
        }}
        title="Coming soon"
      />
    </div>
  );
}

function Divider() {
  return (
    <div
      style={{
        height: 1,
        background: "var(--border)",
        margin: "4px 0",
      }}
    />
  );
}
