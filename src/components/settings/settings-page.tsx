"use client";
/* eslint-disable @next/next/no-img-element -- previews use a local logo data URL */

import * as React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Building2,
  CheckCircle2,
  Database,
  Eye,
  EyeOff,
  HardDrive,
  ImagePlus,
  KeyRound,
  Loader2,
  MonitorCog,
  Printer,
  Save,
  ShieldCheck,
  Trash2,
  Users,
} from "lucide-react";

import { AuditViewer } from "@/components/audit/audit-viewer";
import {
  loadPrintSettings,
  savePrintSettings,
  type FontSize,
  type InvoicePrintSettings,
  type PageOrientation,
  type PaperSize,
  type PrinterDestination,
} from "@/components/invoices/invoice-settings";
import { MaintenancePage } from "@/components/maintenance/maintenance-page";
import { HardwareId, LicenseKeyForm } from "@/components/license/license-gate";
import { PageHeader } from "@/components/page-header";
import { RoleManagement } from "@/components/roles/role-management";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
import { UserManagement } from "@/components/users/user-management";
import { cn } from "@/lib/utils";
import {
  authUpdateLoginDetails,
  licenseStatus,
  printerList,
  seedDemoData,
  settingsGet,
  settingsUpdateGeneral,
  settingsUpdatePrint,
  shopLogoGet,
  shopLogoRemove,
  shopLogoReplace,
  type LicenseStatus,
} from "@/lib/tauri/api";
import { commandErrorMessage, type CommandError } from "@/lib/tauri/client";

type SettingsSection = "general" | "print" | "backup" | "users" | "license" | "system";

const SECTIONS: { id: SettingsSection; label: string; icon: React.ElementType }[] = [
  { id: "general", label: "General", icon: Building2 },
  { id: "print", label: "Receipt & Print", icon: Printer },
  { id: "backup", label: "Backup & Restore", icon: HardDrive },
  { id: "users", label: "Users & Roles", icon: Users },
  { id: "license", label: "License", icon: ShieldCheck },
  { id: "system", label: "System", icon: MonitorCog },
];

const SETTING_KEYS = [
  "shop.name",
  "shop.owner_name",
  "shop.address",
  "shop.phone",
  "shop.currency",
  "shop.timezone",
  "invoice.prefix",
  "inventory.issue_policy",
  "inventory.negative_stock",
  "print.paper_size",
  "print.orientation",
  "print.margin_mm",
  "print.font_size",
  "print.copies",
  "print.show_logo",
  "print.show_address",
  "print.show_phone",
  "print.show_payment_details",
  "print.footer_text",
  "print.printer_destination",
] as const;

type SettingsRows = Record<string, string | null>;

function valueOf<T>(rows: SettingsRows | undefined, key: string, fallback: T): T {
  const raw = rows?.[key];
  if (raw == null) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function Card({
  title,
  description,
  children,
  action,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <section className="rounded-lg border border-neutral-200 bg-white shadow-sm">
      <div className="flex items-start justify-between gap-4 border-b border-neutral-200 px-5 py-4">
        <div>
          <h2 className="text-sm font-semibold text-neutral-900">{title}</h2>
          {description && <p className="mt-1 text-xs text-neutral-500">{description}</p>}
        </div>
        {action}
      </div>
      <div className="p-5">{children}</div>
    </section>
  );
}

export function SettingsPage() {
  const { profile, refresh } = useSession();
  const session = profile?.sessionId ?? "";
  const [section, setSection] = React.useState<SettingsSection>("general");
  const [dirty, setDirty] = React.useState(false);

  const settingsQuery = useQuery({
    queryKey: ["settings", "all"],
    queryFn: async () => {
      const entries = await Promise.all(
        SETTING_KEYS.map(async (key) => [key, await settingsGet(session, key)] as const),
      );
      return Object.fromEntries(entries) as SettingsRows;
    },
    enabled: Boolean(session),
  });
  const logoQuery = useQuery({
    queryKey: ["branding", "logo"],
    queryFn: () => shopLogoGet(session),
    enabled: Boolean(session),
  });
  const printersQuery = useQuery({
    queryKey: ["settings", "printers"],
    queryFn: () => printerList(session),
    enabled: Boolean(session) && section === "print",
  });
  const licenseQuery = useQuery({
    queryKey: ["license-status"],
    queryFn: licenseStatus,
  });

  React.useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (!dirty) return;
      event.preventDefault();
      event.returnValue = "";
    };
    const guardViewChange = (event: Event) => {
      if (dirty && !window.confirm("Discard unsaved settings changes?")) {
        event.preventDefault();
      }
    };
    window.addEventListener("beforeunload", warn);
    window.addEventListener("furniture-before-view-change", guardViewChange);
    return () => {
      window.removeEventListener("beforeunload", warn);
      window.removeEventListener("furniture-before-view-change", guardViewChange);
    };
  }, [dirty]);

  React.useEffect(() => {
    if (settingsQuery.isError && isSessionError(settingsQuery.error)) refresh();
  }, [settingsQuery.isError, settingsQuery.error, refresh]);

  function selectSection(next: SettingsSection) {
    if (next === section) return;
    if (dirty && !window.confirm("Discard unsaved settings changes?")) return;
    setDirty(false);
    setSection(next);
  }

  const common = {
    rows: settingsQuery.data,
    loading: settingsQuery.isLoading,
    logo: logoQuery.data ?? null,
    onDirtyChange: setDirty,
  };

  return (
    <div>
      <PageHeader
        title="Settings"
        subtitle="Business information, A4 printing, users, security, and system tools."
      />
      <div className="mt-5 grid gap-5 lg:grid-cols-[220px_minmax(0,1fr)]">
        <nav
          aria-label="Settings sections"
          className="h-fit rounded-lg border border-neutral-200 bg-white p-1.5 shadow-sm"
        >
          {SECTIONS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => selectSection(item.id)}
              aria-current={section === item.id ? "page" : undefined}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-md px-3 py-2.5 text-left text-sm font-medium transition-colors",
                section === item.id
                  ? "bg-forest-50 text-forest-700"
                  : "text-neutral-600 hover:bg-neutral-50 hover:text-neutral-900",
              )}
            >
              <item.icon className="h-4 w-4" aria-hidden="true" />
              {item.label}
            </button>
          ))}
        </nav>

        <div className="min-w-0">
          {settingsQuery.isError && (
            <p role="alert" className="mb-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">
              {commandErrorMessage(settingsQuery.error)}
            </p>
          )}
          {section === "general" && <GeneralPanel {...common} />}
          {section === "print" && (
            <PrintPanel {...common} printers={printersQuery.data ?? []} />
          )}
          {section === "backup" && <MaintenancePage />}
          {section === "users" && <UsersRolesPanel onDirtyChange={setDirty} />}
          {section === "license" && (
            <LicensePanel status={licenseQuery.data} loading={licenseQuery.isLoading} />
          )}
          {section === "system" && (
            <SystemPanel rows={settingsQuery.data} onOpenBackup={() => selectSection("backup")} />
          )}
        </div>
      </div>
    </div>
  );
}

function GeneralPanel({
  rows,
  loading,
  logo,
  onDirtyChange,
}: {
  rows?: SettingsRows;
  loading: boolean;
  logo: string | null;
  onDirtyChange: (dirty: boolean) => void;
}) {
  const { profile, hasPermission, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const canSave = hasPermission("settings.manage");
  const [form, setForm] = React.useState({
    shopName: "",
    ownerName: "",
    address: "",
    phone: "",
    currency: "PKR",
  });
  const [initial, setInitial] = React.useState(form);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!rows) return;
    const next = {
      shopName: valueOf(rows, "shop.name", "Furniture Showroom"),
      ownerName: valueOf(rows, "shop.owner_name", profile?.fullName ?? ""),
      address: valueOf(rows, "shop.address", ""),
      phone: valueOf(rows, "shop.phone", ""),
      currency: valueOf(rows, "shop.currency", "PKR"),
    };
    setForm(next);
    setInitial(next);
  }, [rows, profile?.fullName]);

  const dirty = JSON.stringify(form) !== JSON.stringify(initial);
  React.useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);

  const saveMutation = useMutation({
    mutationFn: () => settingsUpdateGeneral(profile?.sessionId ?? "", form),
    onSuccess: async () => {
      setInitial(form);
      onDirtyChange(false);
      setError(null);
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      await queryClient.invalidateQueries({ queryKey: ["branding"] });
      await queryClient.invalidateQueries({
        predicate: (query) => query.queryKey.includes("locations"),
      });
      window.dispatchEvent(new CustomEvent("furniture-branding-changed"));
      toast({ variant: "success", title: "General settings saved" });
    },
    onError: (caught: unknown) => {
      if (isSessionError(caught)) return refresh();
      const message = commandErrorMessage(caught);
      setError(message);
      toast({ variant: "error", title: "Could not save settings", description: message });
    },
  });

  function update(key: keyof typeof form, value: string) {
    setForm((current) => ({ ...current, [key]: value }));
    setError(null);
  }

  function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!form.shopName.trim()) return setError("Showroom name is required.");
    if (!/^[A-Za-z]{3}$/.test(form.currency)) {
      return setError("Currency must be a three-letter code such as PKR.");
    }
    saveMutation.mutate();
  }

  return (
    <form onSubmit={submit} className="space-y-5">
      <Card
        title="General"
        description="Showroom identity used by the application and newly generated documents."
        action={
          <Button type="submit" size="sm" disabled={!canSave || !dirty || saveMutation.isPending}>
            {saveMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
            Save changes
          </Button>
        }
      >
        {loading ? (
          <Loader2 className="h-5 w-5 animate-spin text-neutral-400" />
        ) : (
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Showroom / business name" id="general-name">
              <Input id="general-name" value={form.shopName} onChange={(e) => update("shopName", e.target.value)} disabled={!canSave} />
            </Field>
            <Field label="Owner name" id="general-owner">
              <Input id="general-owner" value={form.ownerName} onChange={(e) => update("ownerName", e.target.value)} disabled={!canSave} />
            </Field>
            <Field label="Address" id="general-address" className="sm:col-span-2">
              <Input id="general-address" value={form.address} onChange={(e) => update("address", e.target.value)} disabled={!canSave} />
            </Field>
            <Field label="Phone number" id="general-phone">
              <Input id="general-phone" value={form.phone} onChange={(e) => update("phone", e.target.value)} disabled={!canSave} />
            </Field>
            <Field label="Currency" id="general-currency" hint="Defaults to PKR. It cannot change after financial activity exists.">
              <Input id="general-currency" value={form.currency} maxLength={3} onChange={(e) => update("currency", e.target.value.toUpperCase())} disabled={!canSave} />
            </Field>
          </div>
        )}
        {error && <p role="alert" className="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">{error}</p>}
      </Card>
      <LogoControl logo={logo} canEdit={canSave} />
    </form>
  );
}

function LogoControl({ logo, canEdit }: { logo: string | null; canEdit: boolean }) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const replaceMutation = useMutation({
    mutationFn: async () => {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
      });
      if (!selected || Array.isArray(selected)) return null;
      return shopLogoReplace(profile?.sessionId ?? "", selected);
    },
    onSuccess: async (result) => {
      if (!result) return;
      await queryClient.invalidateQueries({ queryKey: ["branding"] });
      window.dispatchEvent(new CustomEvent("furniture-branding-changed"));
      toast({ variant: "success", title: "Shop logo updated" });
    },
    onError: (caught: unknown) => {
      if (isSessionError(caught)) return refresh();
      toast({ variant: "error", title: "Logo upload failed", description: commandErrorMessage(caught) });
    },
  });
  const removeMutation = useMutation({
    mutationFn: () => shopLogoRemove(profile?.sessionId ?? ""),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["branding"] });
      window.dispatchEvent(new CustomEvent("furniture-branding-changed"));
      toast({ variant: "success", title: "Shop logo removed" });
    },
    onError: (caught: unknown) => {
      if (isSessionError(caught)) return refresh();
      toast({ variant: "error", title: "Logo removal failed", description: commandErrorMessage(caught) });
    },
  });
  const busy = replaceMutation.isPending || removeMutation.isPending;

  return (
    <Card title="Shop logo" description="PNG, JPEG, or WebP. The same logo is used in General and Receipt & Print.">
      <div className="flex flex-wrap items-center gap-4">
        <div className="flex h-24 w-24 items-center justify-center overflow-hidden rounded-lg border border-dashed border-neutral-300 bg-neutral-50">
          {logo ? <img src={logo} alt="Current shop logo" className="h-full w-full object-contain p-2" /> : <ImagePlus className="h-8 w-8 text-neutral-300" aria-hidden="true" />}
        </div>
        <div className="space-y-2">
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="outline" size="sm" disabled={!canEdit || busy} onClick={() => replaceMutation.mutate()}>
              {replaceMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <ImagePlus className="h-4 w-4" />}
              {logo ? "Change logo" : "Upload logo"}
            </Button>
            {logo && (
              <Button type="button" variant="ghost" size="sm" disabled={!canEdit || busy} onClick={() => removeMutation.mutate()} className="text-red-600">
                <Trash2 className="h-4 w-4" /> Remove
              </Button>
            )}
          </div>
          {!canEdit && <p className="text-xs text-neutral-500">Requires settings management permission.</p>}
        </div>
      </div>
    </Card>
  );
}

function PrintPanel({
  rows,
  loading,
  logo,
  printers,
  onDirtyChange,
}: {
  rows?: SettingsRows;
  loading: boolean;
  logo: string | null;
  printers: { name: string; isDefault: boolean }[];
  onDirtyChange: (dirty: boolean) => void;
}) {
  const { profile, hasPermission, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const canSave = hasPermission("settings.manage");
  const [form, setForm] = React.useState<InvoicePrintSettings>(() => loadPrintSettings());
  const [initial, setInitial] = React.useState(form);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!rows) return;
    const local = loadPrintSettings();
    const next: InvoicePrintSettings = {
      paperSize: "a4",
      orientation: valueOf(rows, "print.orientation", local.orientation) as PageOrientation,
      marginMm: valueOf(rows, "print.margin_mm", local.marginMm),
      fontSize: valueOf(rows, "print.font_size", local.fontSize) as FontSize,
      copies: valueOf(rows, "print.copies", local.copies),
      showLogo: valueOf(rows, "print.show_logo", local.showLogo),
      showAddress: valueOf(rows, "print.show_address", local.showAddress),
      showPhone: valueOf(rows, "print.show_phone", local.showPhone),
      showPaymentDetails: valueOf(rows, "print.show_payment_details", local.showPaymentDetails),
      footerText: valueOf(rows, "print.footer_text", local.footerText),
      printerDestination: valueOf(rows, "print.printer_destination", local.printerDestination) as PrinterDestination,
    };
    setForm(next);
    setInitial(next);
    savePrintSettings(next);
  }, [rows]);

  const dirty = JSON.stringify(form) !== JSON.stringify(initial);
  React.useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);
  const update = <K extends keyof InvoicePrintSettings>(key: K, value: InvoicePrintSettings[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
    setError(null);
  };

  const saveMutation = useMutation({
    mutationFn: () => settingsUpdatePrint(profile?.sessionId ?? "", form),
    onSuccess: async () => {
      savePrintSettings(form);
      setInitial(form);
      onDirtyChange(false);
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast({ variant: "success", title: "Receipt and print settings saved" });
    },
    onError: (caught: unknown) => {
      if (isSessionError(caught)) return refresh();
      const message = commandErrorMessage(caught);
      setError(message);
      toast({ variant: "error", title: "Could not save print settings", description: message });
    },
  });

  return (
    <div className="space-y-5">
      <Card
        title="Receipt & Print"
        description="Defaults for normal A4 invoices and payment receipts. Per-job changes do not overwrite these preferences."
        action={<Button size="sm" disabled={!canSave || !dirty || saveMutation.isPending} onClick={() => saveMutation.mutate()}>{saveMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}Save settings</Button>}
      >
        {loading ? <Loader2 className="h-5 w-5 animate-spin text-neutral-400" /> : (
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            <Field label="Printer destination" id="print-destination" hint="Open in Print Window is safest and never prints automatically.">
              <Select value={form.printerDestination} onValueChange={(value) => update("printerDestination", value as PrinterDestination)} disabled={!canSave}>
                <SelectTrigger id="print-destination"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="print-window">Open in Print Window</SelectItem>
                  <SelectItem value="system-default">System Default Printer</SelectItem>
                  {printers.map((printer) => <SelectItem key={printer.name} value={`printer:${printer.name}`}>{printer.name}{printer.isDefault ? " (default)" : ""}</SelectItem>)}
                </SelectContent>
              </Select>
            </Field>
            <Field label="Paper size" id="print-paper" hint="A4: 210 × 297 mm">
              <Select value={form.paperSize} onValueChange={(value) => update("paperSize", value as PaperSize)} disabled={!canSave}>
                <SelectTrigger id="print-paper"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="a4">A4 — 210 × 297 mm</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label="Orientation" id="print-orientation">
              <Select value={form.orientation} onValueChange={(value) => update("orientation", value as PageOrientation)} disabled={!canSave}>
                <SelectTrigger id="print-orientation"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="portrait">Portrait</SelectItem><SelectItem value="landscape">Landscape</SelectItem></SelectContent>
              </Select>
            </Field>
            <Field label="Margins (mm)" id="print-margin" hint="Between 5 and 40 mm.">
              <Input id="print-margin" type="number" min={5} max={40} value={form.marginMm} onChange={(e) => update("marginMm", Math.max(5, Math.min(40, Number(e.target.value) || 15)))} disabled={!canSave} />
            </Field>
            <Field label="Font size" id="print-font">
              <Select value={form.fontSize} onValueChange={(value) => update("fontSize", value as FontSize)} disabled={!canSave}>
                <SelectTrigger id="print-font"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="small">Small — 10 pt</SelectItem><SelectItem value="normal">Normal — 11 pt</SelectItem><SelectItem value="large">Large — 12 pt</SelectItem></SelectContent>
              </Select>
            </Field>
            <Field label="Default copies" id="print-copies" hint="Applied once; the OS dialog copy count should remain 1.">
              <Input id="print-copies" type="number" min={1} max={10} value={form.copies} onChange={(e) => update("copies", Math.max(1, Math.min(10, Number(e.target.value) || 1)))} disabled={!canSave} />
            </Field>
          </div>
        )}

        <div className="mt-6 grid gap-5 sm:grid-cols-2">
          <fieldset className="space-y-3 rounded-md border border-neutral-200 p-4">
            <legend className="px-1 text-xs font-semibold uppercase tracking-wide text-neutral-500">Document content</legend>
            <Check label="Show logo" checked={form.showLogo} onChange={(value) => update("showLogo", value)} disabled={!canSave} />
            <Check label="Show address" checked={form.showAddress} onChange={(value) => update("showAddress", value)} disabled={!canSave} />
            <Check label="Show phone" checked={form.showPhone} onChange={(value) => update("showPhone", value)} disabled={!canSave} />
            <Check label="Show payment details" checked={form.showPaymentDetails} onChange={(value) => update("showPaymentDetails", value)} disabled={!canSave} />
          </fieldset>
          <Field label="Footer message" id="print-footer" hint={`${form.footerText.length}/300 characters`}>
            <textarea id="print-footer" rows={5} maxLength={300} value={form.footerText} onChange={(e) => update("footerText", e.target.value)} disabled={!canSave} className="w-full rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-forest-500 focus:ring-2 focus:ring-forest-500/20 disabled:bg-neutral-50" />
          </Field>
        </div>
        {error && <p role="alert" className="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">{error}</p>}
      </Card>
      <LogoControl logo={logo} canEdit={canSave} />
      <Card title="A4 print behavior" description="Print opens a preview first. Transactions remain completed if printing is cancelled or fails.">
        <div className="flex items-start gap-3 rounded-md bg-blue-50 p-3 text-sm text-blue-800">
          <Printer className="mt-0.5 h-4 w-4 shrink-0" />
          In “Open in Print Window” mode, the application creates a read-only preview and waits for you to choose the printer and confirm. It never prints automatically.
        </div>
      </Card>
    </div>
  );
}

function UsersRolesPanel({ onDirtyChange }: { onDirtyChange: (dirty: boolean) => void }) {
  const { profile, hasPermission } = useSession();
  const isOwner = profile?.roles.includes("owner") ?? false;
  const canManageUsers = hasPermission("user.manage");
  return (
    <div className="space-y-6">
      {isOwner && <MyAccountPanel onDirtyChange={onDirtyChange} />}
      {canManageUsers ? (
        <>
          <UserManagement />
          <RoleManagement />
        </>
      ) : (
        <Card title="Users & Roles"><p className="text-sm text-neutral-500">You do not have permission to manage users or roles.</p></Card>
      )}
    </div>
  );
}

function MyAccountPanel({ onDirtyChange }: { onDirtyChange: (dirty: boolean) => void }) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const [username, setUsername] = React.useState(profile?.username ?? "");
  const [currentPassword, setCurrentPassword] = React.useState("");
  const [newPassword, setNewPassword] = React.useState("");
  const [confirmPassword, setConfirmPassword] = React.useState("");
  const [visible, setVisible] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const dirty = username.trim() !== (profile?.username ?? "") || Boolean(currentPassword || newPassword || confirmPassword);
  React.useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);
  const mutation = useMutation({
    mutationFn: () => authUpdateLoginDetails(profile?.sessionId ?? "", {
      currentPassword,
      newUsername: username.trim() === profile?.username ? null : username.trim(),
      newPassword: newPassword || null,
      confirmPassword: confirmPassword || null,
    }),
    onSuccess: () => {
      onDirtyChange(false);
      toast({ variant: "success", title: "Login details updated", description: "Please sign in again with your updated credentials." });
      refresh();
    },
    onError: (caught: unknown) => {
      const commandError = caught as CommandError;
      const message = commandError.code === "INVALID_CREDENTIALS"
        ? "The current password is incorrect."
        : commandErrorMessage(caught);
      setError(message);
    },
  });

  function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!username.trim()) return setError("Username is required.");
    if (!currentPassword) return setError("Enter your current password.");
    if (newPassword !== confirmPassword) return setError("New password and confirmation do not match.");
    if (username.trim() === profile?.username && !newPassword) return setError("Enter a new username, a new password, or both.");
    setError(null);
    mutation.mutate();
  }

  return (
    <Card title="My Account / Login Details" description="Changing either value signs this account out on every device. Other users remain signed in.">
      <form onSubmit={submit} className="grid gap-4 sm:grid-cols-2">
        <Field label="Username" id="account-username" className="sm:col-span-2">
          <Input id="account-username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" />
        </Field>
        <PasswordField id="account-current" label="Current password" value={currentPassword} onChange={setCurrentPassword} visible={visible} />
        <div className="hidden sm:block" />
        <PasswordField id="account-new" label="New password" value={newPassword} onChange={setNewPassword} visible={visible} hint="Leave both new-password fields empty to keep the current password." />
        <PasswordField id="account-confirm" label="Confirm new password" value={confirmPassword} onChange={setConfirmPassword} visible={visible} />
        <div className="flex items-center justify-between gap-3 sm:col-span-2">
          <Button type="button" variant="ghost" size="sm" onClick={() => setVisible((value) => !value)}>
            {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            {visible ? "Hide passwords" : "Show passwords"}
          </Button>
          <Button type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <KeyRound className="h-4 w-4" />}
            Save Changes
          </Button>
        </div>
        {error && <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700 sm:col-span-2">{error}</p>}
      </form>
    </Card>
  );
}

function LicensePanel({ status, loading }: { status?: LicenseStatus; loading: boolean }) {
  const [renewing, setRenewing] = React.useState(false);
  if (loading) return <Loader2 className="h-5 w-5 animate-spin text-neutral-400" />;
  const activated = status?.isActivated === true && status.status === "active";
  const formatDate = (value?: string | null) => value ? new Date(value).toLocaleString() : "—";
  return (
    <>
      <Card
        title="License"
        description="Offline license status verified by the secure desktop backend."
        action={<Button size="sm" onClick={() => setRenewing(true)}><KeyRound className="h-4 w-4" />Renew License</Button>}
      >
        <div className="flex items-center gap-3">
          {activated ? <CheckCircle2 className="h-7 w-7 text-emerald-600" /> : <ShieldCheck className="h-7 w-7 text-red-500" />}
          <div>
            <p className={cn("font-semibold", activated ? "text-emerald-700" : "text-red-700")}>{status?.label ?? "License status unavailable"}</p>
            <p className="text-xs text-neutral-500">{status?.message}</p>
          </div>
        </div>
        {status && <div className="mt-5"><HardwareId value={status.hardwareId} /></div>}
        {status && (
          <dl className="mt-5 grid gap-x-6 gap-y-3 border-t border-neutral-100 pt-4 text-sm sm:grid-cols-2">
            <Info label="Customer / Showroom" value={status.customer ?? "—"} />
            <Info label="License ID" value={status.licenseId ?? "—"} />
            <Info label="License Key" value={status.maskedKey ?? "—"} />
            <Info label="Issue Date" value={formatDate(status.issueDate)} />
            <Info label="Last Renewed" value={formatDate(status.lastRenewed)} />
            <Info label="Granted Days" value={status.grantedDays?.toString() ?? "—"} />
            <Info label="Expiry Date" value={formatDate(status.expiresAt)} />
            <Info label="Days Remaining" value={status.daysRemaining.toString()} />
          </dl>
        )}
        {status && (
          <div className="mt-5">
            <div className="mb-2 flex justify-between text-xs text-neutral-500"><span>Remaining validity</span><span>{status.validityPercent}%</span></div>
            <div className="h-2 overflow-hidden rounded-full bg-neutral-100" role="progressbar" aria-label="Remaining license validity" aria-valuemin={0} aria-valuemax={100} aria-valuenow={status.validityPercent}>
              <div className={cn("h-full rounded-full", activated ? "bg-emerald-500" : "bg-red-500")} style={{ width: `${status.validityPercent}%` }} />
            </div>
          </div>
        )}
      </Card>
      <Dialog open={renewing} onOpenChange={setRenewing}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Renew License</DialogTitle>
            <DialogDescription>Paste the newly issued offline license. It must match this computer&apos;s Hardware ID.</DialogDescription>
          </DialogHeader>
          {status && <HardwareId value={status.hardwareId} />}
          <LicenseKeyForm onActivated={() => setRenewing(false)} />
        </DialogContent>
      </Dialog>
    </>
  );
}

function SystemPanel({ rows, onOpenBackup }: { rows?: SettingsRows; onOpenBackup: () => void }) {
  const { profile, hasPermission } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const canSeed = hasPermission("settings.manage") && window.location.protocol === "http:";

  const seedMutation = useMutation({
    mutationFn: () => seedDemoData(profile!.sessionId),
    onSuccess: (result) => {
      toast({ title: "Demo data loaded", description: `${result.products} products, ${result.bundles} bundles, ${result.sales} sales, ${result.deliveries} deliveries, ${result.salesReturns} returns, and ${result.expenses} expenses seeded.` });
      queryClient.invalidateQueries();
    },
    onError: (caught: unknown) => {
      toast({ variant: "error", title: "Seed failed", description: commandErrorMessage(caught) });
    },
  });

  return (
    <div className="space-y-6">
      <Card title="System preferences" description="Existing operational settings remain unchanged in this task." action={<Button variant="outline" size="sm" onClick={onOpenBackup}><HardDrive className="h-4 w-4" />Maintenance tools</Button>}>
        <dl className="divide-y divide-neutral-100">
          <Info label="Timezone" value={valueOf(rows, "shop.timezone", "Asia/Karachi")} />
          <Info label="Invoice prefix" value={valueOf(rows, "invoice.prefix", "INV/")} />
          <Info label="Inventory issue policy" value={valueOf(rows, "inventory.issue_policy", "on_confirmation")} />
          <Info label="Negative stock" value={valueOf(rows, "inventory.negative_stock", "block")} />
        </dl>
      </Card>
      {canSeed && (
        <Card title="Demo data" description="Populate the catalogue, suppliers, customers, purchases, sales, deliveries, returns, bundles, and expenses with realistic sample data. Safe to run multiple times — existing records are preserved.">
          <Button
            variant="outline"
            size="sm"
            onClick={() => seedMutation.mutate()}
            disabled={seedMutation.isPending}
          >
            {seedMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Database className="h-4 w-4" />}
            Seed Demo Data
          </Button>
        </Card>
      )}
      {hasPermission("audit.view") ? <AuditViewer /> : <Card title="Audit Log"><p className="text-sm text-neutral-500">You do not have permission to view the audit log.</p></Card>}
    </div>
  );
}

function Field({ label, id, hint, className, children }: { label: string; id: string; hint?: string; className?: string; children: React.ReactNode }) {
  return <div className={cn("grid gap-1.5", className)}><Label htmlFor={id}>{label}</Label>{children}{hint && <p className="text-[11px] text-neutral-500">{hint}</p>}</div>;
}

function PasswordField({ id, label, value, onChange, visible, hint }: { id: string; label: string; value: string; onChange: (value: string) => void; visible: boolean; hint?: string }) {
  return <Field label={label} id={id} hint={hint}><Input id={id} type={visible ? "text" : "password"} value={value} onChange={(e) => onChange(e.target.value)} autoComplete={id.includes("current") ? "current-password" : "new-password"} /></Field>;
}

function Check({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean }) {
  return <label className="flex items-center gap-2 text-sm text-neutral-700"><input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} disabled={disabled} className="h-4 w-4 rounded border-neutral-300 text-forest-600 focus:ring-forest-500" />{label}</label>;
}

function Info({ label, value }: { label: string; value: string }) {
  return <div className="flex items-center justify-between gap-4 py-2.5"><dt className="text-sm text-neutral-500">{label}</dt><dd className="text-right text-sm font-medium text-neutral-900">{value || "—"}</dd></div>;
}
