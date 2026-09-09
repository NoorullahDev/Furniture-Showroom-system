"use client";

import * as React from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Loader2, KeyRound } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/page-header";
import { authChangePassword, settingsGet } from "@/lib/tauri/api";
import { isSessionError, useSession } from "@/components/session/session-provider";
import type { CommandError } from "@/lib/tauri/client";

const SETTING_KEYS: { key: string; label: string }[] = [
  { key: "shop.name", label: "Shop name" },
  { key: "shop.address", label: "Address" },
  { key: "shop.phone", label: "Phone" },
  { key: "shop.email", label: "Email" },
  { key: "shop.currency", label: "Currency" },
  { key: "shop.timezone", label: "Timezone" },
  { key: "invoice.prefix", label: "Invoice prefix" },
  { key: "inventory.issue_policy", label: "Inventory issue policy" },
  { key: "inventory.negative_stock", label: "Negative stock rule" },
  { key: "backup.location", label: "Backup folder" },
];

function displayJson(raw: string | null): string {
  if (raw === null) return "—";
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (typeof parsed === "string") return parsed;
    if (typeof parsed === "boolean" || typeof parsed === "number") return String(parsed);
    return raw;
  } catch {
    return raw;
  }
}

function SettingRow({ label, raw }: { label: string; raw: string | null | undefined }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2.5">
      <dt className="text-sm text-neutral-600">{label}</dt>
      <dd className="max-w-[55%] truncate text-right text-sm font-medium text-neutral-900">
        {raw === undefined ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : displayJson(raw)}
      </dd>
    </div>
  );
}

export function SettingsPage() {
  const { toast } = useToast();
  const { refresh, profile } = useSession();
  const [changing, setChanging] = React.useState(false);

  const allQuery = useQuery({
    queryKey: ["settings"],
    queryFn: async () => {
      const session = profile?.sessionId ?? "";
      const values: Record<string, string | null> = {};
      for (const item of SETTING_KEYS) {
        values[item.key] = await settingsGet(session, item.key);
      }
      return values;
    },
    enabled: !!profile?.sessionId,
  });

  const rows = allQuery.data ?? {};

  const changeMutation = useMutation({
    mutationFn: (input: { currentPassword: string; newPassword: string }) =>
      authChangePassword(profile?.sessionId ?? "", input.currentPassword, input.newPassword),
    onSuccess: () => {
      toast({ variant: "success", title: "Password changed" });
      setChanging(false);
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      const err = e as CommandError;
      toast({
        variant: "error",
        title: "Change failed",
        description:
          err.code === "INVALID_CREDENTIALS"
            ? "The current password is incorrect."
            : err.message || "Unexpected error",
      });
    },
  });

  return (
    <div>
      <PageHeader
        title="Settings"
        subtitle="Read-only view of configured shop values. Editing arrives with Phase 5+."
        actions={
          <Button variant="outline" onClick={() => setChanging(true)}>
            <KeyRound className="h-4 w-4" />
            Change password
          </Button>
        }
      />

      <div className="mt-5 grid grid-cols-1 gap-5 lg:grid-cols-2">
        <section className="rounded-lg border bg-white p-5 shadow-sm">
          <h2 className="text-base font-semibold text-neutral-900">Shop profile</h2>
          <dl className="mt-3 divide-y divide-neutral-100">
            <SettingRow label="Shop name" raw={rows["shop.name"]} />
            <SettingRow label="Address" raw={rows["shop.address"]} />
            <SettingRow label="Phone" raw={rows["shop.phone"]} />
            <SettingRow label="Email" raw={rows["shop.email"]} />
          </dl>
        </section>

        <section className="rounded-lg border bg-white p-5 shadow-sm">
          <h2 className="text-base font-semibold text-neutral-900">Defaults</h2>
          <dl className="mt-3 divide-y divide-neutral-100">
            <SettingRow label="Currency" raw={rows["shop.currency"]} />
            <SettingRow label="Timezone" raw={rows["shop.timezone"]} />
            <SettingRow label="Invoice prefix" raw={rows["invoice.prefix"]} />
            <SettingRow label="Inventory issue policy" raw={rows["inventory.issue_policy"]} />
            <SettingRow label="Negative stock rule" raw={rows["inventory.negative_stock"]} />
            <SettingRow label="Backup folder" raw={rows["backup.location"]} />
          </dl>
        </section>
      </div>

      {changing && (
        <ChangePasswordDialog
          busy={changeMutation.isPending}
          onChange={(currentPassword, newPassword) =>
            changeMutation.mutate({ currentPassword, newPassword })
          }
          onClose={() => setChanging(false)}
        />
      )}
    </div>
  );
}

function ChangePasswordDialog({
  busy,
  onChange,
  onClose,
}: {
  busy: boolean;
  onChange: (currentPassword: string, newPassword: string) => void;
  onClose: () => void;
}) {
  const [current, setCurrent] = React.useState("");
  const [next, setNext] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!current) {
      setError("Enter your current password.");
      return;
    }
    if (next.length < 8 || !/[A-Za-z]/.test(next) || !/\d/.test(next)) {
      setError("New password must be 8+ characters with at least one letter and one digit.");
      return;
    }
    if (next !== confirm) {
      setError("New passwords do not match.");
      return;
    }
    setError(null);
    onChange(current, next);
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Change password</DialogTitle>
          <DialogDescription>
            Other active sessions are not signed out when you change your own password.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="cp-current">Current password</Label>
            <Input
              id="cp-current"
              type="password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
              autoComplete="current-password"
              autoFocus
            />
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="cp-new">New password</Label>
              <Input
                id="cp-new"
                type="password"
                value={next}
                onChange={(e) => setNext(e.target.value)}
                autoComplete="new-password"
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="cp-confirm">Confirm new password</Label>
              <Input
                id="cp-confirm"
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                autoComplete="new-password"
              />
            </div>
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Change password
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}