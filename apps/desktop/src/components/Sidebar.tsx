import { NavLink } from "react-router-dom";
import { useAuth } from "../hooks/useAuth";

const navItems = [
  { to: "/", label: "Dashboard", icon: "⊞" },
  { to: "/repositories", label: "Repositories", icon: "⊟" },
  { to: "/settings", label: "Settings", icon: "✱" },
];

export function Sidebar() {
  const { auth } = useAuth();

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
          <div className="sidebar-user">
            <span className="sidebar-username text-muted">Not signed in</span>
          </div>
        )}
      </div>
    </nav>
  );
}
