"use client";

import * as React from "react";
import {
  Boxes,
  ClipboardCheck,
  FileText,
  LayoutDashboard,
  Lock,
  LogOut,
  Menu,
  Package,
  PackageCheck,
  ReceiptText,
  Settings,
  ShieldCheck,
  ShoppingCart,
  Truck,
  Users,
  Wallet,
  X,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useSession } from "@/components/session/session-provider";
import type { ShellView } from "@/lib/shell";
import { DashboardView } from "@/components/dashboard/dashboard-view";
import { QuickAddPalette } from "@/components/shell/quick-add";
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

const NAV_SECTIONS: { label: string; items: NavItem[] }[] = [
  {
    label: "Overview",
    items: [{ id: "dashboard", label: "Dashboard", icon: LayoutDashboard, view: "dashboard" }],
  },
  {
    label: "Catalogue",
    items: [{ id: "catalogue", label: "Catalogue", icon: Package, view: "catalogue" }],
  },
  {
    label: "Inventory",
    items: [{ id: "inventory", label: "Inventory", icon: Boxes, view: "inventory" }],
  },
  {
    label: "Administration",
    items: [
      { id: "users", label: "Users", icon: Users, view: "users", permission: "user.manage" },
      {
        id: "roles",
        label: "Roles & permissions",
        icon: ShieldCheck,
        view: "roles",
        permission: "user.manage",
      },
      {
        id: "audit",
        label: "Audit log",
        icon: ClipboardCheck,
        view: "audit",
        permission: "audit.view",
      },
      { id: "settings", label: "Settings", icon: Settings, view: "settings" },
    ],
  },
  {
    label: "Purchasing",
    items: [{ id: "purchases", label: "Purchases", icon: Truck, view: "purchases" }],
  },
  {
    label: "Operations",
    items: [
      { id: "sales", label: "Sales", icon: ShoppingCart, view: "sales" },
      { id: "fulfilment", label: "Fulfilment", icon: PackageCheck, view: "fulfilment" },
    ],
  },
  {
    label: "Finance",
    items: [
      { id: "finance", label: "Finance", icon: Wallet, view: "finance", permission: "expense.view" },
    ],
  },
  {
    label: "Office (coming soon)",
    items: [
      { id: "reports", label: "Reports", icon: FileText, view: "reports", permission: "reports.view" },
      { id: "invoices", label: "Invoices", icon: ReceiptText, phase: "P6" },
    ],
  },
];

const VIEW_TITLES: Record<ShellView, string> = {
  dashboard: "Dashboard",
  catalogue: "Catalogue",
  inventory: "Inventory",
  purchases: "Purchases",
  sales: "Sales",
  fulfilment: "Fulfilment",
  finance: "Finance",
  users: "Users",
  roles: "Roles & permissions",
  audit: "Audit log",
  reports: "Reports",
  settings: "Settings",
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
  closeMenu,
}: {
  current: ShellView;
  onNavigate: (view: ShellView) => void;
  closeMenu?: () => void;
}) {
  const { hasPermission } = useSession();

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-14 items-center gap-2 border-b border-white/10 px-4">
        <span className="flex h-7 w-7 items-center justify-center rounded bg-amber-accent text-sm font-bold text-white">
          FS
        </span>
        <div className="leading-tight">
          <p className="text-sm font-semibold text-white">Furniture Shop</p>
          <p className="text-[10px] text-white/50">Powered by EagleNest Creations</p>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-3 py-3" aria-label="Main navigation">
        {NAV_SECTIONS.map((section) => {
          const visible = section.items.filter((item) =>
            item.permission ? hasPermission(item.permission) : true,
          );
          if (visible.length === 0) return null;
          return (
            <div key={section.label} className="mb-4">
              <p className="px-2 pb-1.5 text-[10px] font-semibold uppercase tracking-wider text-white/40">
                {section.label}
              </p>
              <ul className="space-y-0.5">
                {visible.map((item) => {
                  const active = item.view === current;
                  return (
                    <li key={item.id}>
                      <button
                        type="button"
                        disabled={!item.view}
                        onClick={() => {
                          if (item.view) onNavigate(item.view);
                          closeMenu?.();
                        }}
                        className={cn(
                          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
                          active
                            ? "bg-white/10 text-white"
                            : item.view
                              ? "text-white/70 hover:bg-white/5 hover:text-white"
                              : "cursor-not-allowed text-white/35",
                        )}
                        aria-current={active ? "page" : undefined}
                        title={item.phase ? `Planned for ${item.phase}` : undefined}
                      >
                        <span className="flex items-center gap-2.5">
                          <item.icon className="h-4 w-4 shrink-0" />
                          {item.label}
                        </span>
                        {item.phase && (
                          <span className="text-[9px] font-medium text-white/40">
                            {item.phase}
                          </span>
                        )}
                      </button>
                    </li>
                  );
                })}
              </ul>
            </div>
          );
        })}
      </nav>

      <div className="border-t border-white/10 px-4 py-3">
        <p className="text-[10px] text-white/40">v0.1.0 · local database</p>
      </div>
    </div>
  );
}

export function AppShell() {
  const { profile, lock, logout, busy } = useSession();
  const [mobileOpen, setMobileOpen] = React.useState(false);
  const [view, setView] = React.useState<ShellView>("dashboard");
  const [quickAddOpen, setQuickAddOpen] = React.useState(false);
  const [searchOpen, setSearchOpen] = React.useState(false);

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

  const initial = (profile?.fullName || profile?.username || "?")
    .split(/\s+/)
    .slice(0, 2)
    .map((s) => s[0] ?? "")
    .join("")
    .toUpperCase();

  return (
    <div className="flex h-screen overflow-hidden bg-cream">
      {/* Desktop sidebar */}
      <aside className="hidden w-60 shrink-0 bg-forest-700 lg:block">
        <SidebarContent current={view} onNavigate={setView} />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-neutral-950/40 lg:hidden"
          onClick={() => setMobileOpen(false)}
        >
          <div
            className="relative h-full w-72 bg-forest-700 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => setMobileOpen(false)}
              className="absolute right-2 top-3 rounded p-1 text-white/70 hover:bg-white/10 hover:text-white"
              aria-label="Close menu"
            >
              <X className="h-5 w-5" />
            </button>
            <SidebarContent
              current={view}
              onNavigate={setView}
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
          <div className="flex items-center gap-2 text-xs text-neutral-500">
            <span className="font-medium text-neutral-800">Furniture Shop</span>
            <span>/</span>
            <span>{VIEW_TITLES[view]}</span>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="accent">Trial</Badge>
            <div className="hidden items-center gap-2 sm:flex">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-forest-100 text-xs font-semibold text-forest-700">
                {initial}
              </span>
              <span className="grid gap-0 leading-tight">
                <span className="text-sm font-medium text-neutral-800">
                  {profile?.fullName || profile?.username}
                </span>
                <span className="text-[10px] capitalize text-neutral-500">
                  {(profile?.roles ?? [])[0] ?? ""}
                </span>
              </span>
            </div>
            <Button variant="outline" size="icon" onClick={lock} disabled={busy} aria-label="Lock screen">
              <Lock className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" onClick={logout} disabled={busy} aria-label="Sign out">
              <LogOut className="h-4 w-4" />
            </Button>
          </div>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
          {view === "dashboard" && <DashboardView onNavigate={setView} />}
          {view === "catalogue" && <CataloguePage />}
          {view === "inventory" && <InventoryPage />}
          {view === "purchases" && <PurchasesPage />}
          {view === "sales" && <SalesPage />}
          {view === "fulfilment" && <FulfilmentPage />}
          {view === "finance" && <ExpensesPage />}
          {view === "reports" && <ReportsPage />}
          {view === "users" && <UserManagement />}
          {view === "roles" && <RoleManagement />}
          {view === "audit" && <AuditViewer />}
          {view === "settings" && <SettingsPage />}
        </main>
      </div>

      <QuickAddPalette
        open={quickAddOpen}
        onOpenChange={setQuickAddOpen}
        onNavigate={setView}
      />
      <SearchOverlay
        open={searchOpen}
        onOpenChange={setSearchOpen}
        onNavigate={setView}
      />
    </div>
  );
}