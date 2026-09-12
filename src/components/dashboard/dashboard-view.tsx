"use client";

import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Banknote,
  Boxes,
  CalendarDays,
  Clock3,
  PackageCheck,
  PackageOpen,
  Plus,
  RefreshCw,
  ShoppingCart,
  Truck,
  UserPlus,
  Wallet,
  WalletCards,
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
import { dashboardSummary } from "@/lib/tauri/api";
import { asCommandError } from "@/lib/tauri/client";
import { formatDateTime, formatPkr } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ShellView } from "@/lib/shell";
import { setDashboardTarget, type DashboardTarget } from "@/lib/dashboard-navigation";

function errorText(error: unknown): string {
  const command = asCommandError(error);
  return command.code
    ? `${command.code}${command.correlationId ? ` · ${command.correlationId}` : ""}: ${command.message}`
    : String(error);
}

function Panel({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <section className="min-w-0 rounded-lg border border-neutral-200 bg-white shadow-sm">
      <div className="border-b border-neutral-100 px-5 py-4">
        <h2 className="text-sm font-semibold text-neutral-900">{title}</h2>
        {description && <p className="mt-1 text-xs text-neutral-500">{description}</p>}
      </div>
      <div className="p-5">{children}</div>
    </section>
  );
}

function Skeleton({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-lg bg-neutral-100", className)} />;
}

function MetricCard({
  label,
  value,
  detail,
  icon: Icon,
  allowed,
  onClick,
}: {
  label: string;
  value?: number | null;
  detail: string;
  icon: React.ElementType;
  allowed: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!allowed}
      className="group flex min-h-28 w-full flex-col justify-between rounded-lg border border-neutral-200 bg-white p-4 text-left shadow-sm transition hover:border-forest-300 hover:bg-forest-50/50 disabled:cursor-not-allowed disabled:bg-neutral-50 disabled:opacity-70"
    >
      <span className="flex items-center justify-between gap-3 text-sm font-medium text-neutral-600">
        {label}
        <span className="flex h-8 w-8 items-center justify-center rounded-md bg-forest-50 text-forest-700 group-hover:bg-white">
          <Icon className="h-4 w-4" aria-hidden="true" />
        </span>
      </span>
      <span className="mt-3 text-2xl font-semibold tabular-nums tracking-tight text-neutral-950">
        {allowed && value != null ? formatPkr(value) : "Restricted"}
      </span>
      <span className="mt-1 text-xs text-neutral-500">{allowed ? detail : "Permission required"}</span>
    </button>
  );
}

function CountCard({
  label,
  value,
  detail,
  icon,
  allowed,
  onClick,
}: Omit<React.ComponentProps<typeof MetricCard>, "value"> & { value?: number | null }) {
  const Icon = icon;
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!allowed}
      className="group flex min-h-28 w-full flex-col justify-between rounded-lg border border-neutral-200 bg-white p-4 text-left shadow-sm transition hover:border-forest-300 hover:bg-forest-50/50 disabled:cursor-not-allowed disabled:bg-neutral-50 disabled:opacity-70"
    >
      <span className="flex items-center justify-between gap-3 text-sm font-medium text-neutral-600">
        {label}
        <span className="flex h-8 w-8 items-center justify-center rounded-md bg-forest-50 text-forest-700 group-hover:bg-white">
          <Icon className="h-4 w-4" aria-hidden="true" />
        </span>
      </span>
      <span className="mt-3 text-2xl font-semibold tabular-nums tracking-tight text-neutral-950">
        {allowed && value != null ? value.toLocaleString("en-PK") : "Restricted"}
      </span>
      <span className="mt-1 text-xs text-neutral-500">{allowed ? detail : "Permission required"}</span>
    </button>
  );
}

function displaySchedule(value?: string | null): string {
  if (!value) return "Not scheduled";
  const datePart = value.slice(0, 10);
  const parsed = new Date(`${datePart}T00:00:00`);
  const date = Number.isNaN(parsed.getTime())
    ? datePart
    : parsed.toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "numeric" });
  const time = value.match(/[T ](\d{2}:\d{2})/)?.[1];
  return time ? `${date}, ${time}` : date;
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
    damage: hasPermission("damage.record"),
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
        <div className="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }, (_, index) => <Skeleton key={index} className="h-28" />)}
        </div>
        <div className="mt-5 grid gap-5 xl:grid-cols-2"><Skeleton className="h-72" /><Skeleton className="h-72" /></div>
        <Skeleton className="mt-5 h-28" />
      </div>
    );
  }

  if (summaryQuery.isError || !data) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Dashboard data could not be loaded." actions={<Button variant="outline" onClick={refresh}><RefreshCw className="h-4 w-4" />Retry</Button>} />
        <div role="alert" className="mt-5 rounded-lg border border-red-200 bg-red-50 p-5">
          <p className="font-medium text-red-800">Dashboard failed to load</p>
          <p className="mt-1 text-sm text-red-700">{errorText(summaryQuery.error)}</p>
        </div>
      </div>
    );
  }

  const alerts = [
    data.overdueCustomerMinor && data.overdueCustomerMinor > 0 ? {
      key: "customer-overdue", icon: AlertTriangle, label: "Overdue customer payments",
      detail: `${data.overdueCustomerCount ?? 0} invoice(s) · ${formatPkr(data.overdueCustomerMinor)}`,
      view: "sales" as ShellView, danger: true,
    } : null,
    data.overdueSupplierMinor && data.overdueSupplierMinor > 0 ? {
      key: "supplier-overdue", icon: WalletCards, label: "Overdue supplier payments",
      detail: `${data.overdueSupplierCount ?? 0} invoice(s) · ${formatPkr(data.overdueSupplierMinor)}`,
      view: "purchases" as ShellView, danger: true,
    } : null,
    data.lowStockCount > 0 ? {
      key: "low-stock", icon: Boxes, label: "Low stock",
      detail: `${data.lowStockCount} product(s) below minimum`, view: "inventory" as ShellView, danger: false,
    } : null,
    data.openDamageCount && data.openDamageCount > 0 ? {
      key: "damage", icon: PackageCheck, label: "Unresolved damage",
      detail: `${data.openDamageCount} record(s) awaiting resolution`, view: "fulfilment" as ShellView, danger: true,
    } : null,
  ].filter((alert): alert is NonNullable<typeof alert> => alert !== null);

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle={`Business date ${data.shopDate} · Updated ${formatDateTime(data.asOf)}`}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => open({ view: "sales", target: "new-sale" }, "new-sale")} disabled={!permission.newSale}>
              <Plus className="h-4 w-4" />New Sale
            </Button>
            <Button variant="outline" onClick={onReceivePayment} disabled={!permission.receivePayment}>
              <Banknote className="h-4 w-4" />Receive Payment
            </Button>
            <Button variant="outline" onClick={() => open({ view: "sales", target: "new-customer" }, "customers-tab")} disabled={!permission.addCustomer}>
              <UserPlus className="h-4 w-4" />Add Customer
            </Button>
            <Button variant="outline" onClick={() => open({ view: "catalogue", target: "new-product" })} disabled={!permission.addProduct}>
              <PackageOpen className="h-4 w-4" />Add Product
            </Button>
            <Button variant="outline" onClick={() => open({ view: "finance", target: "new-expense" })} disabled={!permission.addExpense}>
              <Wallet className="h-4 w-4" />Add Expense
            </Button>
            <Button variant="outline" onClick={refresh} disabled={summaryQuery.isFetching}>
              <RefreshCw className={cn("h-4 w-4", summaryQuery.isFetching && "animate-spin")} />Refresh
            </Button>
          </div>
        }
      />

      <div className="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <MetricCard label="Sales Today" value={data.todaySalesMinor} detail={`${data.todaySalesCount ?? 0} sale(s) · cash and credit`} icon={ShoppingCart} allowed={permission.sales} onClick={() => open({ view: "sales", target: "sales-today", shopDate: data.shopDate })} />
        <MetricCard label="Received Today" value={data.todayReceivedMinor} detail={`${data.todayReceivedCount ?? 0} customer payment(s)`} icon={Banknote} allowed={permission.receipts} onClick={() => open({ view: "sales", target: "payments-today", shopDate: data.shopDate })} />
        <MetricCard label="This Month Sales" value={data.monthSalesMinor} detail="Net sales after posted returns" icon={CalendarDays} allowed={permission.sales} onClick={() => open({ view: "sales", target: "sales-month", shopDate: data.shopDate })} />
        <MetricCard label="Customer Dues" value={data.customerDuesMinor} detail={data.overdueCustomerMinor ? `${formatPkr(data.overdueCustomerMinor)} overdue` : "Total outstanding balance"} icon={WalletCards} allowed={permission.receipts} onClick={() => open({ view: "sales", target: "customer-dues" })} />
        <MetricCard label="Supplier Payables" value={data.supplierPayablesMinor} detail={data.overdueSupplierMinor ? `${formatPkr(data.overdueSupplierMinor)} overdue` : "Total outstanding balance"} icon={Truck} allowed={permission.payables} onClick={() => open({ view: "purchases", target: "payables" })} />
        <CountCard label="Pending Deliveries" value={data.pendingDeliveries} detail="Incomplete and not cancelled" icon={PackageCheck} allowed={permission.deliveries} onClick={() => open({ view: "fulfilment", target: "deliveries" })} />
      </div>

      <div className="mt-5 grid gap-5 xl:grid-cols-2">
        <Panel title="Upcoming Deliveries" description="Overdue deliveries are shown first">
          {!permission.deliveries ? (
            <p className="text-sm text-neutral-500">Delivery permission is required.</p>
          ) : data.upcomingDeliveries.length === 0 ? (
            <p className="text-sm text-neutral-500">No incomplete deliveries.</p>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader><TableRow><TableHead>Customer / Items</TableHead><TableHead>Scheduled</TableHead><TableHead>Status</TableHead></TableRow></TableHeader>
                <TableBody>
                  {data.upcomingDeliveries.map((delivery) => (
                    <TableRow
                      key={delivery.id}
                      role="button"
                      tabIndex={0}
                      className="cursor-pointer hover:bg-neutral-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500 focus-visible:ring-inset"
                      onClick={() => open({ view: "fulfilment", target: "delivery", id: delivery.id })}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          open({ view: "fulfilment", target: "delivery", id: delivery.id });
                        }
                      }}
                    >
                      <TableCell className="max-w-[340px]"><p className="font-medium text-neutral-900">{delivery.customerName}</p><p className="mt-0.5 truncate text-xs text-neutral-500" title={delivery.items}>{delivery.items}</p></TableCell>
                      <TableCell className={cn("whitespace-nowrap text-xs", delivery.isOverdue && "font-medium text-red-700")}><Clock3 className="mr-1 inline h-3.5 w-3.5" />{displaySchedule(delivery.scheduledAt)}{delivery.isOverdue && <span className="block text-[10px] uppercase">Overdue</span>}</TableCell>
                      <TableCell><Badge variant={delivery.isOverdue || delivery.status === "failed" ? "danger" : "warning"}>{delivery.status}</Badge></TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </Panel>

        <Panel title="Recent Sales & Payments" description="Latest completed customer transactions">
          {data.recentTransactions.length === 0 ? (
            <p className="text-sm text-neutral-500">No sales or customer payments recorded yet.</p>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader><TableRow><TableHead>Date</TableHead><TableHead>Customer / Reference</TableHead><TableHead>Type</TableHead><TableHead className="text-right">Amount</TableHead></TableRow></TableHeader>
                <TableBody>
                  {data.recentTransactions.map((transaction) => (
                    <TableRow
                      key={`${transaction.transactionType}-${transaction.id}`}
                      role="button"
                      tabIndex={0}
                      className="cursor-pointer hover:bg-neutral-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500 focus-visible:ring-inset"
                      onClick={() => transaction.transactionType === "Sale" ? open({ view: "sales", target: "sale", id: transaction.id }) : transaction.customerId ? open({ view: "sales", target: "payment", customerId: transaction.customerId }) : onNavigate("sales")}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          if (transaction.transactionType === "Sale") open({ view: "sales", target: "sale", id: transaction.id });
                          else if (transaction.customerId) open({ view: "sales", target: "payment", customerId: transaction.customerId });
                          else onNavigate("sales");
                        }
                      }}
                    >
                      <TableCell className="whitespace-nowrap text-xs text-neutral-500">{transaction.transactionDate}</TableCell>
                      <TableCell><p className="font-medium text-neutral-900">{transaction.customerName}</p><p className="font-mono text-xs text-neutral-500">{transaction.reference}</p></TableCell>
                      <TableCell><Badge variant={transaction.transactionType === "Payment" ? "success" : "neutral"}>{transaction.transactionType}</Badge></TableCell>
                      <TableCell className="text-right font-medium tabular-nums">{formatPkr(transaction.amountMinor)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </Panel>
      </div>

      <div className="mt-5">
        <Panel title="Attention" description="Items that need action">
          {alerts.length === 0 ? (
            <div className="flex items-center gap-2 text-sm text-neutral-500"><PackageCheck className="h-4 w-4 text-emerald-600" />Nothing needs attention right now.</div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
              {alerts.map((alert) => (
                <button key={alert.key} type="button" onClick={() => alert.key === "customer-overdue" ? open({ view: "sales", target: "customer-dues" }) : alert.key === "supplier-overdue" ? open({ view: "purchases", target: "payables" }) : alert.key === "low-stock" ? open({ view: "inventory", target: "low-stock" }) : open({ view: "fulfilment", target: "damage" })} className="flex items-start gap-3 rounded-md border border-neutral-200 p-3 text-left transition hover:border-forest-300 hover:bg-forest-50/50">
                  <span className={cn("flex h-8 w-8 shrink-0 items-center justify-center rounded-md", alert.danger ? "bg-red-50 text-red-700" : "bg-amber-50 text-amber-700")}><alert.icon className="h-4 w-4" /></span>
                  <span><span className="block text-sm font-medium text-neutral-900">{alert.label}</span><span className="mt-0.5 block text-xs text-neutral-500">{alert.detail}</span></span>
                </button>
              ))}
            </div>
          )}
        </Panel>
      </div>

    </div>
  );
}
