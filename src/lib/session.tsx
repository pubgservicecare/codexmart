import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { api, onApiError, type AppStatus, type Permission, type Session, type Settings } from './api';
import { setMoneyFormat } from './format';

interface SessionValue {
  status: AppStatus | null;
  session: Session | null;
  settings: Settings | null;
  /** Set when the settings could not be loaded after signing in. */
  settingsError: unknown;
  reloadSettings: () => void;
  loading: boolean;
  can: (p: Permission) => boolean;
  setSession: (s: Session | null) => void;
  refreshStatus: () => Promise<void>;
  signOut: () => Promise<void>;
}

const Ctx = createContext<SessionValue | null>(null);

export function SessionProvider({ children }: { children: ReactNode }) {
  const qc = useQueryClient();
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [session, setSessionState] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.app.status();
      setStatus(s);
      setSessionState(s.session);
      // Signed out by the backend (e.g. after a restore): drop cached data.
      if (!s.session) qc.clear();
    } finally {
      setLoading(false);
    }
  }, [qc]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const settingsQuery = useQuery({
    queryKey: ['settings'],
    queryFn: api.settings.get,
    enabled: !!session,
    staleTime: Infinity,
  });
  const settings = session ? (settingsQuery.data ?? null) : null;
  const settingsError = session ? settingsQuery.error : null;
  const { refetch: refetchSettings } = settingsQuery;
  const reloadSettings = useCallback(() => void refetchSettings(), [refetchSettings]);

  useEffect(() => {
    if (settings) setMoneyFormat({ symbol: settings.currencySymbol, decimals: settings.currencyDecimals });
  }, [settings]);

  const setSession = useCallback(
    (s: Session | null) => {
      // Every sign-in starts on the user's home screen, not on the page the
      // previous user left open.
      if (s) window.location.hash = '#/';
      setSessionState(s);
      if (!s) qc.clear();
    },
    [qc],
  );

  // The backend ends the session (e.g. after a restore): go back to sign-in.
  useEffect(
    () =>
      onApiError((e) => {
        if (e.code === 'UNAUTHENTICATED') {
          setSessionState(null);
          qc.clear();
        }
      }),
    [qc],
  );

  const signOut = useCallback(async () => {
    await api.auth.logout().catch(() => undefined);
    setSession(null);
  }, [setSession]);

  const value = useMemo<SessionValue>(
    () => ({
      status,
      session,
      settings,
      settingsError,
      reloadSettings,
      loading,
      can: (p) => !!session?.permissions.includes(p),
      setSession,
      refreshStatus,
      signOut,
    }),
    [status, session, settings, settingsError, reloadSettings, loading, setSession, refreshStatus, signOut],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSession(): SessionValue {
  const v = useContext(Ctx);
  if (!v) throw new Error('useSession outside SessionProvider');
  return v;
}

/**
 * Settings of the signed-in session. The app shell is only rendered after
 * they have loaded (see `Root` in App.tsx), so screens can rely on them.
 */
export function useSettings(): Settings {
  const { settings } = useSession();
  if (!settings) throw new Error('Settings not loaded');
  return settings;
}
