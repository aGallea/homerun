import { useState, useEffect, useCallback } from "react";
import type { AuthStatus } from "../api/types";
import { api } from "../api/commands";

export function useAuth() {
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

  useEffect(() => {
    refresh();
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

  return { auth, loading, error, loginWithToken, logout, refresh };
}
