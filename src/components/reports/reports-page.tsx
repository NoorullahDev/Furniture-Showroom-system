"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  FileSpreadsheet,
  FileText,
  Loader2,
  ShoppingCart,
  Package,
  Users,
  Truck,
  TrendingUp,
  ReceiptText,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { useSession } from "@/components/session/session-provider";
import { useToast } from "@/components/ui/toast";
import {
  reportExport,
  openFile,
  saleList,
  stockValuation,
  receivables,
  payableAging,
  profitSummary,
  expenseList,
  type ReportFilterInput,
} from "@/lib/tauri/api";
import { formatPkr } from "@/lib/format";

type ReportType =
  | "sales_summary"
  | "stock_valuation"
  | "customer_dues"
  | "supplier_payables"
  | "profit_loss"
  | "expense_report";

type ReportMeta = {
  id: ReportType;
  title: string;
  description: string;
  icon: React.ElementType;
  columns: { header: string; key: string; alignRight?: boolean }[];
};

const REPORTS: ReportMeta[] = [
  {
    id: "sales_summary",
    title: "Sales Summary",
    description: "Date, sale number, customer, totals, paid, due, and status.",
    icon: ShoppingCart,
    columns: [
      { header: "Date", key: "date" },
      { header: "Sale #", key: "saleNumber" },
      { header: "Customer", key: "customer" },
      { header: "Total", key: "total", alignRight: true },
      { header: "Paid", key: "paid", alignRight: true },
      { header: "Due", key: "due", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "stock_valuation",
    title: "Stock Valuation",
    description: "Article, product name, quantity, unit cost, and total value.",
    icon: Package,
    columns: [
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "Qty", key: "qty", alignRight: true },
      { header: "Unit Cost", key: "unitCost", alignRight: true },
      { header: "Value", key: "value", alignRight: true },
    ],
  },
  {
    id: "customer_dues",
    title: "Customer Dues",
    description: "Customer name, phone, balance, due, and overdue amounts.",
    icon: Users,
    columns: [
      { header: "Customer", key: "customer" },
      { header: "Phone", key: "phone" },
      { header: "Balance", key: "balance", alignRight: true },
      { header: "Due", key: "due", alignRight: true },
      { header: "Overdue", key: "overdue", alignRight: true },
    ],
  },
  {
    id: "supplier_payables",
    title: "Supplier Payables",
    description:
      "Supplier name, purchase number, invoice date, due, and age in days.",
    icon: Truck,
    columns: [
      { header: "Supplier", key: "supplier" },
      { header: "Purchase #", key: "purchaseNumber" },
      { header: "Invoice Date", key: "invoiceDate" },
      { header: "Due", key: "due", alignRight: true },
      { header: "Days", key: "days", alignRight: true },
    ],
  },
  {
    id: "profit_loss",
    title: "Profit & Loss",
    description:
      "Revenue, COGS, gross profit, expenses, damage loss, net profit, margin.",
    icon: TrendingUp,
    columns: [
      { header: "Line Item", key: "lineItem" },
      { header: "Amount", key: "amount", alignRight: true },
    ],
  },
  {
    id: "expense_report",
    title: "Expense Report",
    description:
      "Expense number, category, amount, date, account, description, status.",
    icon: ReceiptText,
    columns: [
      { header: "Expense #", key: "expenseNumber" },
      { header: "Category", key: "category" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Date", key: "date" },
      { header: "Account", key: "account" },
      { header: "Description", key: "description" },
      { header: "Status", key: "status" },
    ],
  },
];

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function dateNdaysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString().slice(0, 10);
}

function inRange(dateStr: string, from: string | null, to: string | null): boolean {
  if (from && dateStr < from) return false;
  if (to && dateStr > to) return false;
  return true;
}

type Preset = { label: string; from: string | null; to: string | null };

const PRESETS: Preset[] = [
  { label: "All time", from: null, to: null },
  { label: "This Week", from: dateNdaysAgo(7), to: todayIso() },
  { label: "This Month", from: dateNdaysAgo(30), to: todayIso() },
  { label: "This Quarter", from: dateNdaysAgo(90), to: todayIso() },
  { label: "This Year", from: dateNdaysAgo(365), to: todayIso() },
];

function FilterPanel({
  filter,
  onFilterChange,
}: {
  filter: ReportFilterInput;
  onFilterChange: (f: ReportFilterInput) => void;
}) {
  const [activePreset, setActivePreset] = React.useState<number>(0);

  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-4">
      <div className="flex flex-wrap items-end gap-3">
        <div className="grid gap-1.5">
          <Label>From</Label>
          <Input
            type="date"
            value={filter.fromDate ?? ""}
            onChange={(e) => {
              onFilterChange({ ...filter, fromDate: e.target.value || null });
              setActivePreset(-1);
            }}
            className="w-40"
          />
        </div>
        <div className="grid gap-1.5">
          <Label>To</Label>
          <Input
            type="date"
            value={filter.toDate ?? ""}
            onChange={(e) => {
              onFilterChange({ ...filter, toDate: e.target.value || null });
              setActivePreset(-1);
            }}
            className="w-40"
          />
        </div>
        <div className="flex flex-wrap gap-1.5">
          {PRESETS.map((p, i) => (
            <Button
              key={p.label}
              variant={activePreset === i ? "primary" : "outline"}
              size="sm"
              onClick={() => {
                setActivePreset(i);
                onFilterChange({ fromDate: p.from, toDate: p.to });
              }}
            >
              {p.label}
            </Button>
          ))}
        </div>
      </div>
    </div>
  );
}

function ReportTable({
  meta,
  rows,
  loading,
  error,
}: {
  meta: ReportMeta;
  rows: Record<string, string>[];
  loading: boolean;
  error: string | null;
}) {
  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-neutral-500">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Loading...
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700">
        {error}
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="py-12 text-center text-sm text-neutral-500">
        No data for the selected filters.
      </div>
    );
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          {meta.columns.map((col) => (
            <TableHead
              key={col.key}
              className={col.alignRight ? "text-right" : ""}
            >
              {col.header}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row, i) => (
          <TableRow key={i}>
            {meta.columns.map((col) => (
              <TableCell
                key={col.key}
                className={col.alignRight ? "text-right tabular-nums" : ""}
              >
                {row[col.key] ?? ""}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

// ── Fetch + transform logic per report type ──────────────────────────────────

async function fetchReportData(
  reportType: ReportType,
  session: string,
  filter: ReportFilterInput,
): Promise<{ rows: Record<string, string>[]; rowCount: number }> {
  const from = filter.fromDate ?? null;
  const to = filter.toDate ?? null;

  switch (reportType) {
    case "sales_summary": {
      const sales = await saleList(session);
      const filtered = sales.filter((s) => inRange(s.saleDate, from, to));
      const rows = filtered.map((s) => ({
        date: s.saleDate,
        saleNumber: s.saleNumber ?? "",
        customer: s.customerName ?? "Walk-in",
        total: formatPkr(s.totalMinor),
        paid: formatPkr(s.paidMinor + s.advanceUsedMinor),
        due: formatPkr(s.dueMinor),
        status: s.status,
      }));
      return { rows, rowCount: rows.length };
    }
    case "stock_valuation": {
      const items = await stockValuation(session);
      const rows = items.map((v) => ({
        article: v.articleNumber,
        product: v.productName,
        qty: v.sellableQty.toLocaleString(),
        unitCost: formatPkr(v.unitCostMinor),
        value: formatPkr(v.valueMinor),
      }));
      return { rows, rowCount: rows.length };
    }
    case "customer_dues": {
      const data = await receivables(session);
      const rows = data.highBalance.map((c) => ({
        customer: c.customerName,
        phone: c.phone ?? "",
        balance: formatPkr(c.balanceMinor),
        due: formatPkr(c.dueMinorTotal),
        overdue: formatPkr(c.overdueMinorTotal),
      }));
      return { rows, rowCount: rows.length };
    }
    case "supplier_payables": {
      const data = await payableAging(session);
      const rows = data.map((p) => ({
        supplier: p.supplierName,
        purchaseNumber: p.purchaseNumber ?? "",
        invoiceDate: p.invoiceDate,
        due: formatPkr(p.dueMinor),
        days: p.ageDays.toLocaleString(),
      }));
      return { rows, rowCount: rows.length };
    }
    case "profit_loss": {
      const data = await profitSummary(session, from, to);
      const margin =
        data.revenueMinor > 0
          ? `${((data.operationalProfitMinor / data.revenueMinor) * 100).toFixed(1)}%`
          : "N/A";
      const rows = [
        { lineItem: "Revenue", amount: formatPkr(data.revenueMinor) },
        { lineItem: "Cost of Goods Sold", amount: formatPkr(data.cogsMinor) },
        {
          lineItem: "Gross Profit",
          amount: formatPkr(data.grossProfitMinor),
        },
        { lineItem: "Expenses", amount: formatPkr(data.expensesMinor) },
        {
          lineItem: "Damage Loss",
          amount: formatPkr(data.damageLossMinor),
        },
        {
          lineItem: "Net Profit",
          amount: formatPkr(data.operationalProfitMinor),
        },
        { lineItem: "Margin", amount: margin },
      ];
      return { rows, rowCount: rows.length };
    }
    case "expense_report": {
      const expenses = await expenseList(session, {
        fromDate: from,
        toDate: to,
        categoryId: filter.categoryId ?? null,
        cashAccountId: filter.cashAccountId ?? null,
        status: filter.status ?? null,
        limit: 1000,
      });
      const rows = expenses.map((e) => ({
        expenseNumber: e.expenseNumber ?? "",
        category: e.categoryName,
        amount: formatPkr(e.amountMinor),
        date: e.expenseDate,
        account: e.cashAccountName,
        description: e.description,
        status: e.status,
      }));
      return { rows, rowCount: rows.length };
    }
  }
}

export function ReportsPage() {
  const { profile } = useSession();
  const { toast } = useToast();
  const session = profile?.sessionId ?? "";

  const [selected, setSelected] = React.useState<ReportType | null>(null);
  const [filter, setFilter] = React.useState<ReportFilterInput>({});
  const [exporting, setExporting] = React.useState<"csv" | "pdf" | null>(null);

  const meta = selected ? REPORTS.find((r) => r.id === selected)! : null;

  const reportQuery = useQuery({
    queryKey: ["reports", selected, filter, session],
    queryFn: () => fetchReportData(selected!, session, filter),
    enabled: !!selected && !!session,
  });

  const handleExport = async (format: "csv" | "pdf") => {
    if (!selected || !session) return;
    setExporting(format);
    try {
      const result = await reportExport(session, selected, filter, format);
      await openFile(session, result.reportPath);
      toast({
        title: `${format.toUpperCase()} exported`,
        description: `${result.rowCount} rows generated.`,
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast({ title: "Export failed", description: msg });
    } finally {
      setExporting(null);
    }
  };

  const handleSelect = (id: ReportType) => {
    setSelected(id);
    setFilter({});
  };

  if (!meta) {
    return (
      <div className="grid gap-6">
        <PageHeader
          title="Reports"
          subtitle="Generate and export operational reports."
        />
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {REPORTS.map((r) => {
            const Icon = r.icon;
            return (
              <button
                key={r.id}
                type="button"
                onClick={() => handleSelect(r.id)}
                className="group flex items-start gap-3 rounded-lg border border-neutral-200 bg-white p-4 text-left transition-colors hover:border-forest-300 hover:bg-forest-50/50"
              >
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-forest-100 text-forest-700 transition-colors group-hover:bg-forest-200">
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-medium text-neutral-800">
                    {r.title}
                  </p>
                  <p className="mt-0.5 text-xs text-neutral-500">
                    {r.description}
                  </p>
                </div>
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className="grid gap-4">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setSelected(null)}
        >
          <ArrowLeft className="mr-1 h-4 w-4" />
          All reports
        </Button>
        <PageHeader title={meta.title} subtitle={meta.description} />
      </div>

      <FilterPanel filter={filter} onFilterChange={setFilter} />

      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleExport("csv")}
          disabled={!!exporting}
        >
          {exporting === "csv" ? (
            <Loader2 className="mr-1 h-4 w-4 animate-spin" />
          ) : (
            <FileSpreadsheet className="mr-1 h-4 w-4" />
          )}
          Export CSV
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleExport("pdf")}
          disabled={!!exporting}
        >
          {exporting === "pdf" ? (
            <Loader2 className="mr-1 h-4 w-4 animate-spin" />
          ) : (
            <FileText className="mr-1 h-4 w-4" />
          )}
          Export PDF
        </Button>
        <span className="ml-auto text-xs text-neutral-400">
          {reportQuery.data?.rowCount ?? 0} rows
        </span>
      </div>

      <div className="rounded-lg border border-neutral-200 bg-white">
        <ReportTable
          meta={meta}
          rows={reportQuery.data?.rows ?? []}
          loading={reportQuery.isLoading}
          error={reportQuery.isError ? "Failed to load report data." : null}
        />
      </div>
    </div>
  );
}
