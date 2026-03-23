import { useState, useEffect } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { api } from "../api/commands";

export function Layout() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [daemonConnected, setDaemonConnected] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function check() {
      try {
        const ok = await api.healthCheck();
        if (!cancelled) setDaemonConnected(ok);
      } catch {
        if (!cancelled) setDaemonConnected(false);
      }
    }
    check();
    const timer = setInterval(check, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  return (
    <div className="app">
      <div className="sidebar-wrapper">
        <Sidebar collapsed={sidebarCollapsed} />
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
          <div className="error-banner" style={{ marginBottom: 12, padding: "12px 16px" }}>
            Unable to connect to the HomeRun daemon. Make sure it is running.
          </div>
        )}
        <Outlet />
      </main>
    </div>
  );
}
