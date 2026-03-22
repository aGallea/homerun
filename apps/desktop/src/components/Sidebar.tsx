import { NavLink, useNavigate } from "react-router-dom";
import { useAuth } from "../hooks/useAuth";

const navItems = [
  { to: "/", label: "Dashboard", icon: "⊞" },
  { to: "/repositories", label: "Repositories", icon: "⊟" },
  { to: "/settings", label: "Settings", icon: "✱" },
];

export function Sidebar() {
  const { auth } = useAuth();
  const navigate = useNavigate();

  return (
    <nav className="sidebar">
      <div className="sidebar-header">
        <img src="/icon.png" alt="HomeRun" style={{ width: 48, height: 48, borderRadius: 10 }} />
        <span className="sidebar-title">HomeRun</span>
      </div>
      <div className="sidebar-nav">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) => `sidebar-link${isActive ? " sidebar-link-active" : ""}`}
          >
            <span className="sidebar-icon">{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </div>
      <div className="sidebar-footer">
        {auth.user ? (
          <div className="sidebar-user">
            <img className="sidebar-avatar" src={auth.user.avatar_url} alt={auth.user.login} />
            <span className="sidebar-username">{auth.user.login}</span>
          </div>
        ) : (
          <button
            className="btn btn-primary"
            onClick={() => navigate("/settings")}
            style={{
              width: "100%",
              fontSize: 13,
              padding: "8px 12px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 6,
            }}
          >
            Sign in with GitHub
          </button>
        )}
      </div>
    </nav>
  );
}
