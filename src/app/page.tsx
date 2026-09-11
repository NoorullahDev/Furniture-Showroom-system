"use client";

import { Loader2, RefreshCw, AlertTriangle } from "lucide-react";

import { AppShell } from "@/components/app-shell";
import { FirstRunWizard } from "@/components/session/first-run-wizard";
import { LockScreen } from "@/components/session/lock-screen";
import { LoginScreen } from "@/components/session/login-screen";
import { useSession } from "@/components/session/session-provider";
import { Button } from "@/components/ui/button";

function Splash() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700">
      <div className="flex flex-col items-center gap-3 text-white">
        <span className="flex h-12 w-12 animate-pulse items-center justify-center rounded-lg bg-amber-accent text-sm font-bold">
          FS
        </span>
        <Loader2 className="h-5 w-5 animate-spin text-white/70" />
        <p className="text-xs text-white/50">Starting Furniture Shop…</p>
      </div>
    </div>
  );
}

function StartupErrorScreen({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700">
      <div className="flex flex-col items-center gap-4 text-white max-w-sm text-center">
        <span className="flex h-12 w-12 items-center justify-center rounded-lg bg-red-500/20 text-sm font-bold">
          <AlertTriangle className="h-6 w-6 text-red-400" />
        </span>
        <h2 className="text-lg font-semibold">Startup Failed</h2>
        <p className="text-sm text-white/60">
          Could not connect to the backend. The database may be locked by
          another instance, or a startup error occurred.
        </p>
        <Button
          onClick={onRetry}
          className="bg-white/10 hover:bg-white/20 text-white border border-white/20"
        >
          <RefreshCw className="mr-2 h-4 w-4" />
          Retry
        </Button>
      </div>
    </div>
  );
}

export default function HomePage() {
  const { status, startupError, retryStartup } = useSession();

  if (startupError) return <StartupErrorScreen onRetry={retryStartup} />;
  if (status === "loading") return <Splash />;
  if (status === "first-run") return <FirstRunWizard />;
  if (status === "locked") return <LockScreen />;
  if (status === "logged-out") return <LoginScreen />;
  return <AppShell />;
}