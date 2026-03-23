import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";
import type { AuthStatus } from "../api/types";
import { api } from "../api/commands";

interface AuthContextValue {
  auth: AuthStatus;
  loading: boolean;
  error: string | null;
  loginWithToken: (token: string) => Promise<AuthStatus>;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
  /** Call when an API returns 401 to re-sync auth state from the daemon. */
  handleUnauthorized: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [auth, setAuth] = useState<AuthStatus>({
    authenticated: false,
    user: null,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const status = await api.getAuthStatus();
      setAuth(status);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleUnauthorized = useCallback(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    refresh();
    // Poll auth status to stay in sync with daemon (e.g. after daemon restart)
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  const loginWithToken = useCallback(async (token: string) => {
    try {
      setError(null);
      const status = await api.loginWithToken(token);
      setAuth(status);
      return status;
    } catch (e) {
      const msg = String(e);
      setError(msg);
      throw new Error(msg);
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await api.logout();
      setAuth({ authenticated: false, user: null });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return (
    <AuthContext.Provider
      value={{ auth, loading, error, loginWithToken, logout, refresh, handleUnauthorized }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
