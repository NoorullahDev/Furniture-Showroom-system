"use client";

import * as React from "react";
import { Loader2, LockKeyhole, Store } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  firstRunComplete,
  type FirstRunCompleteInput,
  type LoginResult,
} from "@/lib/tauri/api";
import { useSession } from "@/components/session/session-provider";
import type { CommandError } from "@/lib/tauri/client";

const CURRENCIES = [
  "PKR",
  "USD",
  "EUR",
  "GBP",
  "AED",
  "SAR",
  "INR",
  "AFN",
  "IRR",
  "IQD",
  "BDT",
  "CHF",
  "CAD",
  "AUD",
  "MYR",
  "IDR",
  "CNY",
  "JPY",
];

const TIMEZONES = [
  "Asia/Karachi",
  "Asia/Kota_Kinabalu",
  "Asia/Dhaka",
  "Asia/Kolkata",
  "Asia/Kabul",
  "Asia/Riyadh",
  "Asia/Dubai",
  "Asia/Tehran",
  "Asia/Baghdad",
  "Asia/Tokyo",
  "Asia/Singapore",
  "Asia/Shanghai",
  "Europe/London",
  "Europe/Berlin",
  "Africa/Nairobi",
  "UTC",
  "America/New_York",
  "America/Chicago",
  "America/Denver",
  "America/Los_Angeles",
];

type FormState = {
  shopName: string;
  shopAddress: string;
  shopPhone: string;
  shopEmail: string;
  currency: string;
  timezone: string;
  invoicePrefix: string;
  backupLocation: string;
  ownerUsername: string;
  ownerFullName: string;
  ownerPassword: string;
  ownerPasswordConfirm: string;
};

function emptyForm(): FormState {
  return {
    shopName: "",
    shopAddress: "",
    shopPhone: "",
    shopEmail: "",
    currency: "PKR",
    timezone: "Asia/Karachi",
    invoicePrefix: "INV/",
    backupLocation: "",
    ownerUsername: "",
    ownerFullName: "",
    ownerPassword: "",
    ownerPasswordConfirm: "",
  };
}

const STEPS = ["Shop details", "Currency & location", "Invoicing & backup", "Owner account"];

function StepMarker({ current }: { current: number }) {
  return (
    <ol className="flex items-center gap-2 text-xs" aria-label="Setup steps">
      {STEPS.map((label, i) => (
        <li key={label} className="flex items-center gap-2">
          <span
            className={
              i === current
                ? "flex h-5 w-5 items-center justify-center rounded-full bg-forest-600 text-[10px] font-bold text-white"
                : i < current
                  ? "flex h-5 w-5 items-center justify-center rounded-full bg-forest-100 text-[10px] font-bold text-forest-700"
                  : "flex h-5 w-5 items-center justify-center rounded-full bg-neutral-200 text-[10px] font-medium text-neutral-500"
            }
          >
            {i + 1}
          </span>
          <span className={i <= current ? "text-neutral-800" : "text-neutral-400"}>{label}</span>
          {i < STEPS.length - 1 && <span className="h-px w-4 bg-neutral-300" />}
        </li>
      ))}
    </ol>
  );
}

function Field({
  id,
  label,
  error,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}

const USERNAME_RE = /^[a-zA-Z0-9._-]+$/;
const INVOICE_PREFIX_RE = /^[a-zA-Z0-9/._-]+$/;

export function FirstRunWizard() {
  const { toast } = useToast();
  const { applyLogin } = useSession();
  const [step, setStep] = React.useState(0);
  const [form, setForm] = React.useState<FormState>(emptyForm);
  const [errors, setErrors] = React.useState<Partial<Record<string, string>>>({});
  const [submitting, setSubmitting] = React.useState(false);

  const set = (key: keyof FormState) => (value: string) => {
    setForm((f) => ({ ...f, [key]: value }));
  };

  function validateStep(index: number): boolean {
    const next: Partial<Record<string, string>> = {};
    if (index === 0) {
      const name = form.shopName.trim();
      if (name.length === 0 || name.length > 100) {
        next.shopName = "Shop name is required (100 characters or fewer).";
      }
    }
    if (index === 1) {
      const current = form.currency.trim().toUpperCase();
      if (current.length !== 3 || !/^[A-Z]{3}$/.test(current)) {
        next.currency = "Currency must be a 3-letter code, e.g. PKR.";
      }
      if (form.timezone.trim().length === 0) {
        next.timezone = "Choose a timezone.";
      }
    }
    if (index === 2) {
      const prefix = form.invoicePrefix.trim();
      if (prefix.length === 0) {
        next.invoicePrefix = "Enter a default invoice prefix.";
      } else if (prefix.length > 16 || !INVOICE_PREFIX_RE.test(prefix)) {
        next.invoicePrefix = "Use 1-16 characters: letters, digits, '/', '-', '_' or '.'.";
      }
      const backup = form.backupLocation.trim();
      if (backup.length > 0 && !backup.startsWith("\\\\") && !/^[A-Za-z]:[\\/]/.test(backup)) {
        next.backupLocation = "Backup location must be an absolute folder path.";
      }
    }
    if (index === 3) {
      const username = form.ownerUsername.trim().toLowerCase();
      if (username.length < 3 || username.length > 64 || !USERNAME_RE.test(username)) {
        next.ownerUsername = "Usernames use 3+ characters: letters, digits, '.', '_' or '-'.";
      }
      const fullName = form.ownerFullName.trim();
      if (fullName.length === 0 || fullName.length > 200) {
        next.ownerFullName = "Full name is required.";
      }
      const pw = form.ownerPassword;
      if (pw.length < 8 || !/[A-Za-z]/.test(pw) || !/\d/.test(pw)) {
        next.ownerPassword = "Password must be 8+ characters with at least one letter and one digit.";
      } else if (pw !== form.ownerPasswordConfirm) {
        next.ownerPasswordConfirm = "Passwords do not match.";
      }
    }
    setErrors(next);
    return Object.keys(next).length === 0;
  }

  function next() {
    if (!validateStep(step)) return;
    setStep((s) => Math.min(s + 1, STEPS.length - 1));
  }

  function back() {
    setErrors({});
    setStep((s) => Math.max(s - 1, 0));
  }

  async function submit() {
    if (!validateStep(step)) return;
    setSubmitting(true);
    const input: FirstRunCompleteInput = {
      shopName: form.shopName.trim(),
      shopAddress: form.shopAddress.trim(),
      shopPhone: form.shopPhone.trim(),
      shopEmail: form.shopEmail.trim(),
      currency: form.currency.trim().toUpperCase(),
      timezone: form.timezone.trim(),
      invoicePrefix: form.invoicePrefix.trim(),
      backupLocation: form.backupLocation.trim() || null,
      ownerUsername: form.ownerUsername.trim().toLowerCase(),
      ownerFullName: form.ownerFullName.trim(),
      ownerPassword: form.ownerPassword,
    };
    try {
      const result: LoginResult = await firstRunComplete(input);
      toast({
        variant: "success",
        title: "Setup complete",
        description: `Welcome, ${result.profile.fullName}.`,
      });
      applyLogin(result);
    } catch (e) {
      const err = e as CommandError;
      toast({
        variant: "error",
        title: "Setup failed",
        description: err.message || "Unexpected error",
      });
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-forest-700 p-4">
      <div className="w-full max-w-xl rounded-lg border border-white/10 bg-white p-6 shadow-xl sm:p-8">
        <div className="mb-5 flex items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded bg-amber-accent text-white">
            <Store className="h-5 w-5" />
          </span>
          <div>
            <h1 className="text-xl font-semibold text-forest-700">Welcome to Furniture Shop</h1>
            <p className="text-sm text-neutral-500">
              One-time setup · Powered by EagleNest Creations
            </p>
          </div>
        </div>

        <StepMarker current={step} />

        <div className="mt-6 grid gap-4">
          {step === 0 && (
            <>
              <Field id="shopName" label="Shop name" error={errors.shopName}>
                <Input
                  id="shopName"
                  value={form.shopName}
                  onChange={(e) => set("shopName")(e.target.value)}
                  placeholder="e.g. Shahid Furnitures"
                  autoFocus
                />
              </Field>
              <Field id="shopAddress" label="Address (optional)">
                <Input
                  id="shopAddress"
                  value={form.shopAddress}
                  onChange={(e) => set("shopAddress")(e.target.value)}
                  placeholder="Street, city"
                />
              </Field>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field id="shopPhone" label="Phone (optional)">
                  <Input
                    id="shopPhone"
                    value={form.shopPhone}
                    onChange={(e) => set("shopPhone")(e.target.value)}
                    placeholder="0300-1234567"
                  />
                </Field>
                <Field id="shopEmail" label="Email (optional)">
                  <Input
                    id="shopEmail"
                    type="email"
                    value={form.shopEmail}
                    onChange={(e) => set("shopEmail")(e.target.value)}
                    placeholder="shop@example.com"
                  />
                </Field>
              </div>
            </>
          )}

          {step === 1 && (
            <>
              <Field id="currency" label="Currency" error={errors.currency}>
                <div className="flex gap-2">
                  <Select value={form.currency} onValueChange={set("currency")}>
                    <SelectTrigger id="currency" className="w-32">
                      <SelectValue placeholder="PKR" />
                    </SelectTrigger>
                    <SelectContent>
                      {CURRENCIES.map((c) => (
                        <SelectItem key={c} value={c}>
                          {c}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Input
                    aria-label="Currency code (custom)"
                    value={form.currency}
                    onChange={(e) => set("currency")(e.target.value.toUpperCase())}
                    maxLength={3}
                    className="max-w-24"
                  />
                </div>
                <p className="text-xs text-neutral-500">
                  Amounts are stored in minor units and displayed with this code.
                </p>
              </Field>
              <Field id="timezone" label="Timezone" error={errors.timezone}>
                <Select value={form.timezone} onValueChange={set("timezone")}>
                  <SelectTrigger id="timezone">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {TIMEZONES.map((tz) => (
                      <SelectItem key={tz} value={tz}>
                        {tz}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            </>
          )}

          {step === 2 && (
            <>
              <Field id="invoicePrefix" label="Invoice prefix" error={errors.invoicePrefix}>
                <Input
                  id="invoicePrefix"
                  value={form.invoicePrefix}
                  onChange={(e) => set("invoicePrefix")(e.target.value)}
                  placeholder="INV/"
                />
                <p className="text-xs text-neutral-500">
                  Used as the start of every invoice number, e.g. INV/0001.
                </p>
              </Field>
              <Field id="backupLocation" label="Backup folder (optional)" error={errors.backupLocation}>
                <Input
                  id="backupLocation"
                  value={form.backupLocation}
                  onChange={(e) => set("backupLocation")(e.target.value)}
                  placeholder="D:\FurnitureShop\Backups"
                />
                <p className="text-xs text-neutral-500">
                  Leave empty to use the default app-data backup folder.
                </p>
              </Field>
              <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-xs text-amber-900">
                Inventory policy will default to <strong>issue on confirmation</strong> and{" "}
                <strong>negative stock blocked</strong>. These can be changed later.
              </div>
            </>
          )}

          {step === 3 && (
            <>
              <div className="flex items-center gap-2 text-xs text-neutral-500">
                <LockKeyhole className="h-3.5 w-3.5" />
                This account becomes the Owner with full permissions.
              </div>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field id="ownerUsername" label="Username" error={errors.ownerUsername}>
                  <Input
                    id="ownerUsername"
                    value={form.ownerUsername}
                    onChange={(e) => set("ownerUsername")(e.target.value)}
                    autoComplete="username"
                    autoFocus
                  />
                </Field>
                <Field id="ownerFullName" label="Full name" error={errors.ownerFullName}>
                  <Input
                    id="ownerFullName"
                    value={form.ownerFullName}
                    onChange={(e) => set("ownerFullName")(e.target.value)}
                    autoComplete="name"
                  />
                </Field>
              </div>
              <Field id="ownerPassword" label="Password" error={errors.ownerPassword}>
                <Input
                  id="ownerPassword"
                  type="password"
                  value={form.ownerPassword}
                  onChange={(e) => set("ownerPassword")(e.target.value)}
                  autoComplete="new-password"
                />
              </Field>
              <Field
                id="ownerPasswordConfirm"
                label="Confirm password"
                error={errors.ownerPasswordConfirm}
              >
                <Input
                  id="ownerPasswordConfirm"
                  type="password"
                  value={form.ownerPasswordConfirm}
                  onChange={(e) => set("ownerPasswordConfirm")(e.target.value)}
                  autoComplete="new-password"
                />
              </Field>
            </>
          )}
        </div>

        <div className="mt-6 flex items-center justify-between">
          <Button variant="ghost" onClick={back} disabled={step === 0 || submitting}>
            Back
          </Button>
          {step < STEPS.length - 1 ? (
            <Button onClick={next}>Continue</Button>
          ) : (
            <Button onClick={submit} disabled={submitting}>
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Setting up…
                </>
              ) : (
                "Finish setup"
              )}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
