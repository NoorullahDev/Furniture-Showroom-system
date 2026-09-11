"use client";

import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  authCurrent,
  authLock,
  authLogout,
  authUnlock,
  firstRunStatus,
  type LoginResult,
  type SessionProfile,
} from "@/lib/tauri/api";
import { getStoredSessionId, storeSessionId } from "@/lib/session";
import type { CommandError } from "@/lib/tauri/client";

export type AuthStatus =
  | "loading"
  | "first-run"
  | "logged-out"
  | "locked"
  | "authenticated";

type SessionContextValue = {
  status: AuthStatus;
  profile: SessionProfile | null;
  /** True while a session/lock transition is in flight. */
  busy: boolean;
  applyLogin: (result: LoginResult) => void;
  /** Re-resolve the current session (also recovers from SESSION_LOCKED errors). */
  refresh: () => void;
  lock: () => Promise<void>;
  unlock: (password: string) => Promise<void>;
  logout: () => Promise<void>;
  hasPermission: (code: string) => boolean;
  /** True when startup query failed permanently. */
  startupError: boolean;
  /** Retry the failed startup query. */
  retryStartup: () => void;
};

const SessionContext = React.createContext<SessionContextValue | null>(null);

export function useSession(): SessionContextValue {
  const ctx = React.useContext(SessionContext);
  if (!ctx) throw new Error("useSession must be used inside <SessionProvider>");
  return ctx;
}

export function SessionProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = React.useState<AuthStatus>("loading");
  const [profile, setProfile] = React.useState<SessionProfile | null>(null);
  const [busy, setBusy] = React.useState(false);

  const queryClient = useQueryClient();
  const { data: firstRun, isError: isFirstRunError } = useQuery({
    queryKey: ["firstRunStatus"],
    queryFn: firstRunStatus,
    retry: 1,
  });

  const storedSession = getStoredSessionId();

  const applyLogin = React.useCallback((result: LoginResult) => {
    storeSessionId(result.sessionId);
    setProfile(result.profile);
    setStatus("authenticated");
  }, []);

  const refresh = React.useCallback(() => {
    const id = getStoredSessionId();
    if (!id) {
      setProfile(null);
      setStatus("logged-out");
      return;
    }
    void authCurrent(id)
      .then((resolved) => {
        if (!resolved) {
          storeSessionId(null);
          setProfile(null);
          setStatus("logged-out");
          return;
        }
        if (resolved.lockedAt) {
          setProfile(resolved);
          setStatus("locked");
          return;
        }
        setProfile(resolved);
        setStatus("authenticated");
      })
      .catch(() => {
        // Command failed; treat as logged out so the login screen reappears.
        setProfile(null);
        setStatus("logged-out");
      });
  }, []);

  React.useEffect(() => {
    if (firstRun === undefined) return;
    if (firstRun.required) {
      setStatus("first-run");
      return;
    }
    if (storedSession) {
      refresh();
    } else {
      setStatus("logged-out");
    }
  }, [firstRun, storedSession, refresh]);

  const lock = React.useCallback(async () => {
    const id = profile?.sessionId ?? getStoredSessionId();
    if (!id) return;
    setBusy(true);
    try {
      await authLock(id);
      if (profile) setProfile({ ...profile, lockedAt: new Date().toISOString() });
      setStatus("locked");
    } finally {
      setBusy(false);
    }
  }, [profile]);

  const unlock = React.useCallback(
    async (password: string) => {
      const id = profile?.sessionId;
      if (!id) {
        refresh();
        return;
      }
      setBusy(true);
      try {
        const resolved = await authUnlock(id, password);
        setProfile(resolved);
        setStatus("authenticated");
      } finally {
        setBusy(false);
      }
    },
    [profile, refresh],
  );

  const logout = React.useCallback(async () => {
    const id = profile?.sessionId ?? getStoredSessionId();
    setBusy(true);
    try {
      if (id) await authLogout(id);
    } catch {
      // Logout is best-effort; the local session is cleared regardless.
    } finally {
      storeSessionId(null);
      setProfile(null);
      setStatus("logged-out");
      setBusy(false);
    }
  }, [profile]);

  const hasPermission = React.useCallback(
    (code: string) => profile?.permissions.includes(code) ?? false,
    [profile],
  );

  // Detect permanent startup failure: query failed after retries and firstRun is still undefined.
  const startupFailed = isFirstRunError && firstRun === undefined;

  const retryStartup = React.useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["firstRunStatus"] });
  }, [queryClient]);

  const value = React.useMemo<SessionContextValue>(
    () => ({
      status,
      profile,
      busy,
      applyLogin,
      refresh,
      lock,
      unlock,
      logout,
      hasPermission,
      startupError: startupFailed,
      retryStartup,
    }),
    [status, profile, busy, applyLogin, refresh, lock, unlock, logout, hasPermission, startupFailed, retryStartup],
  );

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function isSessionError(error: unknown): boolean {
  const e = error as CommandError;
  return e.code === "SESSION_LOCKED" || e.code === "UNAUTHORIZED";
}