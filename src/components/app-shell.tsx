"use client";
/* eslint-disable @next/next/no-img-element -- local logo data URLs are supplied by the offline backend */

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Boxes,
  FileText,
  LayoutDashboard,
  Lock,
  LogOut,
  Menu,
  Package,
  PackageCheck,
  ReceiptText,
  Settings,
  ShoppingCart,
  Truck,
  Wallet,
  X,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { useSession } from "@/components/session/session-provider";
import type { ShellView } from "@/lib/shell";
import { DashboardView } from "@/components/dashboard/dashboard-view";
import { QuickAddPalette, type QuickAddForm } from "@/components/shell/quick-add";
import { SearchOverlay } from "@/components/shell/search-overlay";
import { UserManagement } from "@/components/users/user-management";
import { RoleManagement } from "@/components/roles/role-management";
import { AuditViewer } from "@/components/audit/audit-viewer";
import { SettingsPage } from "@/components/settings/settings-page";
import { CataloguePage } from "@/components/catalogue/catalogue-page";
import { InventoryPage } from "@/components/inventory/inventory-page";
import { PurchasesPage } from "@/components/purchases/purchases-page";
import { SalesPage } from "@/components/sales/sales-page";
import { FulfilmentPage } from "@/components/fulfilment/fulfilment-page";
import { ExpensesPage } from "@/components/expenses/expenses-page";
import { ReportsPage } from "@/components/reports/reports-page";
import { MaintenancePage } from "@/components/maintenance/maintenance-page";
import { InvoicesPage } from "@/components/invoices/invoices-page";
import { licenseStatus, settingsGet, shopLogoGet } from "@/lib/tauri/api";

export type { ShellView } from "@/lib/shell";

const IDLE_LOCK_MS = 10 * 60 * 1000; // 10 minutes without activity.

type NavItem = {
  id: string;
  label: string;
  icon: React.ElementType;
  view?: ShellView;
  permission?: string;
  phase?: string;
};

// Primary nav items shown in the scrollable middle area.
// Administration items (users, roles, audit, maintenance) are moved inside the
// Settings page; their ShellView routes remain for backward-compatibility.
const PRIMARY_NAV: NavItem[] = [
  { id: "dashboard",  label: "Dashboard",  icon: LayoutDashboard, view: "dashboard" },
  { id: "catalogue",  label: "Catalogue",  icon: Package,          view: "catalogue" },
  { id: "inventory",  label: "Inventory",  icon: Boxes,            view: "inventory" },
  { id: "purchases",  label: "Purchases",  icon: Truck,            view: "purchases" },
  { id: "sales",      label: "Sales",      icon: ShoppingCart,     view: "sales" },
  { id: "fulfilment", label: "Fulfilment", icon: PackageCheck,     view: "fulfilment" },
  { id: "finance",    label: "Expenses",   icon: Wallet,           view: "finance",   permission: "expense.view" },
  { id: "invoices",   label: "Invoices",   icon: ReceiptText,      view: "invoices",  permission: "invoice.print" },
  { id: "reports",    label: "Reports",    icon: FileText,         view: "reports",   permission: "report.export" },
];

const VIEW_TITLES: Record<ShellView, string> = {
  dashboard: "Dashboard",
  catalogue: "Catalogue",
  inventory: "Inventory",
  purchases: "Purchases",
  sales: "Sales",
  fulfilment: "Fulfilment",
  finance: "Expenses",
  users: "Users",
  roles: "Roles & permissions",
  audit: "Audit log",
  reports: "Reports",
  invoices: "Invoices",
  settings: "Settings",
  maintenance: "Backup & maintenance",
};

function useIdleLock(onIdle: () => void) {
  const timer = React.useRef<number | null>(null);

  const reset = React.useCallback(() => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(onIdle, IDLE_LOCK_MS);
  }, [onIdle]);

  React.useEffect(() => {
    reset();
    const events: (keyof WindowEventMap)[] = ["pointerdown", "keydown", "wheel", "touchstart"];
    events.forEach((ev) => window.addEventListener(ev, reset, { passive: true }));
    return () => {
      events.forEach((ev) => window.removeEventListener(ev, reset));
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, [reset]);
}

function SidebarContent({
  current,
  onNavigate,
  shopName,
  logo,
  closeMenu,
}: {
  current: ShellView;
  onNavigate: (view: ShellView) => void;
  shopName: string;
  logo: string | null;
  closeMenu?: () => void;
}) {
  const { hasPermission, profile, lock, logout, busy } = useSession();

  const initial = (profile?.fullName || profile?.username || "?")
    .split(/\s+/)
    .slice(0, 2)
    .map((s) => s[0] ?? "")
    .join("")
    .toUpperCase();

  const visibleNav = PRIMARY_NAV.filter((item) =>
    item.permission ? hasPermission(item.permission) : true,
  );

  const isSettingsActive =
    current === "settings" ||
    current === "users" ||
    current === "roles" ||
    current === "audit" ||
    current === "maintenance";

  function go(view: ShellView) {
    onNavigate(view);
    closeMenu?.();
  }

  return (
    // Dark navy sidebar: slate-900 background
    <div
      className="flex h-full flex-col"
      style={{ background: "#0f172a" }}
    >
      {/* ── Brand header ── */}
      <div
        className="flex h-14 shrink-0 items-center gap-2.5 px-4"
        style={{ borderBottom: "1px solid rgba(255,255,255,0.07)" }}
      >
        <span className="flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-white/10 text-sm font-bold text-white">
          {logo ? <img src={logo} alt="" className="h-full w-full object-contain" /> : "FS"}
        </span>
        <div className="min-w-0 leading-tight">
          <p className="truncate text-sm font-semibold text-white">{shopName}</p>
          <p className="truncate text-[10px]" style={{ color: "rgba(255,255,255,0.38)" }}>
            Powered by EagleNest Creations
          </p>
        </div>
      </div>

      {/* ── Scrollable primary nav ── */}
      <nav
        className="flex-1 overflow-y-auto px-3 py-3"
        aria-label="Main navigation"
        style={{ scrollbarWidth: "thin", scrollbarColor: "rgba(255,255,255,0.1) transparent" }}
      >
        <ul className="space-y-0.5">
          {visibleNav.map((item) => {
            const active = item.view === current;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => item.view && go(item.view)}
                  disabled={!item.view}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-sm font-medium transition-colors",
                  )}
                  style={{
                    background: active ? "#2563eb" : "transparent",
                    color: active ? "#fff" : "rgba(255,255,255,0.65)",
                  }}
                  onMouseEnter={(e) => {
                    if (!active)
                      (e.currentTarget as HTMLButtonElement).style.background =
                        "rgba(255,255,255,0.07)";
                  }}
                  onMouseLeave={(e) => {
                    if (!active)
                      (e.currentTarget as HTMLButtonElement).style.background = "transparent";
                  }}
                >
                  <item.icon className="h-4 w-4 shrink-0" />
                  <span>{item.label}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      {/* ── Fixed bottom: Settings + user block ── */}
      <div
        className="shrink-0 px-3 pb-3 pt-2"
        style={{ borderTop: "1px solid rgba(255,255,255,0.07)" }}
      >
        {/* Settings */}
        <button
          type="button"
          onClick={() => go("settings")}
          aria-current={isSettingsActive ? "page" : undefined}
          className="mb-1 flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-sm font-medium transition-colors"
          style={{
            background: isSettingsActive ? "#2563eb" : "transparent",
            color: isSettingsActive ? "#fff" : "rgba(255,255,255,0.65)",
          }}
          onMouseEnter={(e) => {
            if (!isSettingsActive)
              (e.currentTarget as HTMLButtonElement).style.background =
                "rgba(255,255,255,0.07)";
          }}
          onMouseLeave={(e) => {
            if (!isSettingsActive)
              (e.currentTarget as HTMLButtonElement).style.background = "transparent";
          }}
        >
          <Settings className="h-4 w-4 shrink-0" />
          <span>Settings</span>
        </button>

        {/* User block */}
        <div
          className="flex items-center gap-2.5 rounded-md px-2.5 py-2"
          style={{ marginTop: 2 }}
        >
          <span
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs font-bold text-white"
            style={{ background: "#1e40af" }}
          >
            {initial}
          </span>
          <div className="min-w-0 flex-1 leading-tight">
            <p className="truncate text-sm font-medium text-white">
              {profile?.fullName || profile?.username}
            </p>
            <p
              className="truncate text-[10px] capitalize"
              style={{ color: "rgba(255,255,255,0.45)" }}
            >
              {(profile?.roles ?? [])[0] ?? ""}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-0.5">
            <button
              type="button"
              onClick={() => void lock()}
              disabled={busy}
              aria-label="Lock screen"
              className="rounded p-1 transition-colors"
              style={{ color: "rgba(255,255,255,0.45)" }}
              onMouseEnter={(e) =>
                ((e.currentTarget as HTMLButtonElement).style.color = "#fff")
              }
              onMouseLeave={(e) =>
                ((e.currentTarget as HTMLButtonElement).style.color =
                  "rgba(255,255,255,0.45)")
              }
            >
              <Lock className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => void logout()}
              disabled={busy}
              aria-label="Sign out"
              className="rounded p-1 transition-colors"
              style={{ color: "rgba(255,255,255,0.45)" }}
              onMouseEnter={(e) =>
                ((e.currentTarget as HTMLButtonElement).style.color = "#fff")
              }
              onMouseLeave={(e) =>
                ((e.currentTarget as HTMLButtonElement).style.color =
                  "rgba(255,255,255,0.45)")
              }
            >
              <LogOut className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export function AppShell() {
  const { lock, profile } = useSession();
  const session = profile?.sessionId ?? "";
  const [mobileOpen, setMobileOpen] = React.useState(false);
  const [view, setView] = React.useState<ShellView>("dashboard");
  const navigate = React.useCallback((next: ShellView) => {
    if (next === view) return;
    const event = new CustomEvent("furniture-before-view-change", { cancelable: true });
    if (window.dispatchEvent(event)) setView(next);
  }, [view]);
  const [quickAddOpen, setQuickAddOpen] = React.useState(false);
  const [quickAddInitialForm, setQuickAddInitialForm] = React.useState<QuickAddForm>(null);
  const [searchOpen, setSearchOpen] = React.useState(false);
  const shopNameQuery = useQuery({
    queryKey: ["branding", "shop-name"],
    queryFn: () => settingsGet(session, "shop.name"),
    enabled: Boolean(session),
  });
  const logoQuery = useQuery({
    queryKey: ["branding", "logo"],
    queryFn: () => shopLogoGet(session),
    enabled: Boolean(session),
  });
  const licenseQuery = useQuery({ queryKey: ["license-status"], queryFn: licenseStatus });
  let shopName = "Furniture Showroom";
  if (shopNameQuery.data) {
    try {
      const parsed = JSON.parse(shopNameQuery.data) as unknown;
      if (typeof parsed === "string" && parsed.trim()) shopName = parsed;
    } catch {
      // Retain the safe fallback for malformed legacy settings.
    }
  }

  const onIdle = React.useCallback(() => {
    void lock();
  }, [lock]);
  useIdleLock(onIdle);

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      const key = e.key.toLowerCase();
      if (e.shiftKey && key === "a") {
        e.preventDefault();
        setSearchOpen(false);
        setQuickAddOpen((o) => !o);
      } else if (key === "k") {
        e.preventDefault();
        setQuickAddOpen(false);
        setSearchOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-screen overflow-hidden bg-cream">
      {/* Desktop sidebar — dark navy */}
      <aside className="hidden w-56 shrink-0 lg:block" style={{ background: "#0f172a" }}>
        <SidebarContent current={view} onNavigate={navigate} shopName={shopName} logo={logoQuery.data ?? null} />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-neutral-950/40 lg:hidden"
          onClick={() => setMobileOpen(false)}
        >
          <div
            className="relative h-full w-72 shadow-xl"
            style={{ background: "#0f172a" }}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => setMobileOpen(false)}
              className="absolute right-2 top-3 rounded p-1 hover:bg-white/10"
              style={{ color: "rgba(255,255,255,0.6)" }}
              aria-label="Close menu"
            >
              <X className="h-5 w-5" />
            </button>
            <SidebarContent
              current={view}
              onNavigate={navigate}
              shopName={shopName}
              logo={logoQuery.data ?? null}
              closeMenu={() => setMobileOpen(false)}
            />
          </div>
        </div>
      )}

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center gap-3 border-b border-neutral-200 bg-white px-4">
          <button
            type="button"
            onClick={() => setMobileOpen(true)}
            className="rounded-md p-1.5 text-neutral-600 hover:bg-neutral-100 lg:hidden"
            aria-label="Open menu"
          >
            <Menu className="h-5 w-5" />
          </button>
          {/* Top bar — simplified: lock/logout moved into sidebar user block */}
          <div className="flex items-center gap-2 text-xs text-neutral-500">
            <span className="font-medium text-neutral-800">{shopName}</span>
            <span>/</span>
            <span>{VIEW_TITLES[view]}</span>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant={licenseQuery.data?.isActivated ? "success" : "neutral"}>
              {licenseQuery.data?.label ?? "License status unavailable"}
            </Badge>
          </div>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
          {view === "dashboard" && (
            <DashboardView
              onNavigate={navigate}
              onReceivePayment={() => {
                setQuickAddInitialForm("receipt");
                setQuickAddOpen(true);
              }}
            />
          )}
          {view === "catalogue" && <CataloguePage />}
          {view === "inventory" && <InventoryPage />}
          {view === "purchases" && <PurchasesPage />}
          {view === "sales" && <SalesPage />}
          {view === "fulfilment" && <FulfilmentPage />}
          {view === "finance" && <ExpensesPage />}
            {view === "reports" && <ReportsPage />}
          {view === "invoices" && <InvoicesPage />}
          {view === "users" && <UserManagement />}
          {view === "roles" && <RoleManagement />}
          {view === "audit" && <AuditViewer />}
          {view === "settings" && <SettingsPage />}
          {view === "maintenance" && <MaintenancePage />}
        </main>
      </div>

      <QuickAddPalette
        open={quickAddOpen}
        onOpenChange={(open) => {
          setQuickAddOpen(open);
          if (!open) setQuickAddInitialForm(null);
        }}
        onNavigate={navigate}
        initialForm={quickAddInitialForm}
      />
      <SearchOverlay
        open={searchOpen}
        onOpenChange={setSearchOpen}
        onNavigate={navigate}
      />
    </div>
  );
}
