import { useState, useEffect, useRef } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { api } from "../api/commands";
import { useRunners } from "../hooks/useRunners";

export function Layout() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [daemonConnected, setDaemonConnected] = useState(true);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [starting, setStarting] = useState(false);
  const wasDisconnectedRef = useRef(false);
  const runnersHook = useRunners();

  useEffect(() => {
    let cancelled = false;
    async function check() {
      try {
        const ok = await api.healthCheck();
        if (!cancelled) {
          if (ok) {
            setRetryAttempt(0);
            wasDisconnectedRef.current = false;
          } else {
            wasDisconnectedRef.current = true;
            setRetryAttempt((n) => n + 1);
          }
          setDaemonConnected(ok);
        }
      } catch {
        if (!cancelled) {
          wasDisconnectedRef.current = true;
          setRetryAttempt((n) => n + 1);
          setDaemonConnected(false);
        }
      }
    }
    check();
    const timer = setInterval(check, 10000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return (
    <div className="app">
      <div className="sidebar-wrapper">
        <Sidebar collapsed={sidebarCollapsed} runners={runnersHook.runners} />
        <button
          className="sidebar-fab"
          onClick={() => setSidebarCollapsed((c) => !c)}
          title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {sidebarCollapsed ? "›" : "‹"}
        </button>
      </div>
      <main className="main-content">
        {!daemonConnected && (
          <div
            className="error-banner"
            style={{
              margin: "16px 24px 0",
              padding: "12px 16px",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              <span>Unable to connect to the HomeRun daemon.</span>
              <span style={{ fontSize: 12, opacity: 0.7 }}>
                Retrying... (attempt {retryAttempt})
              </span>
            </div>
            <button
              className="btn btn-primary btn-sm"
              disabled={starting}
              onClick={async () => {
                setStarting(true);
                try {
                  await api.startDaemon();
                } catch (err) {
                  console.error("Failed to start daemon:", err);
                } finally {
                  setStarting(false);
                }
              }}
            >
              {starting ? "Starting..." : "Start daemon"}
            </button>
          </div>
        )}
        <Outlet context={runnersHook} />
      </main>
    </div>
  );
}
