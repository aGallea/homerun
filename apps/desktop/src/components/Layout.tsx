import { useState } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";

export function Layout() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

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
        <Outlet />
      </main>
    </div>
  );
}
