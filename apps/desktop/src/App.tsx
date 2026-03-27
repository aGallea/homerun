import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { AuthProvider } from "./hooks/useAuth";
import { Layout } from "./components/Layout";
import { Dashboard } from "./pages/Dashboard";
import { Repositories } from "./pages/Repositories";
import { RunnerDetail } from "./pages/RunnerDetail";
import { Settings } from "./pages/Settings";
import { Daemon } from "./pages/Daemon";
import { MiniView } from "./pages/MiniView";
import { TrayPanel } from "./pages/TrayPanel";

function App() {
  return (
    <AuthProvider>
      <BrowserRouter>
        <Routes>
          {/* Standalone windows — no Layout wrapper */}
          <Route path="/mini" element={<MiniView />} />
          <Route path="/tray" element={<TrayPanel />} />

          {/* Main app with sidebar layout */}
          <Route element={<Layout />}>
            <Route index element={<Navigate to="/dashboard" replace />} />
            <Route path="/dashboard" element={<Dashboard />} />
            <Route path="/repositories" element={<Repositories />} />
            <Route path="/runners/:id" element={<RunnerDetail />} />
            <Route path="/daemon" element={<Daemon />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </AuthProvider>
  );
}

export default App;
