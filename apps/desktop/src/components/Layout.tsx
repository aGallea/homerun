import { useState, useEffect, useRef, useCallback } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { Sidebar } from "./Sidebar";
import { api } from "../api/commands";
import { useRunners } from "../hooks/useRunners";
import { useTrayIcon } from "../hooks/useTrayIcon";
import { useNotifications } from "../hooks/useNotifications";
import type { Preferences } from "../api/types";

const SIDEBAR_COLLAPSE_WIDTH = 900;

export function Layout() {
  const navigate = useNavigate();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(
    () => window.innerWidth < SIDEBAR_COLLAPSE_WIDTH,
  );
  const [daemonConnected, setDaemonConnected] = useState(true);
  const [dotCount, setDotCount] = useState(0);
  const [starting, setStarting] = useState(false);
  const wasDisconnectedRef = useRef(false);
  const runnersHook = useRunners();
  const [notifPrefs, setNotifPrefs] = useState<Preferences | null>(null);
  useTrayIcon(runnersHook.runners, daemonConnected);
  useNotifications(runnersHook.runners, notifPrefs);

  useEffect(() => {
    api
      .getPreferences()
      .then(setNotifPrefs)
      .catch(() => {});
    const interval = setInterval(() => {
      api
        .getPreferences()
        .then(setNotifPrefs)
        .catch(() => {});
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  const handleStartDaemon = useCallback(async () => {
    setStarting(true);
    try {
      await api.startDaemon();
      for (let i = 0; i < 10; i++) {
        await new Promise((r) => setTimeout(r, 500));
        try {
          if (await api.healthCheck()) {
            setDaemonConnected(true);
            break;
          }
        } catch {
          /* keep polling */
        }
      }
    } catch (err) {
      console.error("Failed to start daemon:", err);
    } finally {
      setStarting(false);
    }
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("navigate", (event) => {
      navigate(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [navigate]);

  useEffect(() => {
    const onResize = () => {
      setSidebarCollapsed(window.innerWidth < SIDEBAR_COLLAPSE_WIDTH);
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useEffect(() => {
    if (!daemonConnected) {
      const timer = setInterval(() => setDotCount((n) => (n + 1) % 4), 1500);
      return () => clearInterval(timer);
    }
    setDotCount(0);
  }, [daemonConnected]);

  useEffect(() => {
    let cancelled = false;
    async function check() {
      try {
        const ok = await api.healthCheck();
        if (!cancelled) {
          if (ok) {
            wasDisconnectedRef.current = false;
          } else {
            wasDisconnectedRef.current = true;
          }
          setDaemonConnected(ok);
        }
      } catch {
        if (!cancelled) {
          wasDisconnectedRef.current = true;
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
              <span style={{ fontSize: 12, opacity: 0.7 }}>Retrying{".".repeat(dotCount)}</span>
            </div>
            <button
              className="btn btn-primary btn-sm"
              disabled={starting}
              onClick={handleStartDaemon}
            >
              {starting ? "Starting..." : "Start daemon"}
            </button>
          </div>
        )}
        <Outlet context={{ ...runnersHook, daemonStarting: starting, handleStartDaemon }} />
      </main>
    </div>
  );
}
