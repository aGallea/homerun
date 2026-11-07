import { useState, useEffect, useCallback } from "react";
import { Outlet, useNavigate } from "react-router";
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
  const [daemonConnected, setDaemonConnected] = useState<boolean | null>(null);
  const [dotCount, setDotCount] = useState(0);
  const [starting, setStarting] = useState(false);
  const runnersHook = useRunners();
  const [notifPrefs, setNotifPrefs] = useState<Preferences | null>(null);
  useTrayIcon(runnersHook.runners, daemonConnected === true);
  useNotifications(runnersHook.runners, notifPrefs);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    const pollPreferences = async () => {
      try {
        const preferences = await api.getPreferences();
        if (!cancelled) setNotifPrefs(preferences);
      } catch {
        // Keep the last known notification preferences while the daemon transitions.
      } finally {
        if (!cancelled) {
          timer = window.setTimeout(() => void pollPreferences(), 2000);
        }
      }
    };

    void pollPreferences();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
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
    let disposed = false;
    let removeListener: (() => void) | undefined;

    try {
      void listen<string>("navigate", (event) => {
        if (!disposed) navigate(event.payload);
      })
        .then((unlisten) => {
          if (disposed) unlisten();
          else removeListener = unlisten;
        })
        .catch(() => {
          // Browser previews and unit tests do not provide a Tauri event bridge.
        });
    } catch {
      // Treat a synchronously unavailable bridge like an unavailable stream.
    }

    return () => {
      disposed = true;
      removeListener?.();
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
    if (daemonConnected === false) {
      const timer = setInterval(() => setDotCount((n) => (n + 1) % 4), 1500);
      return () => clearInterval(timer);
    }
    setDotCount(0);
  }, [daemonConnected]);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    async function check() {
      try {
        const ok = await api.healthCheck();
        if (!cancelled) setDaemonConnected(ok);
      } catch {
        if (!cancelled) setDaemonConnected(false);
      } finally {
        if (!cancelled) {
          timer = window.setTimeout(() => void check(), 10000);
        }
      }
    }

    void check();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
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
        {daemonConnected === false && (
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
