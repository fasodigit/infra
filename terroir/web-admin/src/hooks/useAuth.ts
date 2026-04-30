// SPDX-License-Identifier: AGPL-3.0-or-later
import { createContext, createElement, useContext, useEffect, useState, type ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { whoami, logout as kratosLogout, type KratosSession } from '../auth/kratos-client';

interface AuthState {
  session: KratosSession | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

export function useAuth(): AuthState {
  // Standalone hook : on charge la session à la demande pour chaque
  // protected route. Une factorisation via Provider est possible plus tard.
  const ctx = useContext(AuthContext);
  if (ctx) return ctx;
  return useStandaloneAuth();
}

function useStandaloneAuth(): AuthState {
  const [session, setSession] = useState<KratosSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await whoami();
      setSession(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setSession(null);
    } finally {
      setLoading(false);
    }
  };

  const logout = async () => {
    await kratosLogout();
    setSession(null);
    window.location.assign('/login');
  };

  useEffect(() => {
    refresh();
  }, []);

  return { session, loading, error, refresh, logout };
}

export function RequireAuth({ children }: { children: ReactNode }) {
  const { session, loading } = useAuth();
  const location = useLocation();

  if (loading) {
    return createElement(
      'div',
      { style: { padding: 24 } },
      'Chargement…',
    );
  }
  if (!session || !session.active) {
    return createElement(Navigate, {
      to: '/login',
      replace: true,
      state: { from: location.pathname },
    });
  }
  return createElement('div', { style: { display: 'contents' } }, children);
}
