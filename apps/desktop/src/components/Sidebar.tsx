import { NavLink, useNavigate } from "react-router-dom";
import { useAuth } from "../hooks/useAuth";

function DashboardIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
    </svg>
  );
}

function RepositoriesIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

const navItems = [
  { to: "/", label: "Dashboard", icon: <DashboardIcon /> },
  { to: "/repositories", label: "Repositories", icon: <RepositoriesIcon /> },
  { to: "/settings", label: "Settings", icon: <SettingsIcon /> },
];

export function Sidebar({ collapsed }: { collapsed: boolean }) {
  const { auth } = useAuth();
  const navigate = useNavigate();

  return (
    <nav className={`sidebar ${collapsed ? "sidebar-collapsed" : ""}`}>
      <div className="sidebar-header">
        <img src="/icon.png" alt="HomeRun" style={{ width: 48, height: 48, borderRadius: 12 }} />
        {!collapsed && <span className="sidebar-title">HomeRun</span>}
      </div>
      <div className="sidebar-nav">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) => `sidebar-link${isActive ? " sidebar-link-active" : ""}`}
            title={collapsed ? item.label : undefined}
          >
            <span className="sidebar-icon">{item.icon}</span>
            {!collapsed && item.label}
          </NavLink>
        ))}
      </div>
      <div className="sidebar-footer">
        {auth.user ? (
          <div className="sidebar-user">
            <img className="sidebar-avatar" src={auth.user.avatar_url} alt={auth.user.login} />
            {!collapsed && <span className="sidebar-username">{auth.user.login}</span>}
          </div>
        ) : (
          <div
            className="sidebar-user"
            style={{
              justifyContent: collapsed ? "center" : "space-between",
              alignItems: "center",
            }}
          >
            {!collapsed && <span className="sidebar-username text-muted">Not signed in</span>}
            <button
              className="btn btn-sm"
              onClick={() => navigate("/settings")}
              style={{ fontSize: 11, padding: "3px 8px" }}
              title={collapsed ? "Sign in" : undefined}
            >
              {collapsed ? "→" : "Sign in"}
            </button>
          </div>
        )}
      </div>
    </nav>
  );
}
