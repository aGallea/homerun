import {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useRef,
  type ReactNode,
} from "react";
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
  const operationGeneration = useRef(0);

  const refresh = useCallback(async () => {
    const generation = ++operationGeneration.current;
    try {
      const status = await api.getAuthStatus();
      if (generation !== operationGeneration.current) return;
      setError(null);
      setAuth(status);
    } catch (e) {
      if (generation === operationGeneration.current) setError(String(e));
    } finally {
      if (generation === operationGeneration.current) setLoading(false);
    }
  }, []);

  const handleUnauthorized = useCallback(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    const poll = async () => {
      await refresh();
      if (!cancelled) {
        timer = window.setTimeout(() => void poll(), 5000);
      }
    };

    void poll();
    return () => {
      cancelled = true;
      operationGeneration.current += 1;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [refresh]);

  const loginWithToken = useCallback(async (token: string) => {
    try {
      setError(null);
      const status = await api.loginWithToken(token);
      operationGeneration.current += 1;
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
      setError(null);
      await api.logout();
      operationGeneration.current += 1;
      setAuth({ authenticated: false, user: null });
    } catch (e) {
      const message = String(e);
      operationGeneration.current += 1;
      setAuth({ authenticated: false, user: null });
      setError(message);
      throw new Error(message);
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
