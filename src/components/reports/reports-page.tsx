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
  ReceiptText,
  Undo2,
  ClipboardList,
  BarChart3,
  ChevronDown,
  Printer,
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
  customerList,
  supplierList,
  settingsGet,
  shopLogoGet,
  productList,
  type ReportFilterInput,
} from "@/lib/tauri/api";
import { formatCurrency, formatPkr, todayIso } from "@/lib/format";
import {
  ReportPrintPreviewDialog,
  type ReportPrintColumn,
  type ReportPrintSummary,
} from "./report-print";

// ══════════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════════

type ReportType =
  | "all_reports"
  | "sales_pos"
  | "sales_history"
  | "returns_exchanges"
  | "products"
  | "inventory"
  | "purchases"
  | "customers"
  | "customer_dues"
  | "suppliers"
  | "supplier_dues"
  | "deliveries"
  | "expenses"
  | "invoices";

type ReportMeta = {
  id: ReportType;
  label: string;
  icon: React.ElementType;
  columns: { header: string; key: string; alignRight?: boolean }[];
};

const DROPDOWN_REPORTS: ReportMeta[] = [
  { id: "all_reports",        label: "All Reports",          icon: BarChart3,      columns: [] },
  { id: "sales_pos",          label: "Sales / POS",          icon: ShoppingCart,   columns: [
    { header: "Date", key: "date" }, { header: "Sale #", key: "saleNumber" },
    { header: "Customer", key: "customer" }, { header: "Items", key: "items", alignRight: true },
    { header: "Total", key: "total", alignRight: true }, { header: "Paid", key: "paid", alignRight: true },
    { header: "Due", key: "due", alignRight: true }, { header: "Status", key: "status" },
  ]},
  { id: "sales_history",      label: "Sales History",        icon: ClipboardList,  columns: [
    { header: "Date", key: "date" }, { header: "Sale #", key: "saleNumber" },
    { header: "Customer", key: "customer" }, { header: "Total", key: "total", alignRight: true },
    { header: "Paid", key: "paid", alignRight: true }, { header: "Due", key: "due", alignRight: true },
    { header: "Status", key: "status" },
  ]},
  { id: "returns_exchanges",  label: "Returns & Exchanges",  icon: Undo2,          columns: [
    { header: "Date", key: "date" }, { header: "Type", key: "type" },
    { header: "Return #", key: "returnNumber" }, { header: "Ref #", key: "refNumber" },
    { header: "Amount", key: "amount", alignRight: true }, { header: "Status", key: "status" },
  ]},
  { id: "products",           label: "Products",             icon: Package,        columns: [
    { header: "Article", key: "article" }, { header: "Product", key: "product" },
    { header: "Category", key: "category" }, { header: "Cost", key: "cost", alignRight: true },
    { header: "Price", key: "price", alignRight: true }, { header: "Stock", key: "stock", alignRight: true },
    { header: "Status", key: "status" },
  ]},
  { id: "inventory",          label: "Inventory",            icon: Package,        columns: [
    { header: "Article", key: "article" }, { header: "Product", key: "product" },
    { header: "Qty", key: "qty", alignRight: true }, { header: "Unit Cost", key: "unitCost", alignRight: true },
    { header: "Value", key: "value", alignRight: true },
  ]},
  { id: "purchases",          label: "Purchases",            icon: Truck,          columns: [
    { header: "Date", key: "date" }, { header: "Purchase #", key: "purchaseNumber" },
    { header: "Supplier", key: "supplier" }, { header: "Total", key: "total", alignRight: true },
    { header: "Paid", key: "paid", alignRight: true }, { header: "Due", key: "due", alignRight: true },
    { header: "Status", key: "status" },
  ]},
  { id: "customers",          label: "Customers",            icon: Users,          columns: [
    { header: "Code", key: "code" }, { header: "Customer", key: "customer" },
    { header: "Phone", key: "phone" }, { header: "Total Due", key: "totalDue", alignRight: true },
    { header: "Total Paid", key: "totalPaid", alignRight: true },
  ]},
  { id: "customer_dues",      label: "Customer Dues",        icon: Users,          columns: [
    { header: "Customer", key: "customer" }, { header: "Phone", key: "phone" },
    { header: "Balance", key: "balance", alignRight: true }, { header: "Due", key: "due", alignRight: true },
    { header: "Overdue", key: "overdue", alignRight: true },
  ]},
  { id: "suppliers",          label: "Suppliers",            icon: Truck,          columns: [
    { header: "Code", key: "code" }, { header: "Supplier", key: "supplier" },
    { header: "Phone", key: "phone" }, { header: "Total Payable", key: "totalPayable", alignRight: true },
  ]},
  { id: "supplier_dues",      label: "Supplier Dues",        icon: Truck,          columns: [
    { header: "Supplier", key: "supplier" }, { header: "Purchase #", key: "purchaseNumber" },
    { header: "Invoice Date", key: "invoiceDate" }, { header: "Due", key: "due", alignRight: true },
    { header: "Days", key: "days", alignRight: true },
  ]},
  { id: "deliveries",         label: "Deliveries",           icon: Truck,          columns: [
    { header: "Date", key: "date" }, { header: "Type", key: "type" },
    { header: "Ref #", key: "refNumber" }, { header: "Details", key: "details" },
    { header: "Status", key: "status" },
  ]},
  { id: "expenses",           label: "Expenses",             icon: ReceiptText,    columns: [
    { header: "Date", key: "date" }, { header: "Expense #", key: "expenseNumber" },
    { header: "Category", key: "category" }, { header: "Note", key: "note" },
    { header: "Amount", key: "amount", alignRight: true }, { header: "Status", key: "status" },
  ]},
  { id: "invoices",           label: "Invoices",             icon: FileText,       columns: [
    { header: "Invoice #", key: "invoiceNumber" }, { header: "Date", key: "date" },
    { header: "Customer", key: "customer" }, { header: "Total", key: "total", alignRight: true },
    { header: "Paid", key: "paid", alignRight: true }, { header: "Due", key: "due", alignRight: true },
    { header: "Status", key: "status" },
  ]},
];

// ══════════════════════════════════════════════════════════════════════
// Date helpers
// ══════════════════════════════════════════════════════════════════════

function dateNdaysAgo(n: number): string { const d = new Date(); d.setDate(d.getDate() - n); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; }
function startOfThisWeek(): string { const d = new Date(); const day = d.getDay(); d.setDate(d.getDate() - day + (day === 0 ? -6 : 1)); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; }
function startOfLastWeek(): string { const d = new Date(); const day = d.getDay(); d.setDate(d.getDate() - day + (day === 0 ? -13 : -6)); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; }
function endOfLastWeek(): string { const d = new Date(); const day = d.getDay(); d.setDate(d.getDate() - day - 1 + (day === 0 ? 0 : 0)); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; }
function startOfYear(): string { return `${new Date().getFullYear()}-01-01`; }
function startOfLastYear(): string { return `${new Date().getFullYear() - 1}-01-01`; }
function endOfLastYear(): string { return `${new Date().getFullYear() - 1}-12-31`; }
function inRange(dateStr: string, from: string | null, to: string | null): boolean {
  if (!dateStr) return false;
  const d = dateStr.slice(0, 10);
  if (from && d < from) return false;
  if (to && d > to) return false;
  return true;
}

function settingString(raw: string | null | undefined): string | null {
  if (raw == null) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    return typeof parsed === "string" && parsed.trim() ? parsed : null;
  } catch {
    return raw.trim() ? raw : null;
  }
}

function reportPeriod(filter: ReportFilterInput): string {
  const from = filter.fromDate || null;
  const to = filter.toDate || null;
  if (!from && !to) return "All time";
  if (!from) return `Up to ${to}`;
  if (!to) return `From ${from}`;
  return from === to ? from : `${from} to ${to}`;
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

// ══════════════════════════════════════════════════════════════════════
// FilterPanel
// ══════════════════════════════════════════════════════════════════════

function FilterPanel({
  filter, onFilterChange, onApply, categories,
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
          onChange={(e) => onFilterChange({ ...filter, categoryId: e.target.value ? Number(e.target.value) : null })}
        >
          <option value="">All expense categories</option>
          {categories.map((c) => <option key={c.id} value={c.id}>{c.name}{c.isActive ? "" : " (archived)"}</option>)}
        </select>
      )}
      <div className="flex items-center gap-2">
        <Input type="date" value={filter.fromDate ?? ""} onChange={(e) => { onFilterChange({ ...filter, fromDate: e.target.value || null }); setActivePreset(-1); }} className="w-40" />
        <span className="text-neutral-400">→</span>
        <Input type="date" value={filter.toDate ?? ""} onChange={(e) => { onFilterChange({ ...filter, toDate: e.target.value || null }); setActivePreset(-1); }} className="w-40" />
      </div>
      <div className="flex flex-wrap items-center gap-1.5 ml-2">
        {PRESETS.map((p, i) => (
          <Button key={p.label} variant="outline" className={cn("text-xs px-3 h-9", activePreset === i && p.label !== "Clear" ? "border-blue-600 text-blue-600 bg-blue-50" : "", p.label === "Clear" ? "text-red-600 hover:text-red-700" : "")}
            onClick={() => { setActivePreset(i); onFilterChange({ ...filter, fromDate: p.from, toDate: p.to }); }}>
            {p.label}
          </Button>
        ))}
      </div>
      <Button onClick={onApply} className="ml-auto bg-blue-600 hover:bg-blue-700">Apply</Button>
    </div>
  );
}

// ══════════════════════════════════════════════════════════════════════
// Fetch + transform logic per report type
// ══════════════════════════════════════════════════════════════════════

async function fetchReportData(
  reportType: ReportType,
  session: string,
  filter: ReportFilterInput,
  currency = "PKR",
): Promise<{ rows: Record<string, string>[]; summary: Record<string, string | number>[]; rowCount: number }> {
  const from = filter.fromDate ?? null;
  const to = filter.toDate ?? null;

  switch (reportType) {
    case "sales_pos":
    case "sales_history": {
      const sales = await saleList(session);
      const filtered = sales.filter((s) => inRange(s.saleDate, from, to));
      const totalNet = filtered.reduce((acc, s) => acc + s.totalMinor, 0);
      const rows = filtered.map((s) => ({
        date: s.saleDate,
        saleNumber: s.saleNumber ?? "",
        customer: s.customerName ?? "Walk-in",
        items: String(s.items?.length ?? 0),
        total: formatPkr(s.totalMinor),
        paid: formatPkr(s.paidMinor + s.advanceUsedMinor),
        due: formatPkr(s.dueMinor),
        status: s.status,
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Net Sales", value: formatPkr(totalNet), subtitle: `${rows.length} transactions` },
        { title: "Total Paid", value: formatPkr(filtered.reduce((a, s) => a + s.paidMinor + s.advanceUsedMinor, 0)), subtitle: "Collected" },
        { title: "Outstanding", value: formatPkr(filtered.reduce((a, s) => a + s.dueMinor, 0)), subtitle: "Unpaid" },
      ]};
    }
    case "returns_exchanges": {
      const sReturns = await saleReturnList(session, { limit: 10000 });
      const supReturns = await supplierReturnList(session);
      const sFiltered = sReturns.filter(r => inRange(r.returnDate, from, to));
      const supFiltered = supReturns.filter(r => inRange(r.returnDate, from, to));
      const rows = [
        ...sFiltered.map(r => ({ date: r.returnDate, type: "Customer Return", returnNumber: r.returnNumber ?? "", refNumber: r.saleNumber ?? "", amount: formatPkr(r.totalRefundMinor), status: r.status })),
        ...supFiltered.map(r => ({ date: r.returnDate, type: "Supplier Return", returnNumber: r.returnNumber ?? "", refNumber: r.purchaseId ? String(r.purchaseId) : "", amount: formatPkr(r.totalMinor), status: r.status })),
      ].sort((a, b) => b.date.localeCompare(a.date));
      return { rows, rowCount: rows.length, summary: [
        { title: "Customer Returns", value: sFiltered.length, subtitle: "Sales returns" },
        { title: "Supplier Returns", value: supFiltered.length, subtitle: "Purchase returns" },
      ]};
    }
    case "products": {
      const products = await productList(session);
      const rows = products.filter(p => p.isActive && !p.archivedAt).map(p => ({
        article: p.articleNumber,
        product: p.name,
        category: p.category,
        cost: formatPkr(p.costMinor ?? 0),
        price: formatPkr(p.salePriceMinor),
        stock: p.trackStock ? "—" : "N/A",
        status: p.isActive ? "Active" : "Inactive",
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Total Products", value: rows.length, subtitle: "Active catalogue" },
      ]};
    }
    case "inventory": {
      const items = await stockValuation(session);
      const totalVal = items.reduce((acc, v) => acc + v.valueMinor, 0);
      const rows = items.map(v => ({
        article: v.articleNumber, product: v.productName,
        qty: v.sellableQty.toLocaleString(), unitCost: formatPkr(v.unitCostMinor),
        value: formatPkr(v.valueMinor),
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Stock Value", value: formatPkr(totalVal), subtitle: `${rows.length} products` },
      ]};
    }
    case "purchases": {
      const purchases = await purchaseList(session);
      const filtered = purchases.filter(p => inRange(p.purchaseDate, from, to));
      const total = filtered.reduce((acc, p) => acc + p.totalMinor, 0);
      const rows = filtered.map(p => ({
        date: p.purchaseDate, purchaseNumber: p.purchaseNumber ?? "", supplier: p.supplierName ?? "",
        total: formatPkr(p.totalMinor), paid: formatPkr(p.paidMinor), due: formatPkr(p.dueMinor), status: p.status,
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Net Purchases", value: formatPkr(total), subtitle: `${rows.length} purchases` },
      ]};
    }
    case "customers": {
      const data = await receivables(session);
      const rows = data.highBalance.map(c => ({
        code: "", customer: c.customerName, phone: c.phone ?? "",
        totalDue: formatPkr(c.dueMinorTotal), totalPaid: formatPkr(c.balanceMinor),
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Customers", value: rows.length, subtitle: "With outstanding balances" },
      ]};
    }
    case "customer_dues": {
      const data = await receivables(session);
      const totalDue = data.highBalance.reduce((acc, c) => acc + c.dueMinorTotal, 0);
      const rows = data.highBalance.map(c => ({
        customer: c.customerName, phone: c.phone ?? "",
        balance: formatPkr(c.balanceMinor), due: formatPkr(c.dueMinorTotal), overdue: formatPkr(c.overdueMinorTotal),
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Total Dues", value: formatPkr(totalDue), subtitle: "Outstanding balances" },
      ]};
    }
    case "suppliers": {
      const suppliers = await supplierList(session);
      const rows = suppliers.map(s => ({
        code: s.code ?? "", supplier: s.name, phone: s.phone ?? "",
        totalPayable: formatPkr(s.balanceMinor ?? 0),
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Suppliers", value: rows.length, subtitle: "Registered suppliers" },
      ]};
    }
    case "supplier_dues": {
      const data = await payableAging(session);
      const totalDue = data.reduce((acc, p) => acc + p.dueMinor, 0);
      const rows = data.map(p => ({
        supplier: p.supplierName, purchaseNumber: p.purchaseNumber ?? "",
        invoiceDate: p.invoiceDate, due: formatPkr(p.dueMinor), days: p.ageDays.toLocaleString(),
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Total Payables", value: formatPkr(totalDue), subtitle: "Outstanding to suppliers" },
      ]};
    }
    case "deliveries": {
      const deliveries = await deliveryList(session, { limit: 10000 });
      const damages = await damageList(session, { limit: 10000 });
      const delFiltered = deliveries.filter(d => inRange(d.scheduledAt || "", from, to));
      const damFiltered = damages.filter(d => inRange(d.damageDate, from, to));
      const rows = [
        ...delFiltered.map(d => ({ date: d.scheduledAt || "", type: "Delivery", refNumber: d.deliveryNumber ?? "", details: `Status: ${d.status}`, status: d.status })),
        ...damFiltered.map(d => ({ date: d.createdAt, type: "Damage", refNumber: d.productName, details: d.reason ?? "", status: d.status })),
      ].sort((a, b) => b.date.localeCompare(a.date));
      return { rows, rowCount: rows.length, summary: [
        { title: "Deliveries", value: delFiltered.length, subtitle: "In period" },
        { title: "Damage Records", value: damFiltered.length, subtitle: "In period" },
      ]};
    }
    case "expenses": {
      const expenses = await expenseList(session, { fromDate: from, toDate: to, categoryId: filter.categoryId ?? null, status: "posted", limit: 10000 });
      const totalExp = expenses.reduce((acc, e) => acc + e.amountMinor, 0);
      const rows = expenses.map(e => ({
        date: e.expenseDate, expenseNumber: e.expenseNumber ?? "", category: e.categoryName,
        note: e.description || "", amount: formatCurrency(e.amountMinor, currency), status: e.status,
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Total Expenses", value: formatCurrency(totalExp, currency), subtitle: `${rows.length} records` },
      ]};
    }
    case "invoices": {
      const sales = await saleList(session);
      const filtered = sales.filter(s => inRange(s.saleDate, from, to) && s.status === "confirmed");
      const totalInv = filtered.reduce((a, s) => a + s.totalMinor, 0);
      const rows = filtered.map(s => ({
        invoiceNumber: s.saleNumber ?? "", date: s.saleDate, customer: s.customerName ?? "Walk-in",
        total: formatPkr(s.totalMinor), paid: formatPkr(s.paidMinor + s.advanceUsedMinor),
        due: formatPkr(s.dueMinor), status: s.status,
      }));
      return { rows, rowCount: rows.length, summary: [
        { title: "Invoices", value: rows.length, subtitle: "Confirmed sales" },
        { title: "Total Invoiced", value: formatPkr(totalInv), subtitle: "Gross value" },
      ]};
    }
    case "all_reports": {
      const [sales, purchases, expenses, returns, supReturns, deliveries, damages, receivablesData, aging, profit, invVal, custs, sups] = await Promise.all([
        saleList(session), purchaseList(session),
        expenseList(session, { fromDate: from, toDate: to, status: "posted", limit: 10000 }),
        saleReturnList(session, { limit: 10000 }), supplierReturnList(session),
        deliveryList(session, { limit: 10000 }), damageList(session, { limit: 10000 }),
        receivables(session), payableAging(session),
        profitSummary(session, from, to), stockValuation(session),
        customerList(session), supplierList(session),
      ]);
      const salesF = sales.filter(s => inRange(s.saleDate, from, to));
      const purchasesF = purchases.filter(p => inRange(p.purchaseDate, from, to));
      const returnsF = returns.filter(r => inRange(r.returnDate, from, to));
      const supplierReturnsF = supReturns.filter(r => inRange(r.returnDate, from, to));
      const deliveriesF = deliveries.filter(d => inRange(d.scheduledAt || d.createdAt, from, to));
      const damagesF = damages.filter(d => inRange(d.damageDate, from, to));
      const totalSalesRevenue = salesF.reduce((a, s) => a + s.totalMinor, 0);
      const totalSalesPaid = salesF.reduce((a, s) => a + s.paidMinor + s.advanceUsedMinor, 0);
      const totalSalesDue = salesF.reduce((a, s) => a + s.dueMinor, 0);
      const totalPurchases = purchasesF.reduce((a, p) => a + p.totalMinor, 0);
      const totalExpenses = expenses.reduce((a, e) => a + e.amountMinor, 0);
      const totalStockValue = invVal.reduce((a, v) => a + v.valueMinor, 0);
      const totalCustomerDues = receivablesData.highBalance.reduce((a, c) => a + c.dueMinorTotal, 0);
      const totalSupplierPayables = aging.reduce((a, p) => a + p.dueMinor, 0);
      const sections: Record<string, string>[] = [
        { section: "SALES", metric: "Transactions", value: String(salesF.length) },
        { section: "SALES", metric: "Net Revenue", value: formatPkr(totalSalesRevenue) },
        { section: "SALES", metric: "Paid", value: formatPkr(totalSalesPaid) },
        { section: "SALES", metric: "Outstanding", value: formatPkr(totalSalesDue) },
        { section: "PURCHASES", metric: "Transactions", value: String(purchasesF.length) },
        { section: "PURCHASES", metric: "Net Purchases", value: formatPkr(totalPurchases) },
        { section: "EXPENSES", metric: "Records", value: String(expenses.length) },
        { section: "EXPENSES", metric: "Total Expenses", value: formatPkr(totalExpenses) },
        { section: "RETURNS", metric: "Customer Returns", value: String(returnsF.length) },
        { section: "RETURNS", metric: "Supplier Returns", value: String(supplierReturnsF.length) },
        { section: "PROFIT & LOSS", metric: "Revenue", value: formatPkr(profit.revenueMinor) },
        { section: "PROFIT & LOSS", metric: "COGS", value: formatPkr(profit.cogsMinor) },
        { section: "PROFIT & LOSS", metric: "Gross Profit", value: formatPkr(profit.grossProfitMinor) },
        { section: "PROFIT & LOSS", metric: "Operational Profit", value: formatPkr(profit.operationalProfitMinor) },
        { section: "INVENTORY", metric: "Products", value: String(invVal.length) },
        { section: "INVENTORY", metric: "Stock Value", value: formatPkr(totalStockValue) },
        { section: "DELIVERIES", metric: "Deliveries", value: String(deliveriesF.length) },
        { section: "DELIVERIES", metric: "Damage Records", value: String(damagesF.length) },
        { section: "CUSTOMERS", metric: "Total Customers", value: String(custs.length) },
        { section: "CUSTOMERS", metric: "Outstanding Dues", value: formatPkr(totalCustomerDues) },
        { section: "SUPPLIERS", metric: "Total Suppliers", value: String(sups.length) },
        { section: "SUPPLIERS", metric: "Outstanding Payables", value: formatPkr(totalSupplierPayables) },
      ];
      return {
        rows: sections, rowCount: sections.length,
        summary: [
          { title: "Net Revenue", value: formatPkr(totalSalesRevenue), subtitle: `${salesF.length} sales in period` },
          { title: "Operational Profit", value: formatPkr(profit.operationalProfitMinor), subtitle: `Margin: ${profit.revenueMinor > 0 ? ((profit.operationalProfitMinor / profit.revenueMinor) * 100).toFixed(1) + "%" : "N/A"}` },
          { title: "Stock Value", value: formatPkr(totalStockValue), subtitle: `${invVal.length} products` },
        ],
      };
    }
  }
  return { rows: [], rowCount: 0, summary: [] };
}

function toBackendReportType(frontendType: ReportType): string {
  switch (frontendType) {
    case "sales_pos":
    case "sales_history":
    case "invoices":
      return "sales_summary";
    case "returns_exchanges":
      return "returns_report";
    case "products":
    case "inventory":
      return "stock_valuation";
    case "purchases":
      return "purchase_report";
    case "customers":
      return "customer_statements";
    case "customer_dues":
      return "customer_dues";
    case "suppliers":
      return "supplier_statements";
    case "supplier_dues":
      return "supplier_payables";
    case "deliveries":
      return "deliveries_report";
    case "expenses":
      return "expense_report";
    case "all_reports":
      return "profit_loss";
    default:
      return "sales_summary";
  }
}

// ══════════════════════════════════════════════════════════════════════
// ReportsPage
// ══════════════════════════════════════════════════════════════════════

export function ReportsPage() {
  const { profile } = useSession();
  const { toast } = useToast();
  const session = profile?.sessionId ?? "";

  const [selected, setSelected] = React.useState<ReportType>("all_reports");
  const [dropdownOpen, setDropdownOpen] = React.useState(false);
  const [filterDraft, setFilterDraft] = React.useState<ReportFilterInput>({
    fromDate: dateNdaysAgo(30), toDate: todayIso(),
  });
  const [appliedFilter, setAppliedFilter] = React.useState<ReportFilterInput>({
    fromDate: dateNdaysAgo(30), toDate: todayIso(),
  });
  const [exporting, setExporting] = React.useState<"csv" | "pdf" | null>(null);
  const [printOpen, setPrintOpen] = React.useState(false);

  const expenseCategoriesQuery = useQuery({
    queryKey: ["expenses", "categories", "reports"],
    queryFn: () => expenseCategoryList(session),
    enabled: !!session && selected === "expenses",
  });
  const currencyQuery = useQuery({
    queryKey: ["settings", "shop.currency"],
    queryFn: () => settingsGet(session, "shop.currency"),
    enabled: !!session,
  });
  const shopNameQuery = useQuery({
    queryKey: ["reports", "shop.name"],
    queryFn: () => settingsGet(session, "shop.name"),
    enabled: !!session,
  });
  const shopAddressQuery = useQuery({
    queryKey: ["reports", "shop.address"],
    queryFn: () => settingsGet(session, "shop.address"),
    enabled: !!session,
  });
  const shopPhoneQuery = useQuery({
    queryKey: ["reports", "shop.phone"],
    queryFn: () => settingsGet(session, "shop.phone"),
    enabled: !!session,
  });
  const shopLogoQuery = useQuery({
    queryKey: ["branding", "logo"],
    queryFn: () => shopLogoGet(session),
    enabled: !!session,
  });
  let currency = "PKR";
  if (currencyQuery.data) {
    try { const p = JSON.parse(currencyQuery.data) as unknown; if (typeof p === "string" && /^[A-Z]{3}$/i.test(p)) currency = p.toUpperCase(); } catch { currency = "PKR"; }
  }

  const meta = DROPDOWN_REPORTS.find(r => r.id === selected)!;

  const reportQuery = useQuery({
    queryKey: ["reports", selected, appliedFilter, session, currency],
    queryFn: () => fetchReportData(selected, session, appliedFilter, currency),
    enabled: !!session,
  });

  const outputColumns: ReportPrintColumn[] = selected === "all_reports"
    ? [
        { header: "Section", key: "section" },
        { header: "Metric", key: "metric" },
        { header: "Value", key: "value", alignRight: true },
      ]
    : meta.columns;
  const outputRows = reportQuery.data?.rows ?? [];
  const outputSummary: ReportPrintSummary[] = (reportQuery.data?.summary ?? []).map((item) => ({
    label: String(item.title ?? "Summary"),
    value: String(item.value ?? ""),
  }));
  const period = reportPeriod(appliedFilter);
  const branding = {
    name: settingString(shopNameQuery.data) ?? "Furniture Shop",
    address: settingString(shopAddressQuery.data),
    phone: settingString(shopPhoneQuery.data),
    logo: shopLogoQuery.data ?? null,
  };
  const brandingLoading = shopNameQuery.isLoading || shopAddressQuery.isLoading || shopPhoneQuery.isLoading || shopLogoQuery.isLoading;

  const handleExport = async (format: "csv" | "pdf") => {
    if (!session || !reportQuery.data) return;
    setExporting(format);
    try {
      const backendType = toBackendReportType(selected);
      const result = await reportExport(session, backendType, appliedFilter, format, {
        title: meta.label,
        columns: outputColumns.map((column) => ({ header: column.header, alignRight: !!column.alignRight })),
        rows: outputRows.map((row) => outputColumns.map((column) => String(row[column.key] ?? ""))),
        summary: outputSummary,
      });
      await openFile(session, result.reportPath);
      toast({ title: `${format.toUpperCase()} exported`, description: `${result.rowCount} rows generated.` });
    } catch (err) {
      toast({ title: "Export failed", description: err instanceof Error ? err.message : String(err) });
    } finally { setExporting(null); }
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
        {/* Report Type Dropdown */}
        <div className="flex items-center gap-4">
          <div className="relative">
            <button
              type="button"
              onClick={() => setDropdownOpen(!dropdownOpen)}
              className="flex items-center gap-2 rounded-lg border border-neutral-200 bg-white px-4 py-2.5 text-sm font-medium text-neutral-900 shadow-sm hover:bg-neutral-50 transition-colors min-w-[220px]"
            >
              <meta.icon className="h-4 w-4 text-blue-600" />
              <span className="flex-1 text-left">{meta.label}</span>
              <ChevronDown className={cn("h-4 w-4 text-neutral-400 transition-transform", dropdownOpen && "rotate-180")} />
            </button>
            {dropdownOpen && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setDropdownOpen(false)} />
                <div className="absolute z-50 mt-1 w-[260px] rounded-lg border border-neutral-200 bg-white shadow-lg py-1 max-h-[400px] overflow-y-auto">
                  {DROPDOWN_REPORTS.map(r => (
                    <button
                      key={r.id}
                      type="button"
                      onClick={() => { setSelected(r.id); setDropdownOpen(false); }}
                      className={cn(
                        "flex w-full items-center gap-2.5 px-3 py-2 text-sm text-left transition-colors",
                        selected === r.id ? "bg-blue-50 text-blue-700 font-medium" : "text-neutral-700 hover:bg-neutral-50"
                      )}
                    >
                      <r.icon className="h-4 w-4 shrink-0" />
                      <span>{r.label}</span>
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>

        {/* Filter Panel */}
        <FilterPanel
          filter={filterDraft}
          onFilterChange={setFilterDraft}
          onApply={() => setAppliedFilter(filterDraft)}
          categories={selected === "expenses" ? expenseCategoriesQuery.data ?? [] : undefined}
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
            <h3 className="font-medium text-neutral-900">{meta.label}</h3>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={() => handleExport("csv")} disabled={!!exporting || reportQuery.isLoading}>
                <FileSpreadsheet className="mr-2 h-4 w-4" /> {exporting === "csv" ? "Exporting..." : "Export CSV"}
              </Button>
              <Button variant="outline" size="sm" onClick={() => handleExport("pdf")} disabled={!!exporting || reportQuery.isLoading}>
                <FileText className="mr-2 h-4 w-4" /> {exporting === "pdf" ? "Exporting..." : "Export PDF"}
              </Button>
              <Button variant="outline" size="sm" onClick={() => setPrintOpen(true)} disabled={!!exporting || reportQuery.isLoading || brandingLoading}>
                <Printer className="mr-2 h-4 w-4" /> Print Report
              </Button>
            </div>
          </div>
          <div className="overflow-x-auto min-h-[300px]">
            {reportQuery.isLoading ? (
              <div className="flex h-64 items-center justify-center text-neutral-500">
                <Loader2 className="mr-2 h-5 w-5 animate-spin" /> Loading data...
              </div>
            ) : reportQuery.isError ? (
              <div className="flex h-64 items-center justify-center text-red-500">Failed to load report data.</div>
            ) : reportQuery.data?.rows.length === 0 ? (
              <div className="flex h-64 items-center justify-center text-neutral-500 font-medium">No data in this period.</div>
            ) : selected === "all_reports" ? (
              <AllReportsTable rows={reportQuery.data?.rows ?? []} />
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    {meta.columns.map(col => (
                      <TableHead key={col.key} className={col.alignRight ? "text-right" : ""}>{col.header}</TableHead>
                    ))}
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {reportQuery.data?.rows.map((row, i) => (
                    <TableRow key={i}>
                      {meta.columns.map(col => (
                        <TableCell key={col.key} className={col.alignRight ? "text-right tabular-nums" : ""}>{row[col.key]}</TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        </div>
      </div>
      {printOpen && (
        <ReportPrintPreviewDialog
          session={session}
          title={meta.label}
          period={period}
          columns={outputColumns}
          rows={outputRows}
          summary={outputSummary}
          branding={branding}
          onClose={() => setPrintOpen(false)}
        />
      )}
    </div>
  );
}

// ── All Reports sectioned table ──────────────────────────────────────

function AllReportsTable({ rows }: { rows: Record<string, string>[] }) {
  const grouped: Record<string, Record<string, string>[]> = {};
  for (const r of rows) {
    const sec = r.section ?? "";
    if (!grouped[sec]) grouped[sec] = [];
    grouped[sec].push(r);
  }
  return (
    <div className="p-4 space-y-6">
      {Object.entries(grouped).map(([section, items]) => (
        <div key={section}>
          <h4 className="text-sm font-bold text-[#1c4a2e] uppercase tracking-wide mb-2 pb-1 border-b-2 border-[#1c4a2e]">{section}</h4>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[50%]">Metric</TableHead>
                <TableHead className="text-right">Value</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {items.map((item, i) => (
                <TableRow key={i}>
                  <TableCell className="font-medium">{item.metric}</TableCell>
                  <TableCell className="text-right tabular-nums font-semibold">{item.value}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      ))}
    </div>
  );
}
