"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Check, Copy, KeyRound, Loader2, RefreshCw, ShieldCheck } from "lucide-react";

import { Button } from "@/components/ui/button";
import { licenseActivate, licenseStatus, type LicenseStatus } from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";

export function LicenseGate({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["license-status"],
    queryFn: licenseStatus,
    retry: 1,
    refetchInterval: 30_000,
  });

  React.useEffect(() => {
    const refresh = () => void queryClient.invalidateQueries({ queryKey: ["license-status"] });
    window.addEventListener("furniture-license-required", refresh);
    return () => window.removeEventListener("furniture-license-required", refresh);
  }, [queryClient]);

  if (query.isLoading) return <LicenseSplash />;
  if (query.isError || !query.data) {
    return <LicenseUnavailable onRetry={() => void query.refetch()} />;
  }
  if (!query.data.isActivated || query.data.status !== "active") {
    return <ActivationScreen status={query.data} />;
  }
  return <>{children}</>;
}

function LicenseSplash() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700 text-white">
      <div className="flex flex-col items-center gap-3">
        <ShieldCheck className="h-11 w-11 text-amber-accent" />
        <Loader2 className="h-5 w-5 animate-spin text-white/70" />
        <p className="text-sm text-white/60">Verifying license…</p>
      </div>
    </div>
  );
}

function LicenseUnavailable({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700 p-6 text-white">
      <div className="max-w-md text-center">
        <AlertTriangle className="mx-auto h-10 w-10 text-amber-300" />
        <h1 className="mt-4 text-xl font-semibold">License verification unavailable</h1>
        <p className="mt-2 text-sm text-white/60">The secure license service could not start. Furniture Shop remains locked.</p>
        <Button className="mt-5 bg-white/10 text-white hover:bg-white/20" onClick={onRetry}>
          <RefreshCw className="h-4 w-4" /> Retry
        </Button>
      </div>
    </div>
  );
}

function ActivationScreen({ status }: { status: LicenseStatus }) {
  return (
    <main className="flex min-h-screen items-center justify-center bg-forest-700 p-5">
      <section className="w-full max-w-2xl overflow-hidden rounded-xl bg-white shadow-2xl">
        <header className="border-b border-neutral-200 bg-neutral-50 px-7 py-6">
          <div className="flex items-start gap-4">
            <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-forest-100 text-forest-700">
              <ShieldCheck className="h-6 w-6" />
            </span>
            <div>
              <h1 className="text-xl font-semibold text-neutral-900">Furniture Shop License</h1>
              <p className="mt-1 text-sm text-neutral-500">A valid offline license is required before the showroom system can open.</p>
            </div>
          </div>
        </header>
        <div className="space-y-5 px-7 py-6">
          <div className="rounded-lg border border-amber-200 bg-amber-50 p-4">
            <p className="font-semibold text-amber-900">{status.label}</p>
            <p className="mt-1 text-sm text-amber-800">{status.message}</p>
          </div>
          <HardwareId value={status.hardwareId} />
          <LicenseKeyForm buttonLabel={status.status === "missing" ? "Activate License" : "Activate / Renew License"} />
          <p className="text-center text-xs text-neutral-400">Verification is fully offline. Send only the Hardware ID to your license provider.</p>
        </div>
      </section>
    </main>
  );
}

export function HardwareId({ value }: { value: string }) {
  const [copied, setCopied] = React.useState(false);
  async function copy() {
    await navigator.clipboard.writeText(value);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }
  return (
    <div>
      <p className="text-xs font-medium uppercase tracking-wide text-neutral-500">Hardware ID</p>
      <div className="mt-2 flex items-center gap-2 rounded-lg border border-neutral-200 bg-neutral-50 p-2">
        <code className="min-w-0 flex-1 select-all px-2 text-base font-semibold tracking-wider text-neutral-900">{value}</code>
        <Button variant="outline" size="sm" onClick={() => void copy()}>
          {copied ? <Check className="h-4 w-4 text-emerald-600" /> : <Copy className="h-4 w-4" />}
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}

export function LicenseKeyForm({ buttonLabel = "Renew License", onActivated }: { buttonLabel?: string; onActivated?: (status: LicenseStatus) => void }) {
  const queryClient = useQueryClient();
  const [key, setKey] = React.useState("");
  const mutation = useMutation({
    mutationFn: () => licenseActivate(key),
    onSuccess: async (next) => {
      setKey("");
      queryClient.setQueryData(["license-status"], next);
      await queryClient.invalidateQueries({ queryKey: ["license-status"] });
      onActivated?.(next);
    },
  });
  return (
    <form onSubmit={(event) => { event.preventDefault(); if (key.trim()) mutation.mutate(); }}>
      <label htmlFor="license-key" className="text-xs font-medium uppercase tracking-wide text-neutral-500">License key</label>
      <textarea
        id="license-key"
        value={key}
        onChange={(event) => setKey(event.target.value)}
        placeholder="Paste the FSLIC1 license key here"
        autoComplete="off"
        spellCheck={false}
        rows={4}
        className="mt-2 w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 font-mono text-xs text-neutral-900 shadow-sm focus:border-forest-500 focus:outline-none focus:ring-2 focus:ring-forest-500/60"
      />
      {mutation.isError && <p role="alert" className="mt-2 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">{commandErrorMessage(mutation.error)}</p>}
      <Button type="submit" className="mt-3 w-full" disabled={!key.trim() || mutation.isPending}>
        {mutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <KeyRound className="h-4 w-4" />}
        {mutation.isPending ? "Verifying…" : buttonLabel}
      </Button>
    </form>
  );
}
