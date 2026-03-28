import { useState, useEffect, useRef, useCallback } from "react";
import { useAuth } from "../hooks/useAuth";
import { api } from "../api/commands";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import type { DeviceFlowResponse, Preferences } from "../api/types";

type DeviceFlowState =
  | { stage: "idle" }
  | { stage: "pending"; flow: DeviceFlowResponse }
  | { stage: "success" }
  | { stage: "error"; message: string };

export function Settings() {
  const { auth, loading, loginWithToken, logout, refresh } = useAuth();

  // Device flow state
  const [deviceFlow, setDeviceFlow] = useState<DeviceFlowState>({ stage: "idle" });
  const [deviceFlowStarting, setDeviceFlowStarting] = useState(false);
  const [copied, setCopied] = useState(false);
  const cancelledRef = useRef(false);

  // Cancel any in-flight device flow poll on unmount
  useEffect(() => {
    cancelledRef.current = false;
    return () => {
      cancelledRef.current = true;
    };
  }, []);

  // App version
  const [appVersion, setAppVersion] = useState<string>("");
  useEffect(() => {
    getVersion().then(setAppVersion);
  }, []);

  // PAT section
  const [patExpanded, setPatExpanded] = useState(false);
  const [token, setToken] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [tokenSuccess, setTokenSuccess] = useState(false);

  // Settings toggles
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [preferences, setPreferences] = useState<Preferences>({
    start_runners_on_launch: false,
    notify_status_changes: true,
    notify_job_completions: true,
    scan_labels: [],
    workspace_path: null,
    auto_scan: false,
  });

  // Check launch-at-login status on mount
  useEffect(() => {
    invoke<boolean>("service_status")
      .then(setLaunchAtLogin)
      .catch(() => {});
    api
      .getPreferences()
      .then(setPreferences)
      .catch(() => {});
  }, []);

  // Labels input local state (synced with preferences)
  const [labelsInput, setLabelsInput] = useState("");

  useEffect(() => {
    setLabelsInput(preferences.scan_labels.join(", "));
  }, [preferences.scan_labels]);

  function updatePreference(key: keyof Preferences, value: boolean) {
    setPreferences((prev) => {
      const updated = { ...prev, [key]: value };
      api
        .updatePreferences(updated)
        .then(setPreferences)
        .catch((e) => {
          console.error("Failed to update preference:", e);
          setPreferences(prev);
        });
      return updated;
    });
  }

  function updatePreferences(updates: Partial<Preferences>) {
    setPreferences((prev) => {
      const updated = { ...prev, ...updates };
      api
        .updatePreferences(updated)
        .then(setPreferences)
        .catch((e) => {
          console.error("Failed to update preference:", e);
          setPreferences(prev);
        });
      return updated;
    });
  }

  const pollInBackground = useCallback(
    (flow: DeviceFlowResponse) => {
      api
        .pollDeviceFlow(flow.device_code, flow.interval)
        .then(async () => {
          if (cancelledRef.current) return;
          setDeviceFlow({ stage: "success" });
          await refresh();
        })
        .catch((e: unknown) => {
          if (cancelledRef.current) return;
          setDeviceFlow({ stage: "error", message: String(e) });
        });
    },
    [refresh],
  );

  async function handleStartDeviceFlow() {
    setDeviceFlowStarting(true);
    setDeviceFlow({ stage: "idle" });
    try {
      const flow = await api.startDeviceFlow();
      if (cancelledRef.current) return;
      setDeviceFlow({ stage: "pending", flow });
      // Open the verification URL in the system browser
      try {
        const { open } = await import("@tauri-apps/plugin-shell");
        await open(flow.verification_uri);
      } catch {
        // Best-effort; user can navigate manually
      }
      // Begin polling in the background
      pollInBackground(flow);
    } catch (e) {
      if (cancelledRef.current) return;
      setDeviceFlow({ stage: "error", message: String(e) });
    } finally {
      if (!cancelledRef.current) {
        setDeviceFlowStarting(false);
      }
    }
  }

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
    setDeviceFlow({ stage: "idle" });
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
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-12">
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
                  <div style={{ fontWeight: 600, marginBottom: 2 }}>@{auth.user.login}</div>
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
            /* Login section */
            <div>
              {/* --- Device flow --- */}
              <div style={{ marginBottom: 24 }}>
                <p className="text-muted" style={{ marginBottom: 16 }}>
                  Authenticate with your GitHub account using the device authorization flow — no
                  personal access token required.
                </p>

                {deviceFlow.stage === "idle" && (
                  <button
                    className="btn btn-primary"
                    onClick={handleStartDeviceFlow}
                    disabled={deviceFlowStarting}
                    style={{ display: "flex", alignItems: "center", gap: 8 }}
                  >
                    <GitHubIcon />
                    {deviceFlowStarting ? "Starting..." : "Login with GitHub"}
                  </button>
                )}

                {deviceFlow.stage === "pending" && (
                  <div
                    style={{
                      background: "var(--bg-tertiary)",
                      border: "1px solid var(--border)",
                      borderRadius: 8,
                      padding: "16px 20px",
                    }}
                  >
                    <div
                      style={{
                        fontSize: 13,
                        color: "var(--text-secondary)",
                        marginBottom: 12,
                      }}
                    >
                      A browser window has opened. Enter the code below on GitHub to authorize
                      HomeRun:
                    </div>
                    <div
                      style={{
                        position: "relative",
                        fontFamily: "var(--font-mono, monospace)",
                        fontSize: 28,
                        fontWeight: 700,
                        letterSpacing: "0.2em",
                        color: "var(--text-primary)",
                        textAlign: "center",
                        padding: "12px 0",
                        marginBottom: 12,
                        background: "var(--bg-secondary)",
                        borderRadius: 6,
                        border: "1px solid var(--border)",
                        cursor: "pointer",
                      }}
                      onClick={() => {
                        navigator.clipboard.writeText(deviceFlow.flow.user_code);
                        setCopied(true);
                        setTimeout(() => setCopied(false), 2000);
                      }}
                      title="Click to copy"
                    >
                      {deviceFlow.flow.user_code}
                      <span
                        style={{
                          position: "absolute",
                          right: 12,
                          top: "50%",
                          transform: "translateY(-50%)",
                          fontSize: 14,
                          opacity: 0.6,
                          color: copied ? "var(--accent-green, #4ade80)" : "var(--text-secondary)",
                        }}
                      >
                        {copied ? (
                          <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          >
                            <polyline points="20 6 9 17 4 12" />
                          </svg>
                        ) : (
                          <svg
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          >
                            <rect x="9" y="9" width="13" height="13" rx="2" />
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                          </svg>
                        )}
                      </span>
                    </div>
                    <div
                      style={{
                        fontSize: 12,
                        color: "var(--text-secondary)",
                        marginBottom: 12,
                        textAlign: "center",
                      }}
                    >
                      Visit{" "}
                      <a
                        href={deviceFlow.flow.verification_uri}
                        style={{ color: "var(--accent-blue, #58a6ff)" }}
                        onClick={(e) => {
                          e.preventDefault();
                          import("@tauri-apps/plugin-shell").then(({ open }) => {
                            if (deviceFlow.stage === "pending") {
                              open(deviceFlow.flow.verification_uri);
                            }
                          });
                        }}
                      >
                        {deviceFlow.flow.verification_uri}
                      </a>{" "}
                      if the browser did not open automatically.
                    </div>
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        fontSize: 13,
                        color: "var(--text-secondary)",
                      }}
                    >
                      <Spinner />
                      Waiting for authorization…
                    </div>
                  </div>
                )}

                {deviceFlow.stage === "success" && (
                  <div
                    style={{
                      background: "rgba(63, 185, 80, 0.15)",
                      border: "1px solid var(--accent-green)",
                      borderRadius: 6,
                      padding: "10px 14px",
                      fontSize: 13,
                      color: "var(--accent-green)",
                    }}
                  >
                    Authenticated successfully!
                  </div>
                )}

                {deviceFlow.stage === "error" && (
                  <div>
                    <div className="error-banner" style={{ marginBottom: 12 }}>
                      {deviceFlow.message}
                    </div>
                    <button
                      className="btn btn-secondary"
                      onClick={() => setDeviceFlow({ stage: "idle" })}
                    >
                      Try again
                    </button>
                  </div>
                )}
              </div>

              {/* --- Advanced: PAT --- */}
              <div
                style={{
                  borderTop: "1px solid var(--border)",
                  paddingTop: 16,
                }}
              >
                <button
                  style={{
                    background: "none",
                    border: "none",
                    padding: 0,
                    cursor: "pointer",
                    fontSize: 12,
                    color: "var(--text-secondary)",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                  }}
                  onClick={() => setPatExpanded((v) => !v)}
                  aria-expanded={patExpanded}
                >
                  <span
                    style={{
                      display: "inline-block",
                      transition: "transform 0.15s",
                      transform: patExpanded ? "rotate(90deg)" : "rotate(0deg)",
                    }}
                  >
                    ▶
                  </span>
                  Advanced: Use Personal Access Token
                </button>

                {patExpanded && (
                  <div style={{ marginTop: 16 }}>
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
                      scopes.
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
                          Your token is stored securely in the macOS keychain.
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
            </div>
          )}
        </div>
      </section>

      {/* Startup */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Startup" />
        <div className="card">
          <ToggleSetting
            label="Launch at login"
            description="Automatically start the HomeRun daemon when you log in to macOS."
            checked={launchAtLogin}
            onChange={async (checked) => {
              try {
                if (checked) {
                  await invoke("install_service");
                } else {
                  await invoke("uninstall_service");
                }
                setLaunchAtLogin(checked);
              } catch (e) {
                console.error("Failed to toggle launch at login:", e);
              }
            }}
          />
          <Divider />
          <ToggleSetting
            label="Restore runners on launch"
            description="Automatically start runners that were running when the daemon was last stopped."
            checked={preferences.start_runners_on_launch}
            onChange={(checked) => updatePreference("start_runners_on_launch", checked)}
          />
        </div>
      </section>

      {/* Notifications */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Notifications" />
        <div className="card">
          <ToggleSetting
            label="Runner status changes"
            description="Notify when a runner goes online, offline, or encounters an error."
            checked={preferences.notify_status_changes}
            onChange={(checked) => updatePreference("notify_status_changes", checked)}
          />
          <Divider />
          <ToggleSetting
            label="Job completions"
            description="Notify when a job completes or fails on a self-hosted runner."
            checked={preferences.notify_job_completions}
            onChange={(checked) => updatePreference("notify_job_completions", checked)}
          />
        </div>
      </section>

      {/* Repository Scanning */}
      <section style={{ marginBottom: 32 }}>
        <SectionHeader title="Repository Scanning" />
        <div className="card">
          {/* Workspace path */}
          <div style={{ padding: "8px 0" }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 500, marginBottom: 2, fontSize: 14 }}>Workspace path</div>
              <p className="text-muted" style={{ margin: 0, fontSize: 12, marginBottom: 8 }}>
                Root directory to scan for local repositories.
              </p>
              <input
                type="text"
                value={preferences.workspace_path ?? ""}
                onChange={(e) => updatePreferences({ workspace_path: e.target.value || null })}
                placeholder="/path/to/workspace"
                style={{ width: "100%", maxWidth: 400 }}
              />
            </div>
          </div>

          <Divider />

          {/* Runner labels */}
          <div style={{ padding: "8px 0" }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 500, marginBottom: 2, fontSize: 14 }}>Runner labels</div>
              <p className="text-muted" style={{ margin: 0, fontSize: 12, marginBottom: 8 }}>
                Comma-separated labels to match in workflow{" "}
                <code
                  style={{
                    background: "var(--bg-tertiary)",
                    padding: "1px 6px",
                    borderRadius: 4,
                    fontSize: 12,
                  }}
                >
                  runs-on:
                </code>{" "}
                fields.
              </p>
              <input
                type="text"
                value={labelsInput}
                onChange={(e) => setLabelsInput(e.target.value)}
                onBlur={() => {
                  const labels = labelsInput
                    .split(",")
                    .map((l) => l.trim())
                    .filter(Boolean);
                  updatePreferences({ scan_labels: labels });
                }}
                placeholder="self-hosted, linux, x64"
                style={{ width: "100%", maxWidth: 400 }}
              />
              {preferences.scan_labels.length > 0 && (
                <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 8 }}>
                  {preferences.scan_labels.map((label) => (
                    <span
                      key={label}
                      style={{
                        fontSize: 11,
                        padding: "2px 8px",
                        borderRadius: 10,
                        background: "rgba(210, 168, 255, 0.15)",
                        color: "var(--accent-purple)",
                      }}
                    >
                      {label}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>

          <Divider />

          {/* Auto-scan toggle */}
          <ToggleSetting
            label="Auto-scan on page load"
            description="Automatically scan when opening the Repositories page."
            checked={preferences.auto_scan}
            onChange={(checked) => updatePreferences({ auto_scan: checked })}
          />
        </div>
      </section>

      {/* About */}
      <section style={{ paddingBottom: 24 }}>
        <SectionHeader title="About" />
        <div className="card" style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <img
              src="/icon.png"
              alt="HomeRun"
              style={{ width: 40, height: 40, borderRadius: 10 }}
            />
            <div>
              <div style={{ fontWeight: 600, fontSize: 14, color: "var(--text-primary)" }}>
                HomeRun
              </div>
              <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                v{appVersion} &middot; MIT &middot;{" "}
                <a
                  href="https://github.com/aGallea"
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: "var(--accent-blue)", textDecoration: "none" }}
                >
                  aGallea
                </a>
              </div>
            </div>
          </div>

          <div
            style={{
              display: "flex",
              gap: 8,
              flexWrap: "wrap",
            }}
          >
            <AboutLink href="https://github.com/aGallea/homerun" icon={<GitHubIcon />}>
              Repository
            </AboutLink>
            <AboutLink
              href="https://github.com/aGallea/homerun/issues/new?template=bug_report.md"
              icon={<BugIcon />}
            >
              Report Bug
            </AboutLink>
            <AboutLink
              href="https://github.com/aGallea/homerun/issues/new?template=feature_request.md"
              icon={<FeatureIcon />}
            >
              Feature Request
            </AboutLink>
          </div>
        </div>
      </section>
    </div>
  );
}

// Helper components

function GitHubIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

function AboutLink({
  href,
  icon,
  children,
}: {
  href: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "6px 12px",
        fontSize: 12,
        color: "var(--text-secondary)",
        textDecoration: "none",
        border: "1px solid var(--border)",
        borderRadius: 6,
        background: "var(--bg-tertiary)",
      }}
    >
      {icon} {children}
    </a>
  );
}

function BugIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="15" r="6" />
      <path d="M12 9V3" />
      <path d="M6.5 9.5L3 6" />
      <path d="M17.5 9.5L21 6" />
      <path d="M6 15H2" />
      <path d="M22 15h-4" />
      <path d="M6.5 20.5L3 24" />
      <path d="M17.5 20.5L21 24" />
    </svg>
  );
}

function FeatureIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
    </svg>
  );
}

function Spinner() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      style={{ animation: "spin 1s linear infinite", flexShrink: 0 }}
      aria-hidden="true"
    >
      <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
      <path d="M12 2a10 10 0 0 1 10 10" />
    </svg>
  );
}

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

function ToggleSetting({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between" style={{ padding: "8px 0" }}>
      <div style={{ flex: 1, marginRight: 24 }}>
        <div style={{ fontWeight: 500, marginBottom: 2, fontSize: 14 }}>{label}</div>
        <p className="text-muted" style={{ margin: 0, fontSize: 12 }}>
          {description}
        </p>
      </div>
      <div
        onClick={() => onChange(!checked)}
        style={{
          width: 40,
          height: 22,
          background: checked ? "var(--accent-green)" : "var(--bg-tertiary)",
          border: `1px solid ${checked ? "var(--accent-green)" : "var(--border)"}`,
          borderRadius: 11,
          cursor: "pointer",
          flexShrink: 0,
          position: "relative",
          transition: "background 0.2s, border-color 0.2s",
        }}
      >
        <div
          style={{
            width: 18,
            height: 18,
            background: checked ? "white" : "var(--text-secondary)",
            borderRadius: "50%",
            position: "absolute",
            top: 1,
            left: checked ? 19 : 1,
            transition: "left 0.2s, background 0.2s",
          }}
        />
      </div>
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
