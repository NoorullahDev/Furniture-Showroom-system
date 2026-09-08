"use client";

import * as React from "react";
import {
  Boxes,
  FileText,
  LayoutDashboard,
  Menu,
  Package,
  ReceiptText,
  Settings,
  ShoppingCart,
  Truck,
  Users,
  X,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";

const NAV_SECTIONS: {
  label: string;
  items: {
    id: string;
    label: string;
    icon: React.ElementType;
    phase: string;
    active?: boolean;
  }[];
}[] = [
  {
    label: "Overview",
    items: [
      { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, phase: "P1", active: true },
    ],
  },
  {
    label: "Operations",
    items: [
      { id: "sales", label: "Sales", icon: ShoppingCart, phase: "P6" },
      { id: "catalogue", label: "Catalogue", icon: Package, phase: "P3" },
      { id: "inventory", label: "Inventory", icon: Boxes, phase: "P4" },
      { id: "purchases", label: "Purchases", icon: Truck, phase: "P5" },
      { id: "customers", label: "Customers", icon: Users, phase: "P6" },
    ],
  },
  {
    label: "Office",
    items: [
      { id: "reports", label: "Reports", icon: FileText, phase: "P11" },
      { id: "invoices", label: "Invoices", icon: ReceiptText, phase: "P6" },
      { id: "settings", label: "Settings", icon: Settings, phase: "P2" },
    ],
  },
];

function SidebarContent() {
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
        {NAV_SECTIONS.map((section) => (
          <div key={section.label} className="mb-4">
            <p className="px-2 pb-1.5 text-[10px] font-semibold uppercase tracking-wider text-white/40">
              {section.label}
            </p>
            <ul className="space-y-0.5">
              {section.items.map((item) => (
                <li key={item.id}>
                  <button
                    type="button"
                    className={cn(
                      "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
                      item.active
                        ? "bg-white/10 text-white"
                        : "text-white/70 hover:bg-white/5 hover:text-white",
                    )}
                    aria-current={item.active ? "page" : undefined}
                  >
                    <span className="flex items-center gap-2.5">
                      <item.icon className="h-4 w-4 shrink-0" />
                      {item.label}
                    </span>
                    <span className="text-[9px] font-medium text-white/40">
                      {item.phase}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </nav>

      <div className="border-t border-white/10 px-4 py-3">
        <p className="text-[10px] text-white/40">
          v0.1.0 · local database
        </p>
      </div>
    </div>
  );
}

export function AppShell({
  children,
  topbarRight,
}: {
  children: React.ReactNode;
  topbarRight?: React.ReactNode;
}) {
  const [mobileOpen, setMobileOpen] = React.useState(false);

  return (
    <div className="flex h-screen overflow-hidden bg-cream">
      {/* Desktop sidebar */}
      <aside className="hidden w-60 shrink-0 bg-forest-700 lg:block">
        <SidebarContent />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-neutral-950/40 lg:hidden"
          onClick={() => setMobileOpen(false)}
        >
          <div
            className="h-full w-72 bg-forest-700 shadow-xl"
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
            <SidebarContent />
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
            <span>Dashboard</span>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="accent">Trial</Badge>
            <div className="hidden items-center gap-2 sm:flex">
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-forest-100 text-xs font-semibold text-forest-700">
                O
              </span>
              <span className="text-sm font-medium text-neutral-800">Owner</span>
            </div>
            {topbarRight}
          </div>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">{children}</main>
      </div>
    </div>
  );
}