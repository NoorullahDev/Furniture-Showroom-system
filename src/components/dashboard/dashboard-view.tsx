"use client";

import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Banknote,
  Boxes,
  Package,
  PackageCheck,
  RefreshCw,
  ShoppingCart,
  Truck,
  Wallet,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useSession } from "@/components/session/session-provider";
import {
  dashboardSummary,
  type DashboardTrendDayDto,
} from "@/lib/tauri/api";
import { asCommandError } from "@/lib/tauri/client";
import { formatDateTime, formatPkr } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ShellView } from "@/lib/shell";

function errorText(e: unknown): string {
  const err = asCommandError(e);
  return err.code
    ? `${err.code}${err.correlationId ? ` · ${err.correlationId}` : ""}: ${err.message}`
    : String(e);
}

function Panel({
  title,
  description,
  children,
  className,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("rounded-lg border bg-white p-5 shadow-sm", className)}>
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="text-base font-semibold text-neutral-900">{title}</h2>
        {description ? <p className="text-xs text-neutral-400">{description}</p> : null}
      </div>
      <div className="mt-4">{children}</div>
    </section>
  );
}

function SkeletonBlock({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-md bg-neutral-100", className)} />;
}

function KpiCard({
  label,
  value,
  sub,
  icon: Icon,
  onClick,
  tone,
}: {
  label: string;
  value: string;
  sub?: string;
  icon: React.ElementType;
  onClick: () => void;
  tone?: "danger" | "warning";
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-full w-full flex-col gap-1 rounded-lg border bg-white p-4 text-left shadow-sm transition-colors hover:border-forest-300 hover:bg-forest-50/60"
    >
      <span className="flex items-center gap-1.5 text-xs font-medium text-neutral-500">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </span>
      <span className="text-lg font-semibold tabular-nums text-neutral-900">{value}</span>
      {sub ? (
        <span
          className={cn(
            "text-xs tabular-nums",
            tone === "danger" ? "text-red-600" : tone === "warning" ? "text-amber-700" : "text-neutral-400",
          )}
        >
          {sub}
        </span>
      ) : null}
    </button>
  );
}

function MiniTrend({ trend }: { trend: DashboardTrendDayDto[] }) {
  const max = Math.max(
    1,
    ...trend.flatMap((t) => [t.salesMinor, t.receiptsMinor, t.expensesMinor]),
  );
  const chartTop = 16;
  const chartHeight = 78;
  const barW = 12;
  const groupX = (i: number) => 44 + i * 72;

  return (
    <div>
      <svg
        viewBox="0 0 560 122"
        className="h-auto w-full"
        role="img"
        aria-label="Seven day sales, receipts and expenses trend"
      >
        {[0.25, 0.5, 0.75, 1].map((f) => {
          const y = chartTop + chartHeight * (1 - f);
          return (
            <line
              key={f}
              x1={12}
              x2={548}
              y1={y}
              y2={y}
              stroke="#e5e7eb"
              strokeWidth={1}
              strokeDasharray="2 3"
            />
          );
        })}
        {trend.map((t, i) => {
          const cx = groupX(i);
          const h = (v: number) => (v / max) * chartHeight;
          const bars = [
            { v: t.salesMinor, color: "#065f46", key: "sales" },
            { v: t.receiptsMinor, color: "#0ea5e9", key: "receipts" },
            { v: t.expensesMinor, color: "#f59e0b", key: "expenses" },
          ] as const;
          return (
            <g key={t.day}>
              {bars.map((b, bIdx) => {
                const bh = h(Math.abs(b.v));
                const y = chartTop + chartHeight - bh;
                const x = cx + (bIdx - 1) * (barW + 4);
                return (
                  <rect
                    key={b.key}
                    x={x}
                    y={y}
                    width={barW}
                    height={Math.max(bh, b.v === 0 ? 1 : 0)}
                    rx={2}
                    fill={b.color}
                    opacity={b.v === 0 ? 0.15 : 1}
                  >
                    <title>{`${formatPkr(b.v)}`}</title>
                  </rect>
                );
              })}
              <text x={cx} y={116} textAnchor="middle" className="fill-neutral-400 text-[9px]">
                {new Date(`${t.day}T00:00:00`).toLocaleDateString("en-GB", {
                  weekday: "short",
                  day: "numeric",
                })}
              </text>
            </g>
          );
        })}
      </svg>
      <div className="mt-2 flex flex-wrap items-center gap-4 text-xs text-neutral-500">
        <span className="flex items-center gap-1.5">
          <span className="h-2.5 w-2.5 rounded-sm bg-forest-700" /> Sales
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-2.5 w-2.5 rounded-sm bg-sky-500" /> Receipts
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-2.5 w-2.5 rounded-sm bg-amber-500" /> Expenses
        </span>
      </div>
    </div>
  );
}

const ENTITY_LABELS: Record<string, string> = {
  product: "Product",
  customer: "Customer",
  supplier: "Supplier",
  sale: "Sale",
  sale_return: "Return",
  delivery: "Delivery",
  purchase: "Purchase",
  supplier_payment: "Supplier payment",
  customer_payment: "Receipt",
  expense: "Expense",
  damage: "Damage",
  credit_note: "Credit note",
  cash_entry: "Cash entry",
  user: "User",
  role: "Role",
  location: "Location",
  movement: "Stock movement",
  count_session: "Stock count",
  bundle: "Furniture set",
};

function ActivityRow({
  id,
  action,
  entityType,
  entityId,
  username,
  createdAt,
}: {
  id: number;
  action: string;
  entityType?: string | null;
  entityId?: string | null;
  username?: string | null;
  createdAt: string;
}) {
  const [entity, verb] = action.split(".");
  const entityLabel = (entity && ENTITY_LABELS[entity]) || entityType || action;
  const verbLabel = (verb || action).replace(/_/g, " ");

  return (
    <TableRow key={id}>
      <TableCell>
        <span className="font-medium text-neutral-800">{username || "System"}</span>
      </TableCell>
      <TableCell>
        <span className="capitalize">{verbLabel}</span>
        <Badge variant="neutral" className="ml-2">
          {entityLabel}
        </Badge>
      </TableCell>
      <TableCell className="font-mono text-xs text-neutral-500">
        {entityId && entityId !== "0" ? entityId : "—"}
      </TableCell>
      <TableCell className="text-right whitespace-nowrap text-neutral-500">
        {formatDateTime(createdAt)}
      </TableCell>
    </TableRow>
  );
}

export function DashboardView({ onNavigate }: { onNavigate: (view: ShellView) => void }) {
  const { profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const summaryQuery = useQuery({
    queryKey: ["dashboard", "summary"],
    queryFn: () => dashboardSummary(session),
    enabled: !!session,
  });

  const can = {
    sales: hasPermission("sale.create") || hasPermission("invoice.print"),
    receipts: hasPermission("payment.receive") || hasPermission("customer.view"),
    expenses: hasPermission("expense.view"),
    cash: hasPermission("payable.view"),
    stockValue: hasPermission("product.cost.view"),
    profit: hasPermission("profit.view"),
    deliveries: hasPermission("delivery.view"),
    damage: hasPermission("damage.record"),
  };

  const isError = summaryQuery.isError;
  const error = isError ? errorText(summaryQuery.error) : null;
  const data = summaryQuery.data;

  const isEmpty =
    data !== undefined &&
    data.todaySalesCount === 0 &&
    data.todaySalesMinor === 0 &&
    data.todayReceiptsMinor === 0 &&
    data.todayExpensesMinor === 0 &&
    data.netCashMinor === 0 &&
    data.duesMinor === 0 &&
    data.overdueDuesMinor === 0 &&
    data.payablesMinor === 0 &&
    data.lowStockCount === 0 &&
    data.pendingDeliveries === 0 &&
    data.openDamageCount === 0 &&
    data.monthRevenueMinor === 0 &&
    data.monthExpensesMinor === 0 &&
    data.recentActivity.length === 0 &&
    data.trend.every(
      (t) =>
        t.salesCount === 0 &&
        t.salesMinor === 0 &&
        t.receiptsMinor === 0 &&
        t.expensesMinor === 0,
    );

  const attention: {
    key: string;
    icon: React.ElementType;
    label: string;
    detail: string;
    tone: "danger" | "warning";
    view: ShellView;
    count: number;
    badge?: string;
  }[] = [];

  if (data) {
    if (data.lowStockCount > 0)
      attention.push({
        key: "low",
        icon: Boxes,
        label: "Low stock",
        detail: `${data.lowStockCount} product(s) at or below minimum`,
        tone: "warning",
        view: "inventory",
        count: data.lowStockCount,
      });
    if (data.pendingDeliveries > 0)
      attention.push({
        key: "pending",
        icon: Truck,
        label: "Pending deliveries",
        detail: `${data.pendingDeliveries} delivery(ies) awaiting dispatch or delivery`,
        tone: "warning",
        view: "fulfilment",
        count: data.pendingDeliveries,
      });
    if (data.overdueDuesMinor > 0)
      attention.push({
        key: "overdue",
        icon: AlertTriangle,
        label: "Overdue customer dues",
        detail: `${formatPkr(data.overdueDuesMinor)} past due across advised customers`,
        tone: "danger",
        view: "sales",
        count: data.overdueDuesMinor,
        badge: `${formatPkr(data.overdueDuesMinor)}`,
      });
    if (data.openDamageCount > 0)
      attention.push({
        key: "damage",
        icon: PackageCheck,
        label: "Open damage",
        detail: `${data.openDamageCount} damage record(s) awaiting decision`,
        tone: "danger",
        view: "fulfilment",
        count: data.openDamageCount,
      });
  }

  const handleRefresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
  };

  if (summaryQuery.isLoading) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Loading your daily command center…" />
        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
          {Array.from({ length: 6 }).map((_, i) => (
            <SkeletonBlock key={i} className="h-24" />
          ))}
        </div>
        <div className="mt-5 grid gap-5 lg:grid-cols-2">
          <SkeletonBlock className="h-64" />
          <SkeletonBlock className="h-64" />
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div>
        <PageHeader
          title="Dashboard"
          subtitle="Your command center could not be loaded."
          actions={
            <Button onClick={handleRefresh} variant="outline">
              <RefreshCw className="mr-2 h-4 w-4" /> Retry
            </Button>
          }
        />
        <div className="mt-5 rounded-lg border border-red-200 bg-red-50 p-5">
          <p className="text-sm font-medium text-red-800">Dashboard failed to load</p>
          <p className="mt-1 font-mono text-xs text-red-700">{error}</p>
          <p className="mt-2 text-xs text-red-600">
            Check your session and try again. If the problem persists, review the application logs.
          </p>
        </div>
      </div>
    );
  }

  if (!data) return null;

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle={
          data
            ? `Updated ${formatDateTime(data.asOf)} · daily, month and trend figures use document dates`
            : "Daily command center"
        }
        actions={
          <Button onClick={handleRefresh} variant="outline" aria-label="Refresh dashboard">
            <RefreshCw className="mr-2 h-4 w-4" /> Refresh
          </Button>
        }
      />

      {isEmpty && (
        <div className="mt-4 rounded-lg border border-forest-200 bg-forest-50 p-4">
          <p className="text-sm font-semibold text-forest-800">Welcome — let us set you up</p>
          <p className="mt-0.5 text-sm text-forest-700">
            The shop is empty. Work through the checklist below to start running daily operations.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button size="sm" onClick={() => onNavigate("catalogue")}>
              Add products
            </Button>
            <Button size="sm" variant="outline" onClick={() => onNavigate("inventory")}>
              Post opening stock
            </Button>
            <Button size="sm" variant="outline" onClick={() => onNavigate("sales")}>
              Make a sale
            </Button>
            <Button size="sm" variant="outline" onClick={() => onNavigate("purchases")}>
              Add a supplier
            </Button>
          </div>
        </div>
      )}

      <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
        {can.sales && (
          <KpiCard
            label="Sales today"
            value={formatPkr(data.todaySalesMinor)}
            sub={`${data.todaySalesCount} sale(s)`}
            icon={ShoppingCart}
            onClick={() => onNavigate("sales")}
          />
        )}
        {can.receipts && (
          <KpiCard
            label="Received today"
            value={formatPkr(data.todayReceiptsMinor)}
            icon={Banknote}
            onClick={() => onNavigate("sales")}
          />
        )}
        {can.expenses && (
          <KpiCard
            label="Expenses today"
            value={formatPkr(data.todayExpensesMinor)}
            icon={Wallet}
            onClick={() => onNavigate("finance")}
          />
        )}
        {can.cash && (
          <KpiCard
            label="Net cash"
            value={formatPkr(data.netCashMinor)}
            icon={Wallet}
            onClick={() => onNavigate("finance")}
          />
        )}
        {can.receipts && (
          <KpiCard
            label="Customer dues"
            value={formatPkr(data.duesMinor)}
            sub={
              data.overdueDuesMinor > 0 ? `${formatPkr(data.overdueDuesMinor)} overdue` : "all current"
            }
            tone={data.overdueDuesMinor > 0 ? "danger" : undefined}
            icon={AlertTriangle}
            onClick={() => onNavigate("sales")}
          />
        )}
        {can.cash && (
          <KpiCard
            label="Supplier payables"
            value={formatPkr(data.payablesMinor)}
            icon={Truck}
            onClick={() => onNavigate("purchases")}
          />
        )}
        {can.stockValue && data.stockValueMinor !== undefined && data.stockValueMinor !== null && (
          <KpiCard
            label="Stock value"
            value={formatPkr(data.stockValueMinor)}
            icon={Boxes}
            onClick={() => onNavigate("inventory")}
          />
        )}
        <KpiCard
          label="Low stock"
          value={String(data.lowStockCount)}
          sub={data.lowStockCount > 0 ? "below minimum" : "healthy"}
          tone={data.lowStockCount > 0 ? "warning" : undefined}
          icon={Boxes}
          onClick={() => onNavigate("inventory")}
        />
        {can.deliveries && (
          <KpiCard
            label="Pending deliveries"
            value={String(data.pendingDeliveries)}
            sub={data.pendingDeliveries > 0 ? "awaiting fulfilment" : "none scheduled"}
            tone={data.pendingDeliveries > 0 ? "warning" : undefined}
            icon={Truck}
            onClick={() => onNavigate("fulfilment")}
          />
        )}
        {can.damage && (
          <KpiCard
            label="Open damage"
            value={String(data.openDamageCount)}
            sub={data.openDamageCount > 0 ? "awaiting decision" : "none open"}
            tone={data.openDamageCount > 0 ? "danger" : undefined}
            icon={AlertTriangle}
            onClick={() => onNavigate("fulfilment")}
          />
        )}
        {can.sales && (
          <KpiCard
            label="Month revenue"
            value={formatPkr(data.monthRevenueMinor)}
            icon={ShoppingCart}
            onClick={() => onNavigate("sales")}
          />
        )}
        {can.profit && data.monthGrossProfitMinor !== undefined && data.monthGrossProfitMinor !== null && (
          <KpiCard
            label="Month gross profit"
            value={formatPkr(data.monthGrossProfitMinor)}
            icon={Package}
            onClick={() => onNavigate("sales")}
          />
        )}
      </div>

      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <Panel title="Seven-day trend" description="Sales, receipts and expenses by document date">
          {data.trend.length > 0 ? (
            <MiniTrend trend={data.trend} />
          ) : (
            <p className="text-sm text-neutral-500">No activity in the last seven days.</p>
          )}
        </Panel>

        <Panel title="Attention" description="Items that need your input today">
          {attention.length === 0 ? (
            <div className="flex items-center gap-2 text-sm text-neutral-500">
              <PackageCheck className="h-4 w-4 text-forest-600" />
              Nothing needs attention right now.
            </div>
          ) : (
            <ul className="divide-y divide-neutral-100">
              {attention.map((item) => (
                <li key={item.key}>
                  <button
                    type="button"
                    onClick={() => onNavigate(item.view)}
                    className="flex w-full items-center justify-between gap-3 rounded-md px-1 py-2.5 text-left transition-colors hover:bg-neutral-50"
                  >
                    <span className="flex items-center gap-3">
                      <span
                        className={cn(
                          "flex h-8 w-8 shrink-0 items-center justify-center rounded-md",
                          item.tone === "danger" ? "bg-red-100 text-red-700" : "bg-amber-100 text-amber-700",
                        )}
                      >
                        <item.icon className="h-4 w-4" />
                      </span>
                      <span className="grid gap-0.5">
                        <span className="text-sm font-medium text-neutral-800">{item.label}</span>
                        <span className="text-xs text-neutral-500">{item.detail}</span>
                      </span>
                    </span>
                    <Badge variant={item.tone === "danger" ? "danger" : "warning"}>
                      {item.badge ?? String(item.count)}
                    </Badge>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>

      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <Panel title="Recent activity" description="Latest audit events from your team">
          {data.recentActivity.length === 0 ? (
            <p className="text-sm text-neutral-500">No activity recorded yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>User</TableHead>
                  <TableHead>Action</TableHead>
                  <TableHead>Ref</TableHead>
                  <TableHead className="text-right">When</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.recentActivity.map((a) => (
                  <ActivityRow
                    key={a.id}
                    id={a.id}
                    action={a.action}
                    entityType={a.entityType}
                    entityId={a.entityId}
                    username={a.username}
                    createdAt={a.createdAt}
                  />
                ))}
              </TableBody>
            </Table>
          )}
        </Panel>

        <Panel title="Shortcuts" description="Jump into the modules you use daily">
          <div className="grid grid-cols-2 gap-3">
            <button
              type="button"
              onClick={() => onNavigate("catalogue")}
              className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
            >
              <Package className="h-4 w-4 text-forest-700" />
              <span className="text-sm font-medium text-neutral-800">Catalogue</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate("inventory")}
              className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
            >
              <Boxes className="h-4 w-4 text-forest-700" />
              <span className="text-sm font-medium text-neutral-800">Inventory</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate("sales")}
              className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
            >
              <ShoppingCart className="h-4 w-4 text-forest-700" />
              <span className="text-sm font-medium text-neutral-800">Sales</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate("purchases")}
              className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
            >
              <Truck className="h-4 w-4 text-forest-700" />
              <span className="text-sm font-medium text-neutral-800">Purchases</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate("fulfilment")}
              className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
            >
              <PackageCheck className="h-4 w-4 text-forest-700" />
              <span className="text-sm font-medium text-neutral-800">Fulfilment</span>
            </button>
            {can.expenses && (
              <button
                type="button"
                onClick={() => onNavigate("finance")}
                className="flex items-center gap-2 rounded-lg border p-3 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/60"
              >
                <Wallet className="h-4 w-4 text-forest-700" />
                <span className="text-sm font-medium text-neutral-800">Finance</span>
              </button>
            )}
          </div>
          <p className="mt-3 text-xs text-neutral-400">
            Press Ctrl+Shift+A for Quick Add and Ctrl+K for global search from anywhere.
          </p>
        </Panel>
      </div>
    </div>
  );
}