"use client";

import { Loader2 } from "lucide-react";

import { AppShell } from "@/components/app-shell";
import { FirstRunWizard } from "@/components/session/first-run-wizard";
import { LockScreen } from "@/components/session/lock-screen";
import { LoginScreen } from "@/components/session/login-screen";
import { useSession } from "@/components/session/session-provider";

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

export default function HomePage() {
  const { status } = useSession();

  if (status === "loading") return <Splash />;
  if (status === "first-run") return <FirstRunWizard />;
  if (status === "locked") return <LockScreen />;
  if (status === "logged-out") return <LoginScreen />;
  return <AppShell />;
}