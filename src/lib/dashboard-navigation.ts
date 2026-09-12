import type { ShellView } from "@/lib/shell";

export type DashboardTarget =
  | { view: "sales"; target: "new-sale" | "new-customer" | "sales-today" | "sales-month" | "payments-today" | "customer-dues"; shopDate?: string }
  | { view: "sales"; target: "sale"; id: number }
  | { view: "sales"; target: "payment"; customerId: number }
  | { view: "catalogue"; target: "new-product" }
  | { view: "finance"; target: "new-expense" }
  | { view: "purchases"; target: "payables" | "suppliers" }
  | { view: "fulfilment"; target: "deliveries" | "damage" | "returns" }
  | { view: "fulfilment"; target: "delivery"; id: number }
  | { view: "inventory"; target: "low-stock" };

const STORAGE_KEY = "furniture-dashboard-target";

export function setDashboardTarget(target: DashboardTarget): void {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(target));
  } catch {
    // Navigation still works even when session storage is unavailable.
  }
}

export function takeDashboardTarget<T extends ShellView>(view: T): Extract<DashboardTarget, { view: T }> | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as DashboardTarget;
    if (parsed.view !== view) return null;
    sessionStorage.removeItem(STORAGE_KEY);
    return parsed as Extract<DashboardTarget, { view: T }>;
  } catch {
    sessionStorage.removeItem(STORAGE_KEY);
    return null;
  }
}
