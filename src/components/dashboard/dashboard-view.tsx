"use client";

import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Banknote,
  BarChart3,
  PackageCheck,
  PackageOpen,
  RefreshCw,
  ShoppingCart,
  Truck,
  UserPlus,
  Users,
  Wallet,
} from "lucide-react";

import { StoredImage } from "@/components/catalogue/stored-image";
import { PageHeader } from "@/components/page-header";
import { useSession } from "@/components/session/session-provider";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { setDashboardTarget, type DashboardTarget } from "@/lib/dashboard-navigation";
import { formatDateTime, formatPkr } from "@/lib/format";
import type { ShellView } from "@/lib/shell";
import { dashboardSummary } from "@/lib/tauri/api";
import { asCommandError } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";

function errorText(error: unknown): string {
  const command = asCommandError(error);
  return command.code
    ? `${command.code}${command.correlationId ? ` · ${command.correlationId}` : ""}: ${command.message}`
    : String(error);
}

function displayDate(value: string): string {
  const parsed = new Date(`${value.slice(0, 10)}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

function Panel({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="min-w-0 overflow-hidden rounded-xl border border-slate-200 bg-white shadow-[0_1px_2px_rgba(15,23,42,0.04)]">
      <div className="flex min-h-12 items-center justify-between gap-3 border-b border-slate-100 px-4 py-2.5">
        <div className="min-w-0">
          <h2 className="text-sm font-semibold text-slate-900">{title}</h2>
          {description && <p className="mt-0.5 text-[11px] text-slate-500">{description}</p>}
        </div>
        {action}
      </div>
      {children}
    </section>
  );
}

function PanelLink({ children, onClick }: { children: React.ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="shrink-0 rounded-md px-2 py-1 text-xs font-medium text-blue-600 transition hover:bg-blue-50 hover:text-blue-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/40"
    >
      {children}
    </button>
  );
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-36 items-center justify-center px-5 py-8 text-center text-sm text-slate-500">
      {children}
    </div>
  );
}

function Skeleton({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-xl bg-slate-200/70", className)} />;
}

type MetricTone = "blue" | "green" | "violet" | "amber" | "rose" | "cyan";

const metricTones: Record<MetricTone, { card: string; icon: string }> = {
  blue: { card: "border-blue-200 bg-blue-50/80", icon: "bg-blue-100 text-blue-600" },
  green: { card: "border-emerald-200 bg-emerald-50/80", icon: "bg-emerald-100 text-emerald-700" },
  violet: { card: "border-violet-200 bg-violet-50/80", icon: "bg-violet-100 text-violet-600" },
  amber: { card: "border-amber-200 bg-amber-50/80", icon: "bg-amber-100 text-amber-700" },
  rose: { card: "border-rose-200 bg-rose-50/80", icon: "bg-rose-100 text-rose-700" },
  cyan: { card: "border-cyan-200 bg-cyan-50/80", icon: "bg-cyan-100 text-cyan-700" },
};

function MetricCard({
  label,
  value,
  detail,
  icon: Icon,
  allowed,
  onClick,
  tone,
  count = false,
}: {
  label: string;
  value?: number | null;
  detail: string;
  icon: React.ElementType;
  allowed: boolean;
  onClick: () => void;
  tone: MetricTone;
  count?: boolean;
}) {
  const colors = metricTones[tone];
  const displayValue = allowed && value != null
    ? count ? value.toLocaleString("en-PK") : formatPkr(value)
    : "Restricted";

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!allowed}
      className={cn(
        "group flex min-h-24 w-full items-start gap-4 rounded-xl border p-4 text-left transition duration-150",
        "hover:-translate-y-0.5 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/40",
        "disabled:cursor-not-allowed disabled:opacity-65 disabled:hover:translate-y-0 disabled:hover:shadow-none",
        colors.card,
      )}
    >
      <span className={cn("flex h-11 w-11 shrink-0 items-center justify-center rounded-lg", colors.icon)}>
        <Icon className="h-5 w-5" aria-hidden="true" />
      </span>
      <span className="min-w-0 pt-0.5">
        <span className="block text-xs font-medium text-slate-600">{label}</span>
        <span className="mt-0.5 block text-xl font-bold tabular-nums tracking-tight text-slate-950">
          {displayValue}
        </span>
        <span className="mt-0.5 block truncate text-[11px] text-slate-500">
          {allowed ? detail : "Permission required"}
        </span>
      </span>
    </button>
  );
}

function ClickableRow({
  children,
  onClick,
}: {
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <TableRow
      role="button"
      tabIndex={0}
      className="cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-inset"
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
    >
      {children}
    </TableRow>
  );
}

export function DashboardView({
  onNavigate,
  onReceivePayment,
}: {
  onNavigate: (view: ShellView) => void;
  onReceivePayment: () => void;
}) {
  const { profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";
  const summaryQuery = useQuery({
    queryKey: ["dashboard", "summary"],
    queryFn: () => dashboardSummary(session),
    enabled: Boolean(session),
    refetchOnMount: "always",
  });

  const permission = {
    sales: hasPermission("sale.create") || hasPermission("invoice.print"),
    newSale: hasPermission("sale.create"),
    addCustomer: hasPermission("customer.create"),
    addProduct: hasPermission("product.create"),
    addExpense: hasPermission("expense.create") && hasPermission("expense.view"),
    receipts: hasPermission("payment.receive") || hasPermission("customer.view"),
    receivePayment: hasPermission("payment.receive") && hasPermission("payable.view"),
    payables: hasPermission("payable.view"),
    deliveries: hasPermission("delivery.view"),
  };
  const data = summaryQuery.data;
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
  const open = (target: DashboardTarget, destination: ShellView = target.view) => {
    setDashboardTarget(target);
    onNavigate(destination);
  };

  if (summaryQuery.isLoading) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Loading showroom activity…" />
        <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }, (_, index) => <Skeleton key={index} className="h-24" />)}
        </div>
        <div className="mt-4 grid gap-4 lg:grid-cols-2"><Skeleton className="h-64" /><Skeleton className="h-64" /></div>
        <Skeleton className="mt-4 h-64" />
      </div>
    );
  }

  if (summaryQuery.isError || !data) {
    return (
      <div>
        <PageHeader
          title="Dashboard"
          subtitle="Dashboard data could not be loaded."
          actions={<Button variant="outline" onClick={refresh}><RefreshCw className="h-4 w-4" />Retry</Button>}
        />
        <div role="alert" className="mt-4 rounded-xl border border-red-200 bg-red-50 p-5">
          <p className="font-medium text-red-800">Dashboard failed to load</p>
          <p className="mt-1 text-sm text-red-700">{errorText(summaryQuery.error)}</p>
        </div>
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle={`${displayDate(data.shopDate)} · Updated ${formatDateTime(data.asOf)}`}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button size="sm" onClick={() => open({ view: "sales", target: "new-sale" }, "new-sale")} disabled={!permission.newSale}>
              <ShoppingCart className="h-3.5 w-3.5" />New Sale
            </Button>
            <Button size="sm" variant="outline" onClick={onReceivePayment} disabled={!permission.receivePayment}>
              <Banknote className="h-3.5 w-3.5" />Receive Payment
            </Button>
            <Button size="sm" variant="outline" onClick={() => open({ view: "sales", target: "new-customer" }, "customers-tab")} disabled={!permission.addCustomer}>
              <UserPlus className="h-3.5 w-3.5" />Add Customer
            </Button>
            <Button size="sm" variant="outline" onClick={() => open({ view: "catalogue", target: "new-product" })} disabled={!permission.addProduct}>
              <PackageOpen className="h-3.5 w-3.5" />Add Product
            </Button>
            <Button size="sm" variant="outline" onClick={() => open({ view: "finance", target: "new-expense" })} disabled={!permission.addExpense}>
              <Wallet className="h-3.5 w-3.5" />Add Expense
            </Button>
            <Button size="sm" variant="outline" onClick={refresh} disabled={summaryQuery.isFetching}>
              <RefreshCw className={cn("h-3.5 w-3.5", summaryQuery.isFetching && "animate-spin")} />Refresh
            </Button>
          </div>
        }
      />

      <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <MetricCard tone="blue" label="Sales Today" value={data.todaySalesMinor} detail={`${data.todaySalesCount ?? 0} confirmed sale(s)`} icon={ShoppingCart} allowed={permission.sales} onClick={() => open({ view: "sales", target: "sales-today", shopDate: data.shopDate })} />
        <MetricCard tone="green" label="Received Today" value={data.todayReceivedMinor} detail={`${data.todayReceivedCount ?? 0} customer payment(s)`} icon={Banknote} allowed={permission.receipts} onClick={() => open({ view: "sales", target: "payments-today", shopDate: data.shopDate })} />
        <MetricCard tone="violet" label="This Month Sales" value={data.monthSalesMinor} detail="Net sales after returns" icon={BarChart3} allowed={permission.sales} onClick={() => open({ view: "sales", target: "sales-month", shopDate: data.shopDate })} />
        <MetricCard tone="amber" label="Customer Dues" value={data.customerDuesMinor} detail={data.overdueCustomerMinor ? `${formatPkr(data.overdueCustomerMinor)} overdue` : "Total outstanding balance"} icon={Users} allowed={permission.receipts} onClick={() => open({ view: "sales", target: "customer-dues" })} />
        <MetricCard tone="rose" label="Supplier Payables" value={data.supplierPayablesMinor} detail={data.overdueSupplierMinor ? `${formatPkr(data.overdueSupplierMinor)} overdue` : "Total outstanding balance"} icon={Truck} allowed={permission.payables} onClick={() => open({ view: "purchases", target: "payables" })} />
        <MetricCard tone="cyan" label="Pending Deliveries" value={data.pendingDeliveries} detail="Incomplete deliveries" icon={PackageCheck} allowed={permission.deliveries} count onClick={() => open({ view: "fulfilment", target: "deliveries" })} />
      </div>

      <div className="mt-4 grid items-stretch gap-4 lg:grid-cols-2">
        <Panel
          title="Recent Sales"
          action={<PanelLink onClick={() => onNavigate("sales-history")}>View all</PanelLink>}
        >
          {!permission.sales ? (
            <EmptyState>Sales permission is required.</EmptyState>
          ) : data.recentSales.length === 0 ? (
            <EmptyState>No confirmed sales have been recorded yet.</EmptyState>
          ) : (
            <Table>
              <TableHeader><TableRow><TableHead>Invoice</TableHead><TableHead>Customer</TableHead><TableHead>Date</TableHead><TableHead className="text-right">Amount</TableHead></TableRow></TableHeader>
              <TableBody>
                {data.recentSales.map((sale) => (
                  <ClickableRow key={sale.id} onClick={() => open({ view: "sales", target: "sale", id: sale.id })}>
                    <TableCell className="whitespace-nowrap font-mono text-xs font-medium text-slate-700">{sale.invoice}</TableCell>
                    <TableCell className="max-w-40 truncate font-medium text-slate-800">{sale.customerName}</TableCell>
                    <TableCell className="whitespace-nowrap text-xs text-slate-500">{displayDate(sale.saleDate)}</TableCell>
                    <TableCell className="whitespace-nowrap text-right font-semibold tabular-nums text-slate-900">{formatPkr(sale.amountMinor)}</TableCell>
                  </ClickableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </Panel>

        <Panel
          title={data.lowStockUsesThreshold ? "Low Stock" : "Out of Stock"}
          description={data.lowStockUsesThreshold ? "Products below their saved minimum level" : "No minimum-stock thresholds are configured"}
          action={<PanelLink onClick={() => data.lowStockUsesThreshold ? open({ view: "inventory", target: "low-stock" }) : onNavigate("inventory")}>View all</PanelLink>}
        >
          {data.lowStockItems.length === 0 ? (
            <EmptyState>
              {data.lowStockUsesThreshold
                ? "No products are below their minimum stock level."
                : "No products are out of stock."}
            </EmptyState>
          ) : (
            <Table>
              <TableHeader><TableRow><TableHead>Product</TableHead><TableHead className="text-right">Available</TableHead></TableRow></TableHeader>
              <TableBody>
                {data.lowStockItems.map((item) => (
                  <ClickableRow key={item.productId} onClick={() => data.lowStockUsesThreshold ? open({ view: "inventory", target: "low-stock" }) : onNavigate("inventory")}>
                    <TableCell>
                      <div className="flex items-center gap-3">
                        <div className="h-9 w-12 shrink-0 overflow-hidden rounded-md border border-slate-200">
                          <StoredImage path={item.thumbnailPath} alt={item.productName} className="h-full w-full object-cover object-center" />
                        </div>
                        <span className="truncate font-medium text-slate-800">{item.productName}</span>
                      </div>
                    </TableCell>
                    <TableCell className="text-right">
                      <span className="inline-flex rounded-full bg-amber-100 px-2.5 py-1 text-xs font-semibold tabular-nums text-amber-800">
                        {item.available.toLocaleString("en-PK")} available
                      </span>
                    </TableCell>
                  </ClickableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </Panel>
      </div>

      <div className="mt-4">
        <Panel
          title="Top Products"
          description="Last 30 days by sales revenue"
          action={<PanelLink onClick={() => onNavigate("catalogue")}>View all</PanelLink>}
        >
          {!permission.sales ? (
            <EmptyState>Sales permission is required.</EmptyState>
          ) : data.topProducts.length === 0 ? (
            <EmptyState>No confirmed product sales were recorded in the last 30 days.</EmptyState>
          ) : (
            <Table>
              <TableHeader><TableRow><TableHead>Product</TableHead><TableHead className="text-right">Units sold</TableHead><TableHead className="text-right">Sales amount</TableHead></TableRow></TableHeader>
              <TableBody>
                {data.topProducts.map((product) => (
                  <ClickableRow key={`${product.itemType}-${product.itemId}-${product.productName}`} onClick={() => onNavigate("catalogue")}>
                    <TableCell>
                      <div className="flex items-center gap-3">
                        <div className="h-9 w-12 shrink-0 overflow-hidden rounded-md border border-slate-200">
                          <StoredImage path={product.imagePath} alt={product.productName} className="h-full w-full object-cover object-center" />
                        </div>
                        <span className="font-medium text-slate-800">{product.productName}</span>
                        {product.itemType === "bundle" && <span className="rounded bg-slate-100 px-1.5 py-0.5 text-[10px] font-medium text-slate-500">Set</span>}
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-medium tabular-nums text-slate-700">{product.unitsSold.toLocaleString("en-PK")}</TableCell>
                    <TableCell className="text-right font-semibold tabular-nums text-slate-900">{formatPkr(product.salesAmountMinor)}</TableCell>
                  </ClickableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </Panel>
      </div>
    </div>
  );
}
