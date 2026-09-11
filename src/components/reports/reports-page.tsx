"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import {
  FileSpreadsheet,
  FileText,
  Loader2,
  ShoppingCart,
  Package,
  Users,
  Truck,
  TrendingUp,
  ReceiptText,
  Undo2,
  ClipboardList,
  BarChart3,
  UserCheck,
  LayoutGrid,
  Award,
  ArrowRightLeft,
  AlertTriangle,
  MapPin,
  CreditCard,
  PiggyBank,
  ShieldAlert,
  Building2,
  DollarSign,
  BookOpen,
  Landmark,
  Activity,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
import { cn } from "@/lib/utils";
import {
  reportExport,
  openFile,
  saleList,
  stockValuation,
  receivables,
  payableAging,
  profitSummary,
  expenseList,
  expenseCategoryList,
  purchaseList,
  saleReturnList,
  supplierReturnList,
  deliveryList,
  damageList,
  auditQuery,
  customerList,
  supplierList,
  cashAccountList,
  settingsGet,
  type ReportFilterInput,
} from "@/lib/tauri/api";
import { formatCurrency, formatPkr } from "@/lib/format";

type ReportType =
  | "sales_summary"
  | "customer_dues"
  | "supplier_payables"
  | "purchase_report"
  | "stock_valuation"
  | "returns_report"
  | "expense_report"
  | "profit_loss"
  | "deliveries_report"
  | "audit_report"
  | "sales_by_product"
  | "sales_by_customer"
  | "sales_by_category"
  | "best_slow_sellers"
  | "stock_movements"
  | "low_stock"
  | "stock_by_location"
  | "customer_statements"
  | "customer_receipts"
  | "customer_advances"
  | "credit_limit_exceptions"
  | "supplier_statements"
  | "supplier_payments"
  | "cash_book"
  | "account_balances"
  | "user_activity"
  | "all_reports";

type ReportMeta = {
  id: ReportType;
  title: string;
  icon: React.ElementType;
  columns: { header: string; key: string; alignRight?: boolean }[];
};

const REPORTS: ReportMeta[] = [
  {
    id: "sales_summary",
    title: "Sales",
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
    id: "returns_report",
    title: "Returns",
    icon: Undo2,
    columns: [
      { header: "Date", key: "date" },
      { header: "Type", key: "type" },
      { header: "Return #", key: "returnNumber" },
      { header: "Ref #", key: "refNumber" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "purchase_report",
    title: "Purchases",
    icon: Truck,
    columns: [
      { header: "Date", key: "date" },
      { header: "Purchase #", key: "purchaseNumber" },
      { header: "Supplier", key: "supplier" },
      { header: "Total", key: "total", alignRight: true },
      { header: "Paid", key: "paid", alignRight: true },
      { header: "Due", key: "due", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "expense_report",
    title: "Expenses",
    icon: ReceiptText,
    columns: [
      { header: "Date", key: "date" },
      { header: "Expense #", key: "expenseNumber" },
      { header: "Category", key: "category" },
      { header: "Note", key: "note" },
      { header: "Reference", key: "reference" },
      { header: "Method", key: "method" },
      { header: "Account", key: "account" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "profit_loss",
    title: "Profit",
    icon: TrendingUp,
    columns: [
      { header: "Line Item", key: "lineItem" },
      { header: "Amount", key: "amount", alignRight: true },
    ],
  },
  {
    id: "stock_valuation",
    title: "Inventory",
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
    id: "supplier_payables",
    title: "Suppliers",
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
    id: "customer_dues",
    title: "Customers",
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
    id: "deliveries_report",
    title: "Deliveries & Damage",
    icon: Truck,
    columns: [
      { header: "Date", key: "date" },
      { header: "Type", key: "type" },
      { header: "Ref #", key: "refNumber" },
      { header: "Details", key: "details" },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "audit_report",
    title: "Audit & Reversals",
    icon: ClipboardList,
    columns: [
      { header: "Time", key: "time" },
      { header: "User", key: "user" },
      { header: "Action", key: "action" },
      { header: "Target", key: "target" },
      { header: "Details", key: "details" },
    ],
  },
  {
    id: "sales_by_product",
    title: "Sales by Product",
    icon: BarChart3,
    columns: [
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "Qty Sold", key: "qtySold", alignRight: true },
      { header: "Revenue", key: "revenue", alignRight: true },
      { header: "Cost", key: "cost", alignRight: true },
      { header: "Profit", key: "profit", alignRight: true },
    ],
  },
  {
    id: "sales_by_customer",
    title: "Sales by Customer",
    icon: UserCheck,
    columns: [
      { header: "Customer", key: "customer" },
      { header: "Sales Count", key: "salesCount", alignRight: true },
      { header: "Total Sales", key: "totalSales", alignRight: true },
      { header: "Paid", key: "paid", alignRight: true },
      { header: "Due", key: "due", alignRight: true },
    ],
  },
  {
    id: "sales_by_category",
    title: "Sales by Category",
    icon: LayoutGrid,
    columns: [
      { header: "Category", key: "category" },
      { header: "Qty Sold", key: "qtySold", alignRight: true },
      { header: "Revenue", key: "revenue", alignRight: true },
    ],
  },
  {
    id: "best_slow_sellers",
    title: "Best / Slow Sellers",
    icon: Award,
    columns: [
      { header: "Rank", key: "rank", alignRight: true },
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "Qty Sold", key: "qtySold", alignRight: true },
      { header: "Revenue", key: "revenue", alignRight: true },
    ],
  },
  {
    id: "stock_movements",
    title: "Stock Movements",
    icon: ArrowRightLeft,
    columns: [
      { header: "Date/Time", key: "dateTime" },
      { header: "Type", key: "type" },
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "Location", key: "location" },
      { header: "Qty Delta", key: "qtyDelta", alignRight: true },
      { header: "Reason", key: "reason" },
      { header: "User", key: "user" },
    ],
  },
  {
    id: "low_stock",
    title: "Low Stock",
    icon: AlertTriangle,
    columns: [
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "Category", key: "category" },
      { header: "Available", key: "available", alignRight: true },
      { header: "Min Stock", key: "minStock", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "stock_by_location",
    title: "Stock by Location",
    icon: MapPin,
    columns: [
      { header: "Location", key: "location" },
      { header: "Article", key: "article" },
      { header: "Product", key: "product" },
      { header: "On Hand", key: "onHand", alignRight: true },
      { header: "Reserved", key: "reserved", alignRight: true },
      { header: "Damaged", key: "damaged", alignRight: true },
      { header: "Available", key: "available", alignRight: true },
    ],
  },
  {
    id: "customer_statements",
    title: "Customer Statements",
    icon: FileText,
    columns: [
      { header: "Code", key: "code" },
      { header: "Customer", key: "customer" },
      { header: "Phone", key: "phone" },
      { header: "Total Due", key: "totalDue", alignRight: true },
      { header: "Total Paid", key: "totalPaid", alignRight: true },
      { header: "Opening Balance", key: "openingBalance", alignRight: true },
    ],
  },
  {
    id: "customer_receipts",
    title: "Customer Receipts",
    icon: CreditCard,
    columns: [
      { header: "Receipt #", key: "receiptNumber" },
      { header: "Date", key: "date" },
      { header: "Customer", key: "customer" },
      { header: "Method", key: "method" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Advance", key: "advance", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "customer_advances",
    title: "Customer Advances",
    icon: PiggyBank,
    columns: [
      { header: "Code", key: "code" },
      { header: "Customer", key: "customer" },
      { header: "Phone", key: "phone" },
      { header: "Advance Balance", key: "advanceBalance", alignRight: true },
    ],
  },
  {
    id: "credit_limit_exceptions",
    title: "Credit Limit Exceptions",
    icon: ShieldAlert,
    columns: [
      { header: "Code", key: "code" },
      { header: "Customer", key: "customer" },
      { header: "Phone", key: "phone" },
      { header: "Credit Limit", key: "creditLimit", alignRight: true },
      { header: "Current Balance", key: "currentBalance", alignRight: true },
      { header: "Over Limit", key: "overLimit", alignRight: true },
    ],
  },
  {
    id: "supplier_statements",
    title: "Supplier Statements",
    icon: Building2,
    columns: [
      { header: "Code", key: "code" },
      { header: "Supplier", key: "supplier" },
      { header: "Phone", key: "phone" },
      { header: "Total Payable", key: "totalPayable", alignRight: true },
      { header: "Total Paid", key: "totalPaid", alignRight: true },
    ],
  },
  {
    id: "supplier_payments",
    title: "Supplier Payments",
    icon: DollarSign,
    columns: [
      { header: "Payment #", key: "paymentNumber" },
      { header: "Date", key: "date" },
      { header: "Supplier", key: "supplier" },
      { header: "Method", key: "method" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Status", key: "status" },
    ],
  },
  {
    id: "cash_book",
    title: "Cash Book",
    icon: BookOpen,
    columns: [
      { header: "Date/Time", key: "dateTime" },
      { header: "Account", key: "account" },
      { header: "Type", key: "type" },
      { header: "Amount", key: "amount", alignRight: true },
      { header: "Reference", key: "reference" },
      { header: "Reason", key: "reason" },
      { header: "User", key: "user" },
    ],
  },
  {
    id: "account_balances",
    title: "Account Balances",
    icon: Landmark,
    columns: [
      { header: "Code", key: "code" },
      { header: "Account", key: "account" },
      { header: "Kind", key: "kind" },
      { header: "Opening Balance", key: "openingBalance", alignRight: true },
      { header: "Current Balance", key: "currentBalance", alignRight: true },
    ],
  },
  {
    id: "user_activity",
    title: "User Activity",
    icon: Activity,
    columns: [
      { header: "User", key: "user" },
      { header: "Action", key: "action" },
      { header: "Count", key: "count", alignRight: true },
      { header: "First Action", key: "firstAction" },
      { header: "Last Action", key: "lastAction" },
    ],
  },
  {
    id: "all_reports",
    title: "All Reports",
    icon: BarChart3,
    columns: [
      { header: "Section", key: "section" },
      { header: "Metric", key: "metric" },
      { header: "Value", key: "value", alignRight: true },
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

function startOfThisWeek(): string {
  const d = new Date();
  const day = d.getDay();
  const diff = d.getDate() - day + (day === 0 ? -6 : 1);
  d.setDate(diff);
  return d.toISOString().slice(0, 10);
}

function startOfLastWeek(): string {
  const d = new Date();
  const day = d.getDay();
  const diff = d.getDate() - day + (day === 0 ? -13 : -6);
  d.setDate(diff);
  return d.toISOString().slice(0, 10);
}

function endOfLastWeek(): string {
  const d = new Date();
  const day = d.getDay();
  const diff = d.getDate() - day - 1 + (day === 0 ? 0 : 0);
  d.setDate(diff);
  return d.toISOString().slice(0, 10);
}

function startOfYear(): string {
  return `${new Date().getFullYear()}-01-01`;
}

function startOfLastYear(): string {
  return `${new Date().getFullYear() - 1}-01-01`;
}

function endOfLastYear(): string {
  return `${new Date().getFullYear() - 1}-12-31`;
}

function inRange(dateStr: string, from: string | null, to: string | null): boolean {
  if (!dateStr) return false;
  const d = dateStr.slice(0, 10);
  if (from && d < from) return false;
  if (to && d > to) return false;
  return true;
}

type Preset = { label: string; from: string | null; to: string | null };

const PRESETS: Preset[] = [
  { label: "Today", from: todayIso(), to: todayIso() },
  { label: "Yesterday", from: dateNdaysAgo(1), to: dateNdaysAgo(1) },
  { label: "This Week", from: startOfThisWeek(), to: todayIso() },
  { label: "Last Week", from: startOfLastWeek(), to: endOfLastWeek() },
  { label: "This Month", from: dateNdaysAgo(30), to: todayIso() },
  { label: "Last Month", from: dateNdaysAgo(60), to: dateNdaysAgo(30) },
  { label: "This Year", from: startOfYear(), to: todayIso() },
  { label: "Last Year", from: startOfLastYear(), to: endOfLastYear() },
  { label: "All Time", from: null, to: null },
  { label: "Clear", from: null, to: null },
];

function FilterPanel({
  filter,
  onFilterChange,
  onApply,
  categories,
}: {
  filter: ReportFilterInput;
  onFilterChange: (f: ReportFilterInput) => void;
  onApply: () => void;
  categories?: { id: number; name: string; isActive: boolean }[];
}) {
  const [activePreset, setActivePreset] = React.useState<number>(5);

  return (
    <div className="flex flex-wrap items-end gap-3 rounded-lg border border-neutral-200 bg-white p-4">
      {categories && (
        <select
          className="h-9 min-w-48 rounded-md border border-neutral-300 bg-white px-3 text-sm"
          value={filter.categoryId ?? ""}
          onChange={(event) => onFilterChange({ ...filter, categoryId: event.target.value ? Number(event.target.value) : null })}
          aria-label="Expense category"
        >
          <option value="">All expense categories</option>
          {categories.map((category) => <option key={category.id} value={category.id}>{category.name}{category.isActive ? "" : " (archived)"}</option>)}
        </select>
      )}
      <div className="flex items-center gap-2">
        <Input
          type="date"
          value={filter.fromDate ?? ""}
          onChange={(e) => {
            onFilterChange({ ...filter, fromDate: e.target.value || null });
            setActivePreset(-1);
          }}
          className="w-40"
        />
        <span className="text-neutral-400">→</span>
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
      <div className="flex flex-wrap items-center gap-1.5 ml-2">
        {PRESETS.map((p, i) => (
          <Button
            key={p.label}
            variant="outline"
            className={cn(
              "text-xs px-3 h-9",
              activePreset === i && p.label !== "Clear" ? "border-blue-600 text-blue-600 bg-blue-50" : "",
              p.label === "Clear" ? "text-red-600 hover:text-red-700" : ""
            )}
            onClick={() => {
              setActivePreset(i);
              onFilterChange({ ...filter, fromDate: p.from, toDate: p.to });
            }}
          >
            {p.label}
          </Button>
        ))}
      </div>
      <Button onClick={onApply} className="ml-auto bg-blue-600 hover:bg-blue-700">
        Apply
      </Button>
    </div>
  );
}

// ── Fetch + transform logic per report type ──────────────────────────────────

async function fetchReportData(
  reportType: ReportType,
  session: string,
  filter: ReportFilterInput,
  currency = "PKR",
): Promise<{ rows: Record<string, string>[]; summary: Record<string, string | number>[]; rowCount: number }> {
  const from = filter.fromDate ?? null;
  const to = filter.toDate ?? null;

  switch (reportType) {
    case "sales_summary": {
      const sales = await saleList(session);
      const filtered = sales.filter((s) => inRange(s.saleDate, from, to));
      const totalNet = filtered.reduce((acc, s) => acc + s.totalMinor, 0);
      const rows = filtered.map((s) => ({
        date: s.saleDate,
        saleNumber: s.saleNumber ?? "",
        customer: s.customerName ?? "Walk-in",
        total: formatPkr(s.totalMinor),
        paid: formatPkr(s.paidMinor + s.advanceUsedMinor),
        due: formatPkr(s.dueMinor),
        status: s.status,
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Net Sales", value: formatPkr(totalNet), subtitle: `${rows.length} transactions` },
          { title: "Transactions", value: rows.length, subtitle: "Completed sales" }
        ]
      };
    }
    case "returns_report": {
      const sReturns = await saleReturnList(session, { limit: 10000 });
      const supReturns = await supplierReturnList(session);
      
      const sFiltered = sReturns.filter(r => inRange(r.returnDate, from, to));
      const supFiltered = supReturns.filter(r => inRange(r.returnDate, from, to));
      
      const rows = [
        ...sFiltered.map(r => ({
          date: r.returnDate,
          type: "Customer Return",
          returnNumber: r.returnNumber ?? "",
          refNumber: r.saleNumber ?? "",
          amount: formatPkr(r.totalRefundMinor),
          status: r.status,
        })),
        ...supFiltered.map(r => ({
          date: r.returnDate,
          type: "Supplier Return",
          returnNumber: r.returnNumber ?? "",
          refNumber: r.purchaseId ? String(r.purchaseId) : "",
          amount: formatPkr(r.totalMinor),
          status: r.status,
        }))
      ].sort((a, b) => b.date.localeCompare(a.date));

      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Returns", value: rows.length, subtitle: "Combined returns" }
        ]
      };
    }
    case "purchase_report": {
      const purchases = await purchaseList(session);
      const filtered = purchases.filter((p) => inRange(p.purchaseDate, from, to));
      const total = filtered.reduce((acc, p) => acc + p.totalMinor, 0);
      const rows = filtered.map((p) => ({
        date: p.purchaseDate,
        purchaseNumber: p.purchaseNumber ?? "",
        supplier: p.supplierName ?? "",
        total: formatPkr(p.totalMinor),
        paid: formatPkr(p.paidMinor),
        due: formatPkr(p.dueMinor),
        status: p.status,
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Net Purchases", value: formatPkr(total), subtitle: `${rows.length} purchases` }
        ]
      };
    }
    case "stock_valuation": {
      const items = await stockValuation(session);
      const totalVal = items.reduce((acc, v) => acc + v.valueMinor, 0);
      const rows = items.map((v) => ({
        article: v.articleNumber,
        product: v.productName,
        qty: v.sellableQty.toLocaleString(),
        unitCost: formatPkr(v.unitCostMinor),
        value: formatPkr(v.valueMinor),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Stock Value", value: formatPkr(totalVal), subtitle: `${rows.length} products` }
        ]
      };
    }
    case "customer_dues": {
      const data = await receivables(session);
      const totalDue = data.highBalance.reduce((acc, c) => acc + c.dueMinorTotal, 0);
      const rows = data.highBalance.map((c) => ({
        customer: c.customerName,
        phone: c.phone ?? "",
        balance: formatPkr(c.balanceMinor),
        due: formatPkr(c.dueMinorTotal),
        overdue: formatPkr(c.overdueMinorTotal),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Dues", value: formatPkr(totalDue), subtitle: "Outstanding balances" }
        ]
      };
    }
    case "supplier_payables": {
      const data = await payableAging(session);
      const totalDue = data.reduce((acc, p) => acc + p.dueMinor, 0);
      const rows = data.map((p) => ({
        supplier: p.supplierName,
        purchaseNumber: p.purchaseNumber ?? "",
        invoiceDate: p.invoiceDate,
        due: formatPkr(p.dueMinor),
        days: p.ageDays.toLocaleString(),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Payables", value: formatPkr(totalDue), subtitle: "Outstanding to suppliers" }
        ]
      };
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
        { lineItem: "Gross Profit", amount: formatPkr(data.grossProfitMinor) },
        { lineItem: "Expenses", amount: formatPkr(data.expensesMinor) },
        { lineItem: "Damage Loss", amount: formatPkr(data.damageLossMinor) },
        { lineItem: "Net Profit", amount: formatPkr(data.operationalProfitMinor) },
        { lineItem: "Margin", amount: margin },
      ];
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Net Profit", value: formatPkr(data.operationalProfitMinor), subtitle: `Margin: ${margin}` },
          { title: "Revenue", value: formatPkr(data.revenueMinor), subtitle: "Gross income" }
        ]
      };
    }
    case "expense_report": {
      const expenses = await expenseList(session, {
        fromDate: from,
        toDate: to,
        categoryId: filter.categoryId ?? null,
        status: "posted",
        limit: 10000,
      });
      const totalExp = expenses.reduce((acc, e) => acc + e.amountMinor, 0);
      const categoryTotals = expenses.reduce<Record<string, number>>((totals, expense) => {
        totals[expense.categoryName] = (totals[expense.categoryName] ?? 0) + expense.amountMinor;
        return totals;
      }, {});
      const rows = expenses.map((e) => ({
        expenseNumber: e.expenseNumber ?? "",
        category: e.categoryName,
        amount: formatCurrency(e.amountMinor, currency),
        date: e.expenseDate,
        note: e.description || "",
        reference: e.reference ?? "",
        method: e.paymentMethodName ?? "",
        account: e.cashAccountName,
        status: e.status,
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Expenses", value: formatCurrency(totalExp, currency), subtitle: `${rows.length} records` },
          ...Object.entries(categoryTotals).sort(([a], [b]) => a.localeCompare(b)).map(([category, amount]) => ({
            title: category,
            value: formatCurrency(amount, currency),
            subtitle: "Category total",
          })),
        ]
      };
    }
    case "deliveries_report": {
      const deliveries = await deliveryList(session, { limit: 10000 });
      const damages = await damageList(session, { limit: 10000 });
      
      const delFiltered = deliveries.filter(d => inRange(d.scheduledAt || "", from, to));
      const damFiltered = damages.filter(d => inRange(d.createdAt, from, to));

      const rows = [
        ...delFiltered.map(d => ({
          date: d.scheduledAt || "",
          type: "Delivery",
          refNumber: d.deliveryNumber ?? "",
          details: `Status: ${d.status}`,
          status: d.status,
        })),
        ...damFiltered.map(d => ({
          date: d.createdAt,
          type: "Damage",
          refNumber: d.productName,
          details: d.reason ?? "",
          status: d.status,
        }))
      ].sort((a, b) => b.date.localeCompare(a.date));

      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Deliveries", value: delFiltered.length, subtitle: "In period" },
          { title: "Damage Records", value: damFiltered.length, subtitle: "In period" }
        ]
      };
    }
    case "audit_report": {
      const audit = await auditQuery(session, { limit: 1000 });
      const filtered = audit.items.filter(a => inRange(a.createdAt, from, to));
      const rows = filtered.map(a => ({
        time: new Date(a.createdAt).toLocaleString(),
        user: a.userId ? `User ${a.userId}` : "System",
        action: a.action,
        target: a.entityType || "",
        details: a.reason ? (a.reason.length > 50 ? a.reason.substring(0, 50) + '...' : a.reason) : "",
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Audit Logs", value: rows.length, subtitle: "Events recorded" }
        ]
      };
    }
    case "sales_by_product": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Full data available via export" }]
      };
    }
    case "sales_by_customer": {
      const sales = await saleList(session);
      const filtered = sales.filter(s => inRange(s.saleDate, from, to));
      const grouped: Record<string, { salesCount: number; totalSales: number; paid: number; due: number }> = {};
      for (const s of filtered) {
        const name = s.customerName || "Walk-in";
        if (!grouped[name]) grouped[name] = { salesCount: 0, totalSales: 0, paid: 0, due: 0 };
        grouped[name].salesCount++;
        grouped[name].totalSales += s.totalMinor;
        grouped[name].paid += s.paidMinor + s.advanceUsedMinor;
        grouped[name].due += s.dueMinor;
      }
      const rows = Object.entries(grouped).map(([customer, v]) => ({
        customer,
        salesCount: v.salesCount.toLocaleString(),
        totalSales: formatPkr(v.totalSales),
        paid: formatPkr(v.paid),
        due: formatPkr(v.due),
      }));
      const totalDue = Object.values(grouped).reduce((acc, v) => acc + v.due, 0);
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Dues", value: formatPkr(totalDue), subtitle: "Across all customers" },
          { title: "Customers", value: rows.length, subtitle: "With sales in period" }
        ]
      };
    }
    case "sales_by_category": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Category data requires product details" }]
      };
    }
    case "best_slow_sellers": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Full data available via export" }]
      };
    }
    case "stock_movements": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Stock movement log available via export" }]
      };
    }
    case "low_stock": {
      const items = await stockValuation(session);
      const lowStockItems = items.filter(v => v.sellableQty <= 10);
      const rows = lowStockItems.map(v => ({
        article: v.articleNumber,
        product: v.productName,
        category: "",
        available: v.sellableQty.toLocaleString(),
        minStock: "10",
        status: v.sellableQty === 0 ? "Out of Stock" : "Low",
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Low Stock Items", value: rows.length, subtitle: "Products at or below min level" }
        ]
      };
    }
    case "stock_by_location": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Location-wise stock available via export" }]
      };
    }
    case "customer_statements": {
      const data = await receivables(session);
      const rows = data.highBalance.map(c => ({
        code: "",
        customer: c.customerName,
        phone: c.phone ?? "",
        totalDue: formatPkr(c.dueMinorTotal),
        totalPaid: formatPkr(c.balanceMinor),
        openingBalance: "0",
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Customers", value: rows.length, subtitle: "With outstanding balances" }
        ]
      };
    }
    case "customer_receipts": {
      const sales = await saleList(session);
      const filtered = sales.filter(s => inRange(s.saleDate, from, to) && s.paidMinor > 0);
      const rows = filtered.map(s => ({
        receiptNumber: s.saleNumber ?? "",
        date: s.saleDate,
        customer: s.customerName || "Walk-in",
        method: "Cash",
        amount: formatPkr(s.paidMinor + s.advanceUsedMinor),
        advance: formatPkr(s.advanceUsedMinor),
        status: s.status,
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Receipts", value: rows.length, subtitle: "In period" }
        ]
      };
    }
    case "customer_advances": {
      const customers = await customerList(session);
      const withAdvance = customers.filter(c => (c.advanceMinor ?? 0) > 0);
      const rows = withAdvance.map(c => ({
        code: c.code ?? "",
        customer: c.name,
        phone: c.phone ?? "",
        advanceBalance: formatPkr(c.advanceMinor ?? 0),
      }));
      const totalAdvance = withAdvance.reduce((acc, c) => acc + (c.advanceMinor ?? 0), 0);
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Total Advances", value: formatPkr(totalAdvance), subtitle: "Customer prepayments" }
        ]
      };
    }
    case "credit_limit_exceptions": {
      const customers = await customerList(session);
      const exceptions = customers.filter(c => {
        const limit = c.creditLimitMinor ?? 0;
        const balance = c.balanceMinor ?? 0;
        return limit > 0 && balance > limit;
      });
      const rows = exceptions.map(c => ({
        code: c.code ?? "",
        customer: c.name,
        phone: c.phone ?? "",
        creditLimit: formatPkr(c.creditLimitMinor ?? 0),
        currentBalance: formatPkr(c.balanceMinor ?? 0),
        overLimit: formatPkr((c.balanceMinor ?? 0) - (c.creditLimitMinor ?? 0)),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Exceptions", value: rows.length, subtitle: "Customers over credit limit" }
        ]
      };
    }
    case "supplier_statements": {
      const suppliers = await supplierList(session);
      const rows = suppliers.map(s => ({
        code: s.code ?? "",
        supplier: s.name,
        phone: s.phone ?? "",
        totalPayable: formatPkr(s.balanceMinor ?? 0),
        totalPaid: formatPkr(s.openingBalanceMinor ?? 0),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Suppliers", value: rows.length, subtitle: "With activity" }
        ]
      };
    }
    case "supplier_payments": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Payment records available via export" }]
      };
    }
    case "cash_book": {
      return { 
        rows: [], 
        rowCount: 0,
        summary: [{ title: "Note", value: "Use CSV/PDF export", subtitle: "Cash book entries available via export" }]
      };
    }
    case "account_balances": {
      const accounts = await cashAccountList(session);
      const rows = accounts.map(a => ({
        code: a.code ?? "",
        account: a.name,
        kind: a.kind ?? "",
        openingBalance: formatPkr(a.openingBalanceMinor ?? 0),
        currentBalance: formatPkr(a.balanceMinor ?? 0),
      }));
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Accounts", value: rows.length, subtitle: "Cash & bank accounts" }
        ]
      };
    }
    case "user_activity": {
      const audit = await auditQuery(session, { limit: 5000 });
      const filtered = audit.items.filter(a => inRange(a.createdAt, from, to));
      const grouped: Record<string, Record<string, { count: number; first: string; last: string }>> = {};
      for (const a of filtered) {
        const user = a.userId ? `User ${a.userId}` : "System";
        if (!grouped[user]) grouped[user] = {};
        if (!grouped[user][a.action]) grouped[user][a.action] = { count: 0, first: a.createdAt, last: a.createdAt };
        grouped[user][a.action].count++;
        if (a.createdAt < grouped[user][a.action].first) grouped[user][a.action].first = a.createdAt;
        if (a.createdAt > grouped[user][a.action].last) grouped[user][a.action].last = a.createdAt;
      }
      const rows: Record<string, string>[] = [];
      for (const [user, actions] of Object.entries(grouped)) {
        for (const [action, v] of Object.entries(actions)) {
          rows.push({
            user,
            action,
            count: v.count.toLocaleString(),
            firstAction: new Date(v.first).toLocaleString(),
            lastAction: new Date(v.last).toLocaleString(),
          });
        }
      }
      return { 
        rows, 
        rowCount: rows.length,
        summary: [
          { title: "Activity Records", value: rows.length, subtitle: "User actions logged" }
        ]
      };
    }
    case "all_reports": {
      const [sales, purchases, expenses, returns, supReturns, deliveries, damages, receivablesData, aging, profit, invVal, custs, sups] = await Promise.all([
        saleList(session),
        purchaseList(session),
        expenseList(session, { fromDate: from, toDate: to, status: "posted", limit: 10000 }),
        saleReturnList(session, { limit: 10000 }),
        supplierReturnList(session),
        deliveryList(session, { limit: 10000 }),
        damageList(session, { limit: 10000 }),
        receivables(session),
        payableAging(session),
        profitSummary(session, from, to),
        stockValuation(session),
        customerList(session),
        supplierList(session),
      ]);

      const salesFiltered = sales.filter(s => inRange(s.saleDate, from, to));
      const purchasesFiltered = purchases.filter(p => inRange(p.purchaseDate, from, to));
      const returnsFiltered = returns.filter(r => inRange(r.returnDate, from, to));

      const totalSalesRevenue = salesFiltered.reduce((a, s) => a + s.totalMinor, 0);
      const totalSalesPaid = salesFiltered.reduce((a, s) => a + s.paidMinor + s.advanceUsedMinor, 0);
      const totalSalesDue = salesFiltered.reduce((a, s) => a + s.dueMinor, 0);
      const totalPurchases = purchasesFiltered.reduce((a, p) => a + p.totalMinor, 0);
      const totalExpenses = expenses.reduce((a, e) => a + e.amountMinor, 0);
      const totalStockValue = invVal.reduce((a, v) => a + v.valueMinor, 0);
      const totalCustomerDues = receivablesData.highBalance.reduce((a, c) => a + c.dueMinorTotal, 0);
      const totalSupplierPayables = aging.reduce((a, p) => a + p.dueMinor, 0);

      const sections: Record<string, string>[] = [
        { section: "SALES", metric: "Transactions", value: String(salesFiltered.length) },
        { section: "SALES", metric: "Net Revenue", value: formatPkr(totalSalesRevenue) },
        { section: "SALES", metric: "Paid", value: formatPkr(totalSalesPaid) },
        { section: "SALES", metric: "Outstanding", value: formatPkr(totalSalesDue) },
        { section: "PURCHASES", metric: "Transactions", value: String(purchasesFiltered.length) },
        { section: "PURCHASES", metric: "Net Purchases", value: formatPkr(totalPurchases) },
        { section: "EXPENSES", metric: "Records", value: String(expenses.length) },
        { section: "EXPENSES", metric: "Total Expenses", value: formatPkr(totalExpenses) },
        { section: "RETURNS", metric: "Customer Returns", value: String(returnsFiltered.length) },
        { section: "RETURNS", metric: "Supplier Returns", value: String(supReturns.length) },
        { section: "PROFIT & LOSS", metric: "Revenue", value: formatPkr(profit.revenueMinor) },
        { section: "PROFIT & LOSS", metric: "COGS", value: formatPkr(profit.cogsMinor) },
        { section: "PROFIT & LOSS", metric: "Gross Profit", value: formatPkr(profit.grossProfitMinor) },
        { section: "PROFIT & LOSS", metric: "Operational Profit", value: formatPkr(profit.operationalProfitMinor) },
        { section: "INVENTORY", metric: "Products", value: String(invVal.length) },
        { section: "INVENTORY", metric: "Stock Value", value: formatPkr(totalStockValue) },
        { section: "DELIVERIES", metric: "Deliveries", value: String(deliveries.length) },
        { section: "DELIVERIES", metric: "Damage Records", value: String(damages.length) },
        { section: "CUSTOMERS", metric: "Total Customers", value: String(custs.length) },
        { section: "CUSTOMERS", metric: "Outstanding Dues", value: formatPkr(totalCustomerDues) },
        { section: "SUPPLIERS", metric: "Total Suppliers", value: String(sups.length) },
        { section: "SUPPLIERS", metric: "Outstanding Payables", value: formatPkr(totalSupplierPayables) },
      ];

      return {
        rows: sections,
        rowCount: sections.length,
        summary: [
          { title: "Net Revenue", value: formatPkr(totalSalesRevenue), subtitle: `${salesFiltered.length} sales in period` },
          { title: "Operational Profit", value: formatPkr(profit.operationalProfitMinor), subtitle: `Margin: ${profit.revenueMinor > 0 ? ((profit.operationalProfitMinor / profit.revenueMinor) * 100).toFixed(1) + "%" : "N/A"}` },
          { title: "Stock Value", value: formatPkr(totalStockValue), subtitle: `${invVal.length} products` },
        ],
      };
    }
  }
  return { rows: [], rowCount: 0, summary: [] };
}

export function ReportsPage() {
  const { profile } = useSession();
  const { toast } = useToast();
  const session = profile?.sessionId ?? "";

  const [selected, setSelected] = React.useState<ReportType>("sales_summary");
  const [filterDraft, setFilterDraft] = React.useState<ReportFilterInput>({
    fromDate: dateNdaysAgo(30),
    toDate: todayIso(),
  });
  const [appliedFilter, setAppliedFilter] = React.useState<ReportFilterInput>({
    fromDate: dateNdaysAgo(30),
    toDate: todayIso(),
  });
  const [exporting, setExporting] = React.useState<"csv" | "pdf" | null>(null);

  const expenseCategoriesQuery = useQuery({
    queryKey: ["expenses", "categories", "reports"],
    queryFn: () => expenseCategoryList(session),
    enabled: !!session && selected === "expense_report",
  });
  const currencyQuery = useQuery({
    queryKey: ["settings", "shop.currency"],
    queryFn: () => settingsGet(session, "shop.currency"),
    enabled: !!session,
  });
  let currency = "PKR";
  if (currencyQuery.data) {
    try {
      const parsed = JSON.parse(currencyQuery.data) as unknown;
      if (typeof parsed === "string" && /^[A-Z]{3}$/i.test(parsed)) currency = parsed.toUpperCase();
    } catch {
      currency = "PKR";
    }
  }

  const meta = REPORTS.find((r) => r.id === selected)!;

  const reportQuery = useQuery({
    queryKey: ["reports", selected, appliedFilter, session, currency],
    queryFn: () => fetchReportData(selected, session, appliedFilter, currency),
    enabled: !!session && selected !== "all_reports",
  });

  const handleExport = async (format: "csv" | "pdf") => {
    if (!session || selected === "all_reports") return;
    setExporting(format);
    try {
      const result = await reportExport(session, selected, appliedFilter, format);
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

  const handleApply = () => {
    setAppliedFilter(filterDraft);
  };

  return (
    <div className="flex h-full flex-col bg-[#f8fafc]">
      <div className="border-b border-neutral-200 bg-white px-6 py-4">
        <h1 className="text-2xl font-semibold text-neutral-900">Reports</h1>
        <p className="text-sm text-neutral-500 mt-1">
          Sales, returns, purchases, expenses, profit and inventory summaries.
        </p>
      </div>

      <div className="flex-1 overflow-auto p-6 space-y-6">
        {/* Tabs */}
        <div className="flex w-full overflow-x-auto rounded-lg border border-neutral-200 bg-white shadow-sm p-1">
          {REPORTS.map((r) => (
            <button
              key={r.id}
              onClick={() => setSelected(r.id)}
              className={cn(
                "px-4 py-2 text-sm font-medium rounded-md whitespace-nowrap transition-colors",
                selected === r.id
                  ? "bg-blue-50 text-blue-700"
                  : "text-neutral-600 hover:bg-neutral-50 hover:text-neutral-900"
              )}
            >
              {r.title}
            </button>
          ))}
        </div>

        {selected === "all_reports" ? (
          <div className="rounded-lg border border-dashed border-neutral-300 bg-white p-12 text-center text-neutral-500">
            <BarChart3 className="mx-auto mb-4 h-8 w-8 text-neutral-400" />
            <h3 className="text-lg font-medium text-neutral-900 mb-2">Combined Business Report</h3>
            <p>Select a date range and click Apply to generate a comprehensive report covering all areas of your business.</p>
          </div>
        ) : (
          <>
            <FilterPanel
              filter={filterDraft}
              onFilterChange={setFilterDraft}
              onApply={handleApply}
              categories={selected === "expense_report" ? expenseCategoriesQuery.data ?? [] : undefined}
            />

            {/* Summary Cards */}
            {reportQuery.data?.summary && reportQuery.data.summary.length > 0 && (
              <div className="grid gap-4 sm:grid-cols-3">
                {reportQuery.data.summary.map((s, i) => (
                  <div key={i} className="rounded-lg border border-neutral-200 bg-white p-4 shadow-sm">
                    <p className="text-sm font-medium text-neutral-500">{s.title}</p>
                    <p className="mt-2 text-2xl font-bold text-neutral-900">{s.value}</p>
                    {s.subtitle && <p className="mt-1 text-xs text-neutral-400">{s.subtitle}</p>}
                  </div>
                ))}
              </div>
            )}

            {/* Detailed Table */}
            <div className="rounded-lg border border-neutral-200 bg-white shadow-sm overflow-hidden flex flex-col">
              <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-3 bg-neutral-50">
                <h3 className="font-medium text-neutral-900">Detailed Report</h3>
                <div className="flex gap-2">
                  <Button variant="outline" size="sm" onClick={() => handleExport("csv")} disabled={!!exporting || reportQuery.isLoading}>
                    <FileSpreadsheet className="mr-2 h-4 w-4" /> CSV
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => handleExport("pdf")} disabled={!!exporting || reportQuery.isLoading}>
                    <FileText className="mr-2 h-4 w-4" /> Print / PDF
                  </Button>
                </div>
              </div>
              <div className="overflow-x-auto min-h-[300px]">
                {reportQuery.isLoading ? (
                  <div className="flex h-64 items-center justify-center text-neutral-500">
                    <Loader2 className="mr-2 h-5 w-5 animate-spin" /> Loading data...
                  </div>
                ) : reportQuery.isError ? (
                  <div className="flex h-64 items-center justify-center text-red-500">
                    Failed to load report data.
                  </div>
                ) : reportQuery.data?.rows.length === 0 ? (
                  <div className="flex h-64 items-center justify-center text-neutral-500 font-medium">
                    No data in this period.
                  </div>
                ) : (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        {meta.columns.map((col) => (
                          <TableHead key={col.key} className={col.alignRight ? "text-right" : ""}>
                            {col.header}
                          </TableHead>
                        ))}
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {reportQuery.data?.rows.map((row, i) => (
                        <TableRow key={i}>
                          {meta.columns.map((col) => (
                            <TableCell key={col.key} className={col.alignRight ? "text-right tabular-nums" : ""}>
                              {row[col.key]}
                            </TableCell>
                          ))}
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                )}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
