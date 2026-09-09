"use client";

import * as React from "react";
import { Loader2, LockKeyhole, LogOut } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useSession } from "@/components/session/session-provider";
import type { CommandError } from "@/lib/tauri/client";

export function LockScreen() {
  const { profile, unlock, logout, busy } = useSession();
  const [password, setPassword] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [submitting, setSubmitting] = React.useState(false);

  const initial = (profile?.fullName || profile?.username || "You")
    .split(/\s+/)
    .slice(0, 2)
    .map((s) => s[0] ?? "")
    .join("")
    .toUpperCase();

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!password) return;
    setSubmitting(true);
    setError(null);
    try {
      await unlock(password);
    } catch (err) {
      const e2 = err as CommandError;
      setError(
        e2.code === "INVALID_CREDENTIALS" ? "Incorrect password." : e2.message || "Unlock failed.",
      );
      setPassword("");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700 p-4">
      <div className="w-full max-w-sm rounded-lg border border-white/10 bg-white p-6 shadow-xl sm:p-8">
        <div className="mb-6 flex flex-col items-center gap-3 text-center">
          <span className="flex h-12 w-12 items-center justify-center rounded-full bg-forest-100 text-forest-700">
            <LockKeyhole className="h-6 w-6" />
          </span>
          <div className="flex items-center gap-2">
            <span className="flex h-9 w-9 items-center justify-center rounded-full bg-amber-accent text-xs font-semibold text-white">
              {initial}
            </span>
            <span className="text-left">
              <p className="text-sm font-semibold text-neutral-900">
                {profile?.fullName || profile?.username || "Workstation locked"}
              </p>
              <p className="text-xs text-neutral-500">Session locked — sign in to continue</p>
            </span>
          </div>
        </div>

        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="lock-password">Password</Label>
            <Input
              id="lock-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              autoFocus
              disabled={submitting || busy}
            />
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <Button type="submit" disabled={!password || submitting || busy}>
            {submitting || busy ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                Unlocking…
              </>
            ) : (
              "Unlock"
            )}
          </Button>
          <Button type="button" variant="ghost" onClick={logout} disabled={busy}>
            <LogOut className="h-4 w-4" />
            Sign out instead
          </Button>
        </form>
      </div>
    </div>
  );
}