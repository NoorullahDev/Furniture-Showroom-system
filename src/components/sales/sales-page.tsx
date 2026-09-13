"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Banknote,
  CheckCircle2,
  ClipboardList,
  Copy,
  Eye,
  History,
  Layers,
  Loader2,
  Pencil,
  Plus,
  Printer,
  Search,
  TriangleAlert,
  Upload,
  UserPlus,
  Wallet,
  X,
} from "lucide-react";

import { convertFileSrc } from "@tauri-apps/api/core";
import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MoneyInput } from "@/components/ui/money-input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { StoredImage } from "@/components/catalogue/stored-image";
import { loadPrintSettings, type InvoicePrintSettings } from "@/components/invoices/invoice-settings";
import { printInvoiceA4 } from "@/components/invoices/invoice-print";
import {
  bundleAvailability,
  bundleCreate,
  bundleList,
  bundleUpdate,
  cashAccountList,
  customerCreate,
  customerGet,
  customerLedger,
  customerList,
  customerReceiptCreate,
  customerReceiptList,
  customerReceiptPreview,
  customerReceiptVoid,
  customerStatement,
  customerUpdate,
  locationList,
  paymentMethodList,
  productList,
  receivables,
  saleConfirm,
  saleCreate,
  saleDelete,
  saleDraftDelete,
  saleEdit,
  saleGet,
  saleList,
  settingsGet,
  shopLogoGet,
  stockBalanceList,
  type BundleDto,
  type BundleItemInput,
  type CashAccountDto,
  type CustomerDto,
  type CustomerInput,
  type CustomerLedgerEntryDto,
  type CustomerPaymentDto,
  type CustomerReceiptAllocationInput,
  type CustomerStatementDto,
  type LocationDto,
  type PaymentMethodDto,
  type ProductListItemDto,
  type ReceiptPreviewDto,
  type ReceivableCustomerDto,
  type ReceivableSaleDto,
  type ReceivablesDto,
  type SaleDto,
  type StockBalanceDto,
} from "@/lib/tauri/api";
import { formatDateTime, formatPkr, todayIso } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";
import { takeDashboardTarget } from "@/lib/dashboard-navigation";


type CartLine = {
  key: string;
  productId?: number;
  bundleId?: number;
  name: string;
  article: string;
  unitPriceMinor: number;
  quantity: number;
};

const SALES_LEDGER_LABELS: Record<string, string> = {
  opening_balance: "Opening balance",
  sale: "Sale",
  payment: "Payment received",
  advance_used: "Advance used",
  advance_restore: "Advance restored",
  sale_cancellation: "Sale cancelled",
  payment_refund: "Payment refund",
};

function saleReminderText(s: ReceivableSaleDto): string {
  const invoice = s.saleNumber ?? `#${s.saleId}`;
  const due = s.dueDate ?? s.saleDate;
  return `Dear ${s.customerName}, a friendly reminder that invoice ${invoice} dated ${s.saleDate} (due ${due}) still has ${formatPkr(s.dueMinor)} outstanding against a total of ${formatPkr(s.totalMinor)}. Please arrange payment at your earliest convenience. Thank you.`;
}

function customerReminderText(c: ReceivableCustomerDto): string {
  const overdue = c.overdueMinorTotal > 0 ? `, of which ${formatPkr(c.overdueMinorTotal)} is overdue` : "";
  return `Dear ${c.customerName}, a friendly reminder that your account currently shows ${formatPkr(c.balanceMinor)} outstanding${overdue}. Please arrange payment at your earliest convenience. Thank you.`;
}

function statementReminderText(c: CustomerDto, closingMinor: number, asOf: string): string {
  if (closingMinor <= 0) {
    return `Dear ${c.name}, your account is fully settled as of ${asOf}. Thank you.`;
  }
  return `Dear ${c.name}, a statement as of ${asOf} shows a pending balance of ${formatPkr(closingMinor)}. Please arrange payment at your earliest convenience. Thank you.`;
}

function CopyReminderButton({ text, label = "Copy reminder" }: { text: string; label?: string }) {
  const { toast } = useToast();
  const [copied, setCopied] = React.useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard?.writeText(text);
      setCopied(true);
      toast({ variant: "success", title: "Reminder copied" });
    } catch {
      toast({ variant: "error", title: "Could not copy the reminder" });
    }
    window.setTimeout(() => setCopied(false), 1500);
  };
  return (
    <Button type="button" variant="outline" size="sm" onClick={() => void copy()}>
      <Copy className="mr-1 h-3.5 w-3.5" />
      {copied ? "Copied" : label}
    </Button>
  );
}

export function SalesPage({ activeTab }: { activeTab: "pos" | "sales" | "customers" | "due" }) {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";
  const [editSaleId, setEditSaleId] = React.useState<number | null>(null);
  const [salesSearchQ, setSalesSearchQ] = React.useState("");

  const canSell = hasPermission("sale.create");
  const canEdit = hasPermission("sale.create");
  const canManageCustomer = hasPermission("customer.create");
  const canViewCustomer = hasPermission("customer.view");
  const canManageBundles = hasPermission("bundle.create");
  const canViewBundles = hasPermission("bundle.view");
  const canPrint = hasPermission("invoice.print");
  const canReceive = hasPermission("payment.receive");

  const dashboardTarget = React.useMemo(() => takeDashboardTarget("sales"), []);
  const [dashboardSalesFilter, setDashboardSalesFilter] = React.useState<"today" | "month" | null>(
    dashboardTarget?.target === "sales-today" ? "today" : dashboardTarget?.target === "sales-month" ? "month" : null,
  );
  const [dialog, setDialog] = React.useState<
    | null
    | "customer"
    | "ledger"
    | "receipt"
    | "void-receipt"
    | "bundle"
    | "bundle-detail"
    | "sale-detail"
    | "edit-sale"
  >(dashboardTarget?.target === "new-customer" && canManageCustomer ? "customer" : null);
  const [activeCustomer, setActiveCustomer] = React.useState<CustomerDto | null>(null);
  const [activeBundle, setActiveBundle] = React.useState<BundleDto | null>(null);
  const [activeSale, setActiveSale] = React.useState<SaleDto | null>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["selling"] });
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue"] });
  };

  const locationsQuery = useQuery({
    queryKey: ["selling", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session,
  });
  const customersQuery = useQuery({
    queryKey: ["selling", "customers"],
    queryFn: () => customerList(session),
    enabled: !!session && (canViewCustomer || canManageCustomer),
  });
  const salesQuery = useQuery({
    queryKey: ["selling", "sales"],
    queryFn: () => saleList(session),
    enabled: !!session && (canSell || canPrint),
  });
  const bundlesQuery = useQuery({
    queryKey: ["selling", "bundles"],
    queryFn: () => bundleList(session),
    enabled: !!session && (canViewBundles || canManageBundles),
  });
  const methodsQuery = useQuery({
    queryKey: ["selling", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session,
  });
  const accountsQuery = useQuery({
    queryKey: ["selling", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session,
  });
  const productsQuery = useQuery({
    queryKey: ["selling", "products"],
    queryFn: () => productList(session, { scope: "active" }),
    enabled: !!session && (canSell || canManageBundles),
  });

  const locations = locationsQuery.data ?? [];
  const customers = customersQuery.data ?? [];
  const sales = salesQuery.data ?? [];
  const bundles = bundlesQuery.data ?? [];
  const methods = methodsQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];
  const products = productsQuery.data ?? [];
  const dashboardShopDate = dashboardTarget && "shopDate" in dashboardTarget ? dashboardTarget.shopDate ?? "" : "";
  const dashboardFilteredSales = (() => {
    let list = dashboardSalesFilter && dashboardShopDate
      ? sales.filter((sale) => dashboardSalesFilter === "today"
        ? sale.saleDate === dashboardShopDate
        : sale.saleDate.startsWith(dashboardShopDate.slice(0, 7)))
      : sales;
    const term = salesSearchQ.trim().toLowerCase();
    if (term) {
      const phoneByCustomerId = new Map<number, string>();
      for (const c of customers) {
        if (c.phone) phoneByCustomerId.set(c.id, c.phone.toLowerCase());
      }
      list = list.filter((s) =>
        (s.saleNumber ?? "").toLowerCase().includes(term) ||
        (s.customerName ?? "").toLowerCase().includes(term) ||
        (s.customerId ? (phoneByCustomerId.get(s.customerId) ?? "").includes(term) : false)
      );
    }
    return list;
  })();

  React.useEffect(() => {
    if (!dashboardTarget) return;
    if (dashboardTarget.target === "sale" && salesQuery.data) {
      const sale = salesQuery.data.find((candidate) => candidate.id === dashboardTarget.id);
      if (sale) {
        setActiveSale(sale);
        setDialog("sale-detail");
      }
    }
    if (dashboardTarget.target === "payment" && customersQuery.data) {
      const customer = customersQuery.data.find((candidate) => candidate.id === dashboardTarget.customerId);
      if (customer) {
        setActiveCustomer(customer);
        setDialog("ledger");
      }
    }
  }, [dashboardTarget, salesQuery.data, customersQuery.data]);

  const confirmedSales = sales.filter((s) => s.status === "confirmed");
  const totalSales = confirmedSales.reduce((acc, s) => acc + s.totalMinor, 0);
  const totalPaid = confirmedSales.reduce((acc, s) => acc + s.paidMinor + s.advanceUsedMinor, 0);
  const outstanding = confirmedSales.reduce((acc, s) => acc + s.dueMinor, 0);
  const advanceOnHand = customers.reduce((acc, c) => acc + Math.max(0, c.advanceMinor), 0);
  const cashOnHand = accounts.reduce((acc, a) => acc + a.balanceMinor, 0);

  const done = (message: string) => () => {
    invalidate();
    setDialog(null);
    setActiveCustomer(null);
    setActiveBundle(null);
    setActiveSale(null);
    toast({ variant: "success", title: message });
  };
  const failed = (e: Error) => {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    toast({ variant: "error", title: "Operation failed", description: commandErrorMessage(e) });
  };

  const titles = {
    pos: { title: "New Sale / POS", subtitle: "Select products, customer, and complete the sale." },
    sales: { title: "Sales History", subtitle: "Saved sales, invoices, and printing." },
    customers: { title: "Customers", subtitle: "Manage customer directory." },
    due: { title: "Customer Dues", subtitle: "Track balances, advances, and collect payments." },
  };

  return (
    <div>
      <PageHeader
        title={titles[activeTab].title}
        subtitle={titles[activeTab].subtitle}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            {activeTab === "customers" && canManageCustomer && (
              <Button
                variant="outline"
                onClick={() => {
                  setActiveCustomer(null);
                  setDialog("customer");
                }}
              >
                <UserPlus className="h-4 w-4" />
                New customer
              </Button>
            )}
            {/* The Add Customer action in POS is rendered locally in the POS panel, so we only need it here for the Customers tab */}
          </div>
        }
      />

      {activeTab === "sales" && (
        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-4">
          <SummaryCard label="Confirmed sales" value={sales.filter((s) => s.status === "confirmed").length} accent="forest" />
          <SummaryCard label="Sales value" value={totalSales} money accent="gold" />
          <SummaryCard label="Collected" value={totalPaid} money accent="green" />
          <SummaryCard label="Outstanding" value={outstanding} money accent="rose" />
        </div>
      )}
      
      {activeTab === "due" && (
        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3">
          <SummaryCard label="Outstanding dues" value={outstanding} money accent="rose" />
          <SummaryCard label="Customer advances" value={advanceOnHand} money accent="neutral" />
          <SummaryCard label="Cash on hand" value={cashOnHand} money accent="neutral" />
        </div>
      )}

      <div className="mt-5">
        {(activeTab === "pos" || editSaleId) && (
          <PosPanel
            session={session}
            locations={locations}
            products={products}
            bundles={bundles}
            customers={customers}
            methods={methods}
            accounts={accounts}
            canManageCustomer={canManageCustomer}
            canPrint={canPrint}
            canSell={canSell}
            editSaleId={editSaleId}
            onNewCustomer={() => {
              setActiveCustomer(null);
              setDialog("customer");
            }}
            onDone={() => {
              toast({ variant: "success", title: editSaleId ? "Sale updated" : "Sale confirmed" });
              setEditSaleId(null);
              refresh();
            }}
            onFailed={failed}
          />
        )}
        {activeTab === "sales" && !editSaleId && (
          <div>
            {dashboardSalesFilter && (
              <div className="mb-3 flex items-center justify-between rounded-md border border-forest-200 bg-forest-50 px-3 py-2 text-sm text-forest-800">
                Showing {dashboardSalesFilter === "today" ? "today's sales" : "this month's sales"} from the Dashboard.
                <Button variant="ghost" size="sm" onClick={() => setDashboardSalesFilter(null)}>Show all</Button>
              </div>
            )}
            <div className="relative mb-3 max-w-sm">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
              <Input
                value={salesSearchQ}
                onChange={(e) => setSalesSearchQ(e.target.value)}
                placeholder="Search invoice, customer, or phone…"
                className="pl-8"
              />
            </div>
            <SalesTable
              session={session}
              rows={dashboardFilteredSales}
              loading={salesQuery.isLoading}
              canEdit={canEdit}
              canPrint={canPrint}
              onView={(s) => {
                setActiveSale(s);
                setDialog("sale-detail");
              }}
              onEdit={(s) => {
                setActiveSale(s);
                setDialog("edit-sale");
              }}
              onResumeDraft={(s) => {
                setEditSaleId(s.id);
              }}
              onDeleteDraft={async (s) => {
                try {
                  await saleDraftDelete(session, { saleId: s.id, reason: "Draft deleted" });
                  toast({ variant: "success", title: "Draft deleted" });
                  refresh();
                } catch (e) {
                  failed(e as Error);
                }
              }}
            />
          </div>
        )}
        {activeTab === "customers" && (
          <CustomersTable
            rows={customers}
            loading={customersQuery.isLoading}
            canManage={canManageCustomer}
            onLedger={(c) => {
              setActiveCustomer(c);
              setDialog("ledger");
            }}
            onEdit={(c) => {
              setActiveCustomer(c);
              setDialog("customer");
            }}
          />
        )}
        {activeTab === "due" && !editSaleId && canReceive && <DueControlPanel session={session} onError={failed} />}
      </div>

      {dialog === "customer" && (
        <CustomerDialog
          session={session}
          customer={activeCustomer}
          onClose={() => setDialog(null)}
          onDone={done(activeCustomer ? "Customer updated" : "Customer created")}
          onError={failed}
        />
      )}
      {dialog === "ledger" && activeCustomer && (
        <LedgerDialog
          session={session}
          customer={activeCustomer}
          onClose={() => setDialog(null)}
          onError={failed}
        />
      )}
      {dialog === "bundle" && (
        <BundleDialog
          session={session}
          bundle={activeBundle}
          onClose={() => setDialog(null)}
          onDone={done(activeBundle ? "Set updated" : "Set created")}
          onError={failed}
        />
      )}
      {dialog === "bundle-detail" && (
        <BundleAvailabilityDialog
          session={session}
          bundles={bundles}
          locations={locations}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "sale-detail" && activeSale && (
        <SaleDetailDialog
          session={session}
          sale={activeSale}
          canPrint={canPrint}
          onClose={() => setDialog(null)}
          onEdit={(s) => {
            setActiveSale(s);
            setDialog("edit-sale");
          }}
          onPrinted={done("Invoice generated")}
          onError={failed}
        />
      )}
      {dialog === "edit-sale" && activeSale && (
        <EditSaleDialog
          session={session}
          sale={activeSale}
          onClose={() => setDialog(null)}
          onDone={done("Sale updated")}
          onError={failed}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Point of sale
// ---------------------------------------------------------------------------

function PosPanel({
  session,
  locations,
  products,
  bundles,
  customers,
  methods,
  accounts,
  canManageCustomer,
  canPrint,
  canSell,
  editSaleId,
  onBack,
  onNewCustomer,
  onDone,
  onFailed,
}: {
  session: string;
  locations: LocationDto[];
  products: ProductListItemDto[];
  bundles: BundleDto[];
  customers: CustomerDto[];
  methods: PaymentMethodDto[];
  accounts: CashAccountDto[];
  canManageCustomer: boolean;
  canPrint: boolean;
  canSell: boolean;
  editSaleId?: number | null;
  onBack?: () => void;
  onNewCustomer: () => void;
  onDone: () => void;
  onFailed: (e: Error) => void;
}) {
  const locationId = locations[0]?.id ?? null;
  const [q, setQ] = React.useState("");
  const [cart, setCart] = React.useState<CartLine[]>([]);
  const [customerId, setCustomerId] = React.useState<number | null>(null);
  const [discountMinor, setDiscountMinor] = React.useState(0);
  const [deliveryMinor, setDeliveryMinor] = React.useState(0);
  const [paidMinor, setPaidMinor] = React.useState(0);
  const [advanceMinor, setAdvanceMinor] = React.useState(0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [success, setSuccess] = React.useState<SaleDto | null>(null);
  const createdIdRef = React.useRef<number | null>(null);
  const nextKey = React.useRef(() => `l${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);

  const stockQuery = useQuery({
    queryKey: ["selling", "stock", locationId],
    queryFn: () => stockBalanceList(session, locationId),
    enabled: !!session && locationId !== null,
  });
  
  React.useEffect(() => {
    if (!editSaleId || !session) return;
    let mounted = true;
    saleGet(session, editSaleId).then((sale) => {
      if (!mounted) return;
      setCustomerId(sale.customerId ?? null);
      setDiscountMinor(sale.discountMinor ?? 0);
      setDeliveryMinor(sale.deliveryChargeMinor ?? 0);
      
      const cartItems = sale.items.map((i, idx) => ({
        key: 'edit-' + idx,
        productId: i.productId ?? undefined,
        bundleId: i.bundleId ?? undefined,
        name: i.productName ?? 'Unknown item',
        article: i.articleNumber ?? '',
        unitPriceMinor: i.unitPriceMinor,
        quantity: i.quantity,
      }));
      setCart(cartItems);
      
      setPaidMinor(sale.paidMinor ?? 0);
      setAdvanceMinor(sale.advanceUsedMinor ?? 0);
    }).catch(console.error);
    return () => { mounted = false; };
  }, [editSaleId, session]);

  const stockByProduct: Record<number, StockBalanceDto> = React.useMemo(() => {
    const map: Record<number, StockBalanceDto> = {};
    for (const row of stockQuery.data ?? []) map[row.productId] = row;
    return map;
  }, [stockQuery.data]);

  const selectedCustomer = customers.find((c) => c.id === customerId) ?? null;

  const cartQtyByProduct: Record<number, number> = React.useMemo(() => {
    const map: Record<number, number> = {};
    for (const l of cart) {
      if (l.productId) map[l.productId] = (map[l.productId] ?? 0) + l.quantity;
    }
    return map;
  }, [cart]);

  const effectiveAvailable = React.useMemo(() => {
    const map: Record<number, number> = {};
    for (const p of products) {
      const raw = stockByProduct[p.id]?.available ?? 0;
      const inCart = cartQtyByProduct[p.id] ?? 0;
      map[p.id] = Math.max(0, raw - inCart);
    }
    return map;
  }, [products, stockByProduct, cartQtyByProduct]);

  const addProduct = (p: ProductListItemDto) => {
    const avail = effectiveAvailable[p.id] ?? 0;
    if (p.trackStock && avail <= 0) return;
    setCart((prev) => {
      const existing = prev.find((l) => l.productId === p.id);
      if (existing) {
        if (p.trackStock && existing.quantity >= avail) return prev;
        return prev.map((l) =>
          l.productId === p.id ? { ...l, quantity: l.quantity + 1 } : l,
        );
      }
      return [
        ...prev,
        {
          key: nextKey.current(),
          productId: p.id,
          name: p.name,
          article: p.articleNumber,
          unitPriceMinor: p.salePriceMinor,
          quantity: 1,
        },
      ];
    });
  };

  const addBundle = (b: BundleDto) => {
    setCart((prev) => {
      const existing = prev.find((l) => l.bundleId === b.id);
      if (existing) {
        return prev.map((l) =>
          l.bundleId === b.id ? { ...l, quantity: l.quantity + 1 } : l,
        );
      }
      return [
        ...prev,
        {
          key: nextKey.current(),
          bundleId: b.id,
          name: b.name,
          article: b.code,
          unitPriceMinor: b.defaultPriceMinor,
          quantity: 1,
        },
      ];
    });
  };

  const changeQty = (key: string, delta: number) => {
    setCart((prev) =>
      prev
        .map((l) => {
          if (l.key !== key) return l;
          const newQty = l.quantity + delta;
          if (newQty <= 0) return { ...l, quantity: 0 };
          if (delta > 0 && l.productId) {
            const p = products.find((x) => x.id === l.productId);
            if (p?.trackStock && newQty > (effectiveAvailable[l.productId] ?? 0) + l.quantity) return l;
          }
          return { ...l, quantity: newQty };
        })
        .filter((l) => l.quantity > 0),
    );
  };

  const subtotal = cart.reduce((acc, l) => acc + l.unitPriceMinor * l.quantity, 0);
  const total = Math.max(0, subtotal - discountMinor + deliveryMinor);
  const advanceMax = Math.min(selectedCustomer?.advanceMinor ?? 0, total);
  const due = Math.max(0, total - paidMinor - advanceMinor);

  const resetForm = () => {
    setCart([]);
    setCustomerId(null);
    setDiscountMinor(0);
    setDeliveryMinor(0);
    setPaidMinor(0);
    setAdvanceMinor(0);
    setMethodId(null);
    createdIdRef.current = null;
  };

  const confirmMutation = useMutation({
    mutationFn: async () => {
      if (!locationId || cart.length === 0) throw new Error("no items");
      let saleId = createdIdRef.current;
      if (!saleId) {
        const draft = await saleCreate(session, {
          locationId,
          customerId,
          kind: "sale",
          discountMinor: discountMinor > 0 ? discountMinor : null,
          deliveryChargeMinor: deliveryMinor > 0 ? deliveryMinor : null,
          items: cart.map((l) => ({
            productId: l.productId ?? null,
            bundleId: l.bundleId ?? null,
            quantity: l.quantity,
          })),
        });
        saleId = draft.id;
        createdIdRef.current = saleId;
      }
      return saleConfirm(session, {
        saleId,
        idempotencyKey: `sale-confirm-${saleId}-${Date.now()}`,
        paidMinor: paidMinor > 0 ? paidMinor : null,
        cashAccountId: paidMinor > 0 ? defaultAccountId : null,
        paymentMethodId: paidMinor > 0 ? methodId : null,
        advanceUsedMinor: advanceMinor > 0 ? advanceMinor : null,
      });
    },
    onSuccess: (confirmed) => {
      createdIdRef.current = null;
      setSuccess(confirmed);
    },
    onError: onFailed,
  });

  const advanceExceeds = advanceMinor > advanceMax || advanceMinor > total;
  const overpaid = paidMinor + advanceMinor > total;
  const defaultAccountId = accounts[0]?.id ?? null;
  const missingPaymentCtx = paidMinor > 0 && methodId === null;
  const valid =
    cart.length > 0 &&
    !advanceExceeds &&
    !overpaid &&
    !missingPaymentCtx;

  const searchTerm = q.trim().toLowerCase();
  const filteredProducts = (searchTerm
    ? products.filter(
        (p) =>
          p.name.toLowerCase().includes(searchTerm) ||
          p.articleNumber.toLowerCase().includes(searchTerm),
      )
    : products
  ).filter((p) => !p.trackStock || (effectiveAvailable[p.id] ?? 0) > 0);

  const filteredBundles = (searchTerm
    ? bundles.filter(
        (b) => b.name.toLowerCase().includes(searchTerm) || b.code.toLowerCase().includes(searchTerm),
      )
    : bundles
  ).filter((b) => {
    const items = b.items;
    if (items.length === 0) return true;
    return items.every((bi) => {
      const raw = stockByProduct[bi.productId]?.available ?? 0;
      const inCart = cartQtyByProduct[bi.productId] ?? 0;
      return raw - inCart >= bi.quantity;
    });
  });

  if (!canSell) {
    return <EmptyRow message="You don't have permission to create sales." />;
  }

  return (
    <div className="grid items-start gap-4 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="rounded-lg border border-neutral-200 bg-white p-4">
        <div className="flex flex-wrap items-center gap-2">
          <div className="grid gap-1.5">
            <Label>Showroom</Label>
            <div className="flex h-9 min-w-48 items-center rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm text-neutral-700">
              {locations[0]?.name ?? "Loading…"}
            </div>
          </div>
          <div className="grid flex-1 gap-1.5">
            <Label className="invisible">Search</Label>
            <div className="relative min-w-0">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
              <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search products or sets…" className="pl-8" />
            </div>
          </div>
        </div>

        <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
          {filteredProducts.map((p) => {
            const stock = stockByProduct[p.id];
            const avail = effectiveAvailable[p.id] ?? 0;
            const outOfStock = p.trackStock && (!stock || avail <= 0);
            return (
              <button
                key={p.id}
                type="button"
                onClick={() => addProduct(p)}
                disabled={outOfStock}
                className={cn(
                  "flex flex-col gap-2 rounded-md border border-neutral-200 p-2 text-left transition-colors",
                  outOfStock ? "cursor-not-allowed opacity-50" : "hover:border-forest-400 hover:bg-forest-50",
                )}
              >
                <div className="aspect-[4/3] w-full overflow-hidden rounded-md bg-neutral-100">
                  <StoredImage path={p.primaryThumbnailPath} className="h-full w-full object-cover" />
                </div>
                <div className="grid gap-0.5">
                  <span className="truncate text-sm font-medium text-neutral-900">{p.name}</span>
                  <span className="text-[11px] text-neutral-500">{p.articleNumber}</span>
                  <span className="text-sm font-semibold text-forest-700">{formatPkr(p.salePriceMinor)}</span>
                </div>
                {stock && (
                  <p
                    className={cn(
                      "text-[11px]",
                      avail > 0 ? "text-emerald-700" : "text-rose-600",
                    )}
                  >
                    {avail > 0 ? `${avail} available` : "Out of stock"}
                  </p>
                )}
              </button>
            );
          })}
          {filteredBundles.map((b) => (
            <button
              key={b.id}
              type="button"
              onClick={() => addBundle(b)}
              className="flex flex-col gap-2 rounded-md border border-amber-200 bg-amber-50/40 p-2 text-left transition-colors hover:border-amber-400 hover:bg-amber-50"
            >
              {b.coverImagePath ? (
                <div className="relative aspect-[4/3] w-full overflow-hidden rounded-md bg-amber-100">
                  <StoredImage path={b.coverImagePath} className="h-full w-full object-cover" />
                  <span className="absolute right-1 top-1 rounded bg-amber-100/90 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-700 backdrop-blur-sm">
                    Set
                  </span>
                </div>
              ) : (
                <div className="relative flex aspect-[4/3] w-full items-center justify-center overflow-hidden rounded-md bg-amber-100">
                  <Layers className="h-4 w-4 text-amber-600" />
                  <span className="absolute right-1 top-1 rounded bg-amber-100/90 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-700 backdrop-blur-sm">
                    Set
                  </span>
                </div>
              )}
              <div className="grid gap-0.5">
                <span className="truncate text-sm font-medium text-neutral-900">{b.name}</span>
                <span className="text-[11px] text-neutral-500">{b.code} · {b.items.length} item(s)</span>
                <span className="text-sm font-semibold text-amber-700">{formatPkr(b.defaultPriceMinor)}</span>
              </div>
            </button>
          ))}
          {filteredProducts.length === 0 && filteredBundles.length === 0 && (
            <p className="col-span-full rounded-md border border-dashed border-neutral-300 p-8 text-center text-sm text-neutral-500">
              No products or sets match your search.
            </p>
          )}
        </div>
      </div>

      <div className="rounded-lg border border-neutral-200 bg-white p-4">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-sm font-semibold text-neutral-800">Current sale</h3>
          {cart.length > 0 && (
            <button
              type="button"
              onClick={resetForm}
              className="text-xs text-neutral-400 underline hover:text-rose-600"
            >
              Clear all
            </button>
          )}
        </div>

        <div className="mt-3 grid gap-2">
          <div className="grid gap-1.5">
            <Label>Customer</Label>
            <div className="flex items-center gap-2">
              <div className="min-w-0 flex-1">
                <Select value={customerId ? String(customerId) : ""} onValueChange={(v) => setCustomerId(Number(v))}>
                  <SelectTrigger>
                    <SelectValue placeholder="Walk-in customer" />
                  </SelectTrigger>
                  <SelectContent>
                    {customers.map((c) => (
                      <SelectItem key={c.id} value={String(c.id)}>
                        {c.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              {canManageCustomer && (
                <Button type="button" size="icon" variant="outline" onClick={onNewCustomer} aria-label="New customer">
                  <UserPlus className="h-4 w-4" />
                </Button>
              )}
            </div>
            {selectedCustomer && (
              <div className="flex flex-wrap items-center gap-3 text-[11px] text-neutral-500">
                <span>
                  Balance{" "}
                  <span className={cn("font-medium tabular-nums", selectedCustomer.balanceMinor > 0 ? "text-rose-600" : "text-emerald-700")}>
                    {formatPkr(selectedCustomer.balanceMinor)}
                  </span>
                </span>
                <span>
                  Advance{" "}
                  <span className="font-medium tabular-nums text-forest-700">
                    {formatPkr(Math.max(0, selectedCustomer.advanceMinor))}
                  </span>
                </span>
              </div>
            )}
          </div>

          <div className="max-h-72 overflow-y-auto rounded-md border border-neutral-200">
            {cart.length === 0 ? (
              <p className="p-4 text-center text-sm text-neutral-500">Add items from the catalogue.</p>
            ) : (
              <ul className="divide-y divide-neutral-100">
                {cart.map((l) => (
                  <li key={l.key} className="flex items-center gap-2 px-3 py-2">
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-neutral-900">{l.name}</p>
                      <p className="text-[11px] text-neutral-500">{l.article}</p>
                    </div>
                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        onClick={() => changeQty(l.key, -1)}
                        className="flex h-6 w-6 items-center justify-center rounded border border-neutral-200 text-neutral-600 hover:bg-neutral-50"
                        aria-label="Decrease quantity"
                      >
                        −
                      </button>
                      <span className="w-8 text-center text-sm font-medium tabular-nums">{l.quantity}</span>
                      <button
                        type="button"
                        onClick={() => changeQty(l.key, 1)}
                        className="flex h-6 w-6 items-center justify-center rounded border border-neutral-200 text-neutral-600 hover:bg-neutral-50"
                        aria-label="Increase quantity"
                      >
                        +
                      </button>
                    </div>
                    <span className="w-20 text-right text-sm font-medium tabular-nums text-neutral-800">
                      {formatPkr(l.unitPriceMinor * l.quantity)}
                    </span>
                    <button
                      type="button"
                      onClick={() => setCart((prev) => prev.filter((x) => x.key !== l.key))}
                      className="text-neutral-300 hover:text-rose-600"
                      aria-label="Remove item"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="rounded-md bg-neutral-50 p-3 text-sm">
            <div className="flex justify-between py-0.5 text-neutral-600">
              <span>Subtotal</span>
              <span className="tabular-nums">{formatPkr(subtotal)}</span>
            </div>
            <div className="flex items-center justify-between gap-2 py-0.5">
              <span className="text-neutral-600">Discount</span>
              <div className="w-40">
                <MoneyInput value={discountMinor} onCommit={(v) => setDiscountMinor(v < 0 ? 0 : v)} placeholder="0.00" />
              </div>
            </div>
            <div className="flex items-center justify-between gap-2 py-0.5">
              <span className="text-neutral-600">Delivery charge</span>
              <div className="w-40">
                <MoneyInput value={deliveryMinor} onCommit={(v) => setDeliveryMinor(v < 0 ? 0 : v)} placeholder="0.00" />
              </div>
            </div>
            <div className="mt-1 flex justify-between border-t border-neutral-200 pt-1.5 font-semibold text-neutral-900">
              <span>Total</span>
              <span className="tabular-nums">{formatPkr(total)}</span>
            </div>
          </div>

          {selectedCustomer && selectedCustomer.advanceMinor > 0 && (
            <div className="grid gap-1.5">
              <Label>Use advance</Label>
              <MoneyInput
                value={advanceMinor}
                onCommit={(v) => setAdvanceMinor(Math.max(0, Math.min(v, advanceMax)))}
                placeholder="0.00"
              />
              {advanceExceeds && (
                <p className="text-[11px] text-rose-600">Advance cannot exceed {formatPkr(advanceMax)}.</p>
              )}
            </div>
          )}

          <div className="grid gap-1.5">
            <Label>Received today</Label>
            <MoneyInput value={paidMinor} onCommit={(v) => setPaidMinor(v < 0 ? 0 : v)} placeholder="0.00" />
          </div>
          {paidMinor > 0 && (
            <div className="grid gap-1.5">
              <Label>Payment method</Label>
              <Select value={methodId ? String(methodId) : ""} onValueChange={(v) => setMethodId(Number(v))}>
                <SelectTrigger>
                  <SelectValue placeholder="Method" />
                </SelectTrigger>
                <SelectContent>
                  {methods.map((m) => (
                    <SelectItem key={m.id} value={String(m.id)}>
                      {m.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2">
            <span className="text-sm text-neutral-700">Remaining due</span>
            <span className={cn("text-sm font-semibold tabular-nums", due > 0 ? "text-rose-600" : "text-emerald-700")}>
              {formatPkr(due)}
            </span>
          </div>

          {createdIdRef.current !== null && (
            <p className="text-[11px] leading-relaxed text-amber-700">
              A draft sale #{createdIdRef.current} was created but could not be confirmed. Fix the issue below and
              confirm again to reuse it, or{" "}
              <button type="button" className="underline" onClick={() => (createdIdRef.current = null)}>
                abandon draft
              </button>
              .
            </p>
          )}

          <Button type="button" onClick={() => confirmMutation.mutate()} disabled={!valid || confirmMutation.isPending} className="w-full">
            {confirmMutation.isPending ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Confirming…
              </>
            ) : (
              "Confirm sale"
            )}
          </Button>
          {success && canPrint && (
            <PrintInvoiceButton session={session} saleId={success.id} className="w-full" />
          )}
        </div>
      </div>

      {success && (
        <SaleSuccessDialog
          session={session}
          sale={success}
          canPrint={canPrint}
          onClose={() => {
            setSuccess(null);
            resetForm();
            onDone();
          }}
        />
      )}
    </div>
  );
}

function SaleSuccessDialog({
  session,
  sale,
  canPrint,
  onClose,
}: {
  session: string;
  sale: SaleDto;
  canPrint: boolean;
  onClose: () => void;
}) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <CheckCircle2 className="h-5 w-5 text-emerald-600" />
            Sale confirmed
          </DialogTitle>
          <DialogDescription>
            Sale {sale.saleNumber ?? `#${sale.id}`} was recorded successfully.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-2 rounded-md bg-neutral-50 p-3 text-sm">
          <div className="flex justify-between text-neutral-600">
            <span>Total</span>
            <span className="font-medium tabular-nums">{formatPkr(sale.totalMinor)}</span>
          </div>
          <div className="flex justify-between text-neutral-600">
            <span>Received</span>
            <span className="tabular-nums">{formatPkr(sale.paidMinor)}</span>
          </div>
          <div className="flex justify-between text-neutral-600">
            <span>Advance used</span>
            <span className="tabular-nums">{formatPkr(sale.advanceUsedMinor)}</span>
          </div>
          <div className="flex justify-between ">
            <span className="text-neutral-600">Due</span>
            <span className={cn("font-semibold tabular-nums", sale.dueMinor > 0 ? "text-rose-600" : "text-emerald-700")}>
              {formatPkr(sale.dueMinor)}
            </span>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Done
          </Button>
          {canPrint && <PrintInvoiceButton session={session} saleId={sale.id} onClose={onClose} />}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PrintInvoiceButton({
  session,
  saleId,
  className,
  onClose,
  onError,
}: {
  session: string;
  saleId: number;
  className?: string;
  onClose?: () => void;
  onError?: (e: Error) => void;
}) {
  const { toast } = useToast();
  const [busy, setBusy] = React.useState(false);
  const brandingQuery = usePrintBranding(session);
  const run = async () => {
    setBusy(true);
    try {
      const [sale, branding] = await Promise.all([
        saleGet(session, saleId),
        resolvePrintBranding(brandingQuery),
      ]);
      const cust = sale.customerId ? await customerGet(session, sale.customerId) : null;
      let payments: { date: string; amount: number; method: string }[] | undefined;
      if (sale.customerId) {
        const allReceipts = await customerReceiptList(session, sale.customerId);
        const matched = (allReceipts as CustomerPaymentDto[])
          .filter((p) => p.status === "posted" && p.allocations.some((a) => a.saleId === sale.id))
          .sort((a, b) => a.paymentDate.localeCompare(b.paymentDate));
        if (matched.length > 0) {
          payments = matched.map((p) => {
            const alloc = p.allocations.find((a) => a.saleId === sale.id);
            return { date: p.paymentDate, amount: alloc?.amountMinor ?? p.amountMinor, method: p.paymentMethodName };
          });
        }
      }
      await printInvoiceA4(
        sale,
        loadPrintSettings(),
        branding.name,
        branding.address,
        branding.phone,
        branding.logo,
        cust?.phone,
        cust?.address,
        payments,
      );
      toast({ variant: "success", title: "Invoice print window opened" });
      if (onClose) onClose();
    } catch (e) {
      toast({ variant: "error", title: "Could not generate invoice", description: commandErrorMessage(e) });
      onError?.(e as Error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Button type="button" variant="outline" onClick={() => void run()} disabled={busy || brandingQuery.isLoading} className={className}>
      {busy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Printer className="mr-2 h-4 w-4" />}
      Print invoice
    </Button>
  );
}

function PrintReceiptButton({
  session,
  payment,
  className,
  onError,
}: {
  session: string;
  payment: CustomerPaymentDto;
  className?: string;
  onError?: (e: Error) => void;
}) {
  const { toast } = useToast();
  const [busy, setBusy] = React.useState(false);
  const brandingQuery = usePrintBranding(session);
  const run = async () => {
    setBusy(true);
    try {
      const branding = await resolvePrintBranding(brandingQuery);
      await printPaymentReceipt(payment, loadPrintSettings(), branding);
      toast({ variant: "success", title: "Receipt print window opened" });
    } catch (e) {
      toast({ variant: "error", title: "Could not generate receipt", description: commandErrorMessage(e) });
      onError?.(e as Error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Button type="button" variant="outline" size="sm" onClick={() => void run()} disabled={busy || brandingQuery.isLoading} className={className}>
      {busy ? <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" /> : <Printer className="mr-1 h-3.5 w-3.5" />}
      Print
    </Button>
  );
}

type PrintBranding = { name: string; address: string | null; phone: string | null; logo: string | null };

function usePrintBranding(session: string) {
  return useQuery({
    queryKey: ["receipt-print", "branding"],
    queryFn: async () => {
      const [name, address, phone, logo] = await Promise.all([
        settingsGet(session, "shop.name"),
        settingsGet(session, "shop.address"),
        settingsGet(session, "shop.phone"),
        shopLogoGet(session),
      ]);
      return {
        name: parseSavedString(name) ?? "Furniture Showroom",
        address: parseSavedString(address),
        phone: parseSavedString(phone),
        logo,
      };
    },
    enabled: Boolean(session),
  });
}

async function resolvePrintBranding(
  query: ReturnType<typeof usePrintBranding>,
): Promise<PrintBranding> {
  if (query.data) return query.data;
  const result = await query.refetch();
  if (!result.data) throw result.error ?? new Error("Could not load document branding.");
  return result.data;
}

function parseSavedString(raw: string | null | undefined): string | null {
  if (raw == null) return null;
  try {
    const value = JSON.parse(raw) as unknown;
    return typeof value === "string" ? value : null;
  } catch {
    return raw;
  }
}

function escapeReceiptHtml(value: string): string {
  return value
    .replace(/[&]/g, "&amp;")
    .replace(/[<]/g, "&lt;")
    .replace(/[>]/g, "&gt;")
    .replace(/[\"]/g, "&quot;")
    .replace(/[']/g, "&#39;");
}

async function printPaymentReceipt(
  payment: CustomerPaymentDto,
  settings: InvoicePrintSettings,
  branding: { name: string; address: string | null; phone: string | null; logo: string | null },
): Promise<void> {
  const money = (minor: number) => formatPkr(minor);
  const fontSize = settings.fontSize === "small" ? "10pt" : settings.fontSize === "large" ? "12pt" : "11pt";
  const copies = Math.max(1, Math.min(settings.copies, 10));
  const allocations = payment.allocations.map((allocation) => `
    <tr style="page-break-inside:avoid;border-bottom:1px solid #e5e7eb">
      <td style="padding:8px">${escapeReceiptHtml(allocation.saleNumber ?? `Sale #${allocation.saleId}`)}</td>
      <td style="padding:8px;text-align:right">${escapeReceiptHtml(money(allocation.amountMinor))}</td>
    </tr>`).join("");
  const receipt = `
    <section class="receipt">
      <header style="display:flex;justify-content:space-between;gap:24px;border-bottom:2px solid #16345f;padding-bottom:16px;margin-bottom:24px">
        <div>
          ${settings.showLogo ? branding.logo ? `<img src="${escapeReceiptHtml(branding.logo)}" alt="" style="width:56px;height:56px;object-fit:contain;margin-bottom:8px">` : '<div style="width:52px;height:52px;border-radius:8px;background:#16345f;color:white;display:flex;align-items:center;justify-content:center;font-weight:700;margin-bottom:8px">FS</div>' : ""}
          <div style="font-size:18px;font-weight:700;color:#16345f">${escapeReceiptHtml(branding.name)}</div>
          ${settings.showAddress && branding.address ? `<div style="margin-top:3px;color:#4b5563">${escapeReceiptHtml(branding.address)}</div>` : ""}
          ${settings.showPhone && branding.phone ? `<div style="color:#4b5563">Tel: ${escapeReceiptHtml(branding.phone)}</div>` : ""}
        </div>
        <div style="text-align:right"><div style="font-size:22px;font-weight:700;color:#16345f">PAYMENT RECEIPT</div><div style="margin-top:8px"><strong>${escapeReceiptHtml(payment.receiptNumber ?? `#${payment.id}`)}</strong></div><div>${escapeReceiptHtml(payment.paymentDate)}</div></div>
      </header>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:16px;margin-bottom:22px">
        <div><div class="label">Received from</div><strong>${escapeReceiptHtml(payment.customerName)}</strong></div>
        <div style="text-align:right"><div class="label">Amount received</div><strong style="font-size:18px;color:#16345f">${escapeReceiptHtml(money(payment.amountMinor))}</strong></div>
        ${settings.showPaymentDetails ? `<div><div class="label">Payment method</div>${escapeReceiptHtml(payment.paymentMethodName)}</div><div style="text-align:right"><div class="label">Account</div>${escapeReceiptHtml(payment.cashAccountName)}</div>` : ""}
      </div>
      ${settings.showPaymentDetails ? `<table style="width:100%;border-collapse:collapse"><thead><tr style="background:#16345f;color:white"><th style="padding:9px;text-align:left">Applied invoice</th><th style="padding:9px;text-align:right">Amount</th></tr></thead><tbody>${allocations || '<tr><td colspan="2" style="padding:12px;color:#64748b">Unallocated customer advance</td></tr>'}</tbody></table>${payment.advanceAllocMinor > 0 ? `<p style="margin-top:12px;text-align:right"><strong>Advance retained: ${escapeReceiptHtml(money(payment.advanceAllocMinor))}</strong></p>` : ""}` : ""}
      ${payment.notes ? `<p style="margin-top:18px"><strong>Notes:</strong> ${escapeReceiptHtml(payment.notes)}</p>` : ""}
      ${settings.footerText ? `<footer style="margin-top:32px;border-top:1px solid #e2e8f0;padding-top:14px;text-align:center;color:#64748b">${escapeReceiptHtml(settings.footerText)}</footer>` : ""}
    </section>`;
  const body = Array.from({ length: copies }, (_, index) => `${index ? '<div class="copy-break"></div>' : ""}${receipt}`).join("");
  const html = `<!doctype html><html><head><meta charset="utf-8"><title>Receipt ${escapeReceiptHtml(payment.receiptNumber ?? String(payment.id))}</title><style>@page{size:A4 ${settings.orientation};margin:${settings.marginMm}mm}*{box-sizing:border-box}body{font-family:'Segoe UI',Arial,sans-serif;font-size:${fontSize};color:#111827;margin:0;-webkit-print-color-adjust:exact;print-color-adjust:exact}.label{font-size:8pt;text-transform:uppercase;letter-spacing:.04em;color:#64748b;margin-bottom:3px}thead{display:table-header-group}.receipt{break-inside:auto}.copy-break{break-before:page;page-break-before:always}</style></head><body>${body}</body></html>`;

  await new Promise<void>((resolve, reject) => {
    const frame = document.createElement("iframe");
    frame.style.cssText = "position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden";
    const cleanup = () => frame.remove();
    frame.onload = () => {
      try {
        const printWindow = frame.contentWindow;
        if (!printWindow) throw new Error("Could not open the receipt print window.");
        printWindow.focus();
        printWindow.print();
        window.setTimeout(() => { cleanup(); resolve(); }, 1500);
      } catch (error) {
        cleanup();
        reject(error);
      }
    };
    frame.onerror = () => { cleanup(); reject(new Error("Could not prepare the receipt preview.")); };
    document.body.appendChild(frame);
    const frameDocument = frame.contentDocument ?? frame.contentWindow?.document;
    if (!frameDocument) { cleanup(); reject(new Error("Could not access the receipt preview.")); return; }
    frameDocument.open();
    frameDocument.write(html);
    frameDocument.close();
  });
}

// ---------------------------------------------------------------------------
// Sales list
// ---------------------------------------------------------------------------

function SalesTable({
  session,
  rows,
  loading,
  canEdit,
  canPrint,
  onView,
  onEdit,
  onResumeDraft,
  onDeleteDraft,
}: {
  session: string;
  rows: SaleDto[];
  loading: boolean;
  canEdit: boolean;
  canPrint: boolean;
  onView: (s: SaleDto) => void;
  onEdit: (s: SaleDto) => void;
  onResumeDraft: (s: SaleDto) => void;
  onDeleteDraft: (s: SaleDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No sales recorded yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Sale</TableHead>
              <TableHead>Customer</TableHead>
              <TableHead>Date</TableHead>
              <TableHead>Kind</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Total</TableHead>
              <TableHead className="text-right">Received</TableHead>
              <TableHead className="text-right">Due</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((s) => (
              <TableRow key={s.id}>
                <TableCell className="font-medium text-neutral-900">{s.saleNumber ?? `#${s.id}`}</TableCell>
                <TableCell>{s.customerName ?? "—"}</TableCell>
                <TableCell className="whitespace-nowrap">{s.saleDate ? new Date(s.saleDate).toLocaleDateString(undefined, { dateStyle: "medium" }) : "—"}</TableCell>
                <TableCell className="capitalize">{s.kind}</TableCell>
                <TableCell>
                  <div className="flex items-center gap-1.5">
                    <SaleStatusBadge status={s.status} />
                    {s.status === "confirmed" && <PaymentStatusBadge dueMinor={s.dueMinor} paidMinor={s.paidMinor} />}
                  </div>
                </TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(s.totalMinor)}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(s.paidMinor + s.advanceUsedMinor)}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(s.dueMinor)}</TableCell>
                <TableCell className="text-right">
                  <div className="flex items-center justify-end gap-2">
                    <Button variant="outline" size="sm" onClick={() => onView(s)}>
                      View
                    </Button>
                    {s.status === "draft" && (
                      <>
                        <Button variant="outline" size="sm" className="text-forest-700" onClick={() => onResumeDraft(s)}>
                          Resume
                        </Button>
                        <Button variant="outline" size="sm" className="text-rose-600" onClick={() => onDeleteDraft(s)}>
                          Delete
                        </Button>
                      </>
                    )}
                    {canPrint && s.status === "confirmed" && (
                      <PrintInvoiceButton session={session} saleId={s.id} />
                    )}
                    {canEdit && s.status === "confirmed" && (
                      <Button variant="outline" size="sm" className="text-amber-600" onClick={() => onEdit(s)}>
                        <Pencil className="mr-1 h-3.5 w-3.5" />
                        Edit
                      </Button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function SaleStatusBadge({ status }: { status: string }) {
  if (status === "confirmed") return <Badge variant="success">Confirmed</Badge>;
  if (status === "cancelled") return <Badge variant="danger">Cancelled</Badge>;
  if (status === "quotation") return <Badge variant="warning">Quotation</Badge>;
  return <Badge variant="neutral">Draft</Badge>;
}

function PaymentStatusBadge({ dueMinor, paidMinor }: { dueMinor: number; paidMinor: number }) {
  if (dueMinor <= 0 && paidMinor > 0) return <Badge variant="success">Paid</Badge>;
  if (paidMinor > 0 && dueMinor > 0) return <Badge variant="warning">Partial</Badge>;
  return <Badge variant="danger">Unpaid</Badge>;
}

function SaleDetailDialog({
  session,
  sale,
  canPrint,
  onClose,
  onEdit,
  onPrinted,
  onError,
}: {
  session: string;
  sale: SaleDto;
  canPrint: boolean;
  onClose: () => void;
  onEdit: (s: SaleDto) => void;
  onPrinted: () => void;
  onError: (e: Error) => void;
}) {
  const detailQuery = useQuery({
    queryKey: ["selling", "sale-detail", sale.id],
    queryFn: () => saleGet(session, sale.id),
    enabled: !!session && sale.status === "confirmed",
  });
  const current = detailQuery.data ?? sale;

  const paymentsQuery = useQuery({
    queryKey: ["selling", "receipts", current.customerId],
    queryFn: () => customerReceiptList(session, current.customerId!),
    enabled: !!session && !!current.customerId && current.status === "confirmed",
  });
  const salePayments = React.useMemo(() => {
    if (!paymentsQuery.data || !current.customerId) return [];
    const posted = (paymentsQuery.data as CustomerPaymentDto[]).filter((p) => p.status === "posted");
    const matched: CustomerPaymentDto[] = [];
    for (const p of posted) {
      for (const a of p.allocations) {
        if (a.saleId === current.id) {
          matched.push(p);
          break;
        }
      }
    }
    return matched;
  }, [paymentsQuery.data, current.id, current.customerId]);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{current.saleNumber ?? `Sale #${current.id}`}</DialogTitle>
          <DialogDescription>
            {current.customerName ?? "Walk-in"} · {current.saleDate} · <SaleStatusBadge status={current.status} />
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-2 rounded-md bg-neutral-50 p-3 text-sm md:grid-cols-4">
          <div>
            <p className="text-[11px] text-neutral-500">Subtotal</p>
            <p className="font-medium tabular-nums">{formatPkr(current.subtotalMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Discount</p>
            <p className="font-medium tabular-nums">{formatPkr(current.discountMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Delivery</p>
            <p className="font-medium tabular-nums">{formatPkr(current.deliveryChargeMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Total</p>
            <p className="font-semibold tabular-nums">{formatPkr(current.totalMinor)}</p>
          </div>
        </div>
        <div className="max-h-72 overflow-y-auto rounded-md border border-neutral-200">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Item</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Price</TableHead>
                <TableHead className="text-right">Total</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {current.items.map((i) => (
                <TableRow key={i.id}>
                  <TableCell>
                    <p className="font-medium text-neutral-900">{i.productName}</p>
                    <p className="text-[11px] text-neutral-500">
                      {i.bundleId ? `${i.articleNumber} · set` : i.articleNumber}
                    </p>
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{i.quantity}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatPkr(i.unitPriceMinor)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatPkr(i.lineTotalMinor)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <div className="grid grid-cols-3 gap-2 text-sm">
          <div>
            <p className="text-[11px] text-neutral-500">Received</p>
            <p className="font-medium tabular-nums">{formatPkr(current.paidMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Advance used</p>
            <p className="font-medium tabular-nums">{formatPkr(current.advanceUsedMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Due</p>
            <p className="font-medium tabular-nums">{formatPkr(current.dueMinor)}</p>
          </div>
        </div>
        {salePayments.length > 0 && (
          <div className="rounded-md border border-neutral-200 bg-neutral-50 p-3">
            <p className="mb-2 text-xs font-medium text-neutral-600">Payment history</p>
            <div className="space-y-1.5">
              {salePayments.map((p) => {
                const alloc = p.allocations.find((a) => a.saleId === current.id);
                return (
                  <div key={p.id} className="flex items-center justify-between text-sm">
                    <div className="flex items-center gap-2">
                      <span className="text-neutral-500">{p.paymentDate}</span>
                      <span className="text-xs text-neutral-400">·</span>
                      <span className="text-xs text-neutral-500">{p.paymentMethodName}</span>
                    </div>
                    <span className="font-medium tabular-nums text-forest-700">{formatPkr(alloc?.amountMinor ?? p.amountMinor)}</span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {current.notes && <p className="rounded-md bg-neutral-50 p-3 text-sm text-neutral-600">{current.notes}</p>}
        <DialogFooter className="items-center justify-between">
          <p className="text-xs text-neutral-400">
            Created {formatDateTime(current.createdAt)}
            {current.confirmedAt ? ` · confirmed ${formatDateTime(current.confirmedAt)}` : ""}
          </p>
          <div className="flex items-center gap-2">
            {canPrint && current.status === "confirmed" && (
              <PrintInvoiceButton session={session} saleId={current.id} onError={onError} onClose={onPrinted} />
            )}
            {current.status === "confirmed" && (
              <Button variant="ghost" className="text-amber-600" onClick={() => onEdit(current)}>
                <Pencil className="mr-2 h-4 w-4" />
                Edit sale
              </Button>
            )}
            <Button variant="outline" onClick={onClose}>
              Close
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function EditSaleDialog({
  session,
  sale: initialSale,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  sale: SaleDto;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const queryClient = useQueryClient();
  const [showDeleteConfirm, setShowDeleteConfirm] = React.useState(false);
  const [deleteReason, setDeleteReason] = React.useState("");

  const saleQuery = useQuery({
    queryKey: ["sale-detail", session, initialSale.id],
    queryFn: () => saleGet(session, initialSale.id),
  });
  const fullSale = saleQuery.data ?? initialSale;

  const [customerId, setCustomerId] = React.useState<number | null>(fullSale.customerId ?? null);
  const [discountMinor, setDiscountMinor] = React.useState(fullSale.discountMinor);
  const [deliveryChargeMinor, setDeliveryChargeMinor] = React.useState(fullSale.deliveryChargeMinor);
  const [paidMinor, setPaidMinor] = React.useState(fullSale.paidMinor);
  const [paymentMethodId, setPaymentMethodId] = React.useState<number | null>(null);
  const [notes, setNotes] = React.useState(fullSale.notes ?? "");
  const [items, setItems] = React.useState<Array<{ productId?: number | null; bundleId?: number | null; quantity: number; unitPriceMinor: number; name: string; articleNumber: string }>>(() =>
    fullSale.items.map((it) => ({
      productId: it.productId,
      bundleId: it.bundleId,
      quantity: it.quantity,
      unitPriceMinor: it.unitPriceMinor,
      name: it.productName,
      articleNumber: it.articleNumber,
    }))
  );

  React.useEffect(() => {
    if (saleQuery.data) {
      const s = saleQuery.data;
      setCustomerId(s.customerId ?? null);
      setDiscountMinor(s.discountMinor);
      setDeliveryChargeMinor(s.deliveryChargeMinor);
      setPaidMinor(s.paidMinor);
      setNotes(s.notes ?? "");
      setItems(s.items.map((it) => ({
        productId: it.productId,
        bundleId: it.bundleId,
        quantity: it.quantity,
        unitPriceMinor: it.unitPriceMinor,
        name: it.productName,
        articleNumber: it.articleNumber,
      })));
    }
  }, [saleQuery.data]);

  const customers = useQuery({
    queryKey: ["customers", session],
    queryFn: () => customerList(session),
    enabled: !!session,
  });

  const methods = useQuery({
    queryKey: ["payment-methods", session],
    queryFn: () => paymentMethodList(session),
    enabled: !!session,
  });

  const subtotalMinor = React.useMemo(() => items.reduce((sum, it) => sum + it.unitPriceMinor * it.quantity, 0), [items]);
  const totalMinor = subtotalMinor - discountMinor + deliveryChargeMinor;
  const dueMinor = Math.max(0, totalMinor - paidMinor);

  const updateItemQty = (idx: number, qty: number) => {
    setItems((prev) => prev.map((it, i) => (i === idx ? { ...it, quantity: Math.max(1, qty) } : it)));
  };
  const updateItemPrice = (idx: number, price: number) => {
    setItems((prev) => prev.map((it, i) => (i === idx ? { ...it, unitPriceMinor: Math.max(0, price) } : it)));
  };

  const editMutation = useMutation({
    mutationFn: () => saleEdit(session, {
      saleId: fullSale.id,
      customerId,
      discountMinor,
      deliveryChargeMinor,
      paidMinor,
      paymentMethodId,
      notes: notes.trim() || null,
      items: items.map((it) => ({
        productId: it.productId,
        bundleId: it.bundleId,
        quantity: it.quantity,
      })),
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sales", session] });
      onDone();
    },
    onError,
  });

  const deleteMutation = useMutation({
    mutationFn: () => saleDelete(session, { saleId: fullSale.id, reason: deleteReason.trim() || null }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sales", session] });
      onClose();
      onDone();
    },
    onError,
  });

  if (showDeleteConfirm) {
    return (
      <FormDialog
        title={`Delete sale ${fullSale.saleNumber ?? `#${fullSale.id}`}`}
        description="This will permanently remove the sale and restore all stock. Customer dues and payments will be reversed."
        onSubmit={() => deleteMutation.mutate()}
        busy={deleteMutation.isPending}
        submitLabel="Delete sale"
        onClose={onClose}
      >
        <div className="grid gap-1.5">
          <Label>Reason (recommended)</Label>
          <textarea
            value={deleteReason}
            onChange={(e) => setDeleteReason(e.target.value)}
            placeholder="Why is this sale being deleted?"
            rows={3}
            className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm"
          />
        </div>
        <p className="text-xs text-rose-600">This action cannot be undone.</p>
      </FormDialog>
    );
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Edit sale {fullSale.saleNumber ?? `#${fullSale.id}`}</DialogTitle>
          <DialogDescription>
            Modify items, pricing, discount, or payment. Totals will recalculate automatically.
          </DialogDescription>
        </DialogHeader>

        {saleQuery.isLoading ? (
          <div className="flex items-center justify-center py-8"><Loader2 className="h-6 w-6 animate-spin" /></div>
        ) : (
          <div className="grid gap-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-1.5">
                <Label>Customer</Label>
                <Select value={customerId ? String(customerId) : "walkin"} onValueChange={(v) => setCustomerId(v === "walkin" ? null : Number(v))}>
                  <SelectTrigger><SelectValue placeholder="Walk-in" /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="walkin">Walk-in</SelectItem>
                    {(customers.data ?? []).map((c) => (
                      <SelectItem key={c.id} value={String(c.id)}>{c.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-1.5">
                <Label>Notes</Label>
                <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Optional" />
              </div>
            </div>

            <div className="grid gap-1.5">
              <Label>Items</Label>
              <div className="rounded-lg border border-neutral-200 overflow-hidden">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Item</TableHead>
                      <TableHead className="w-24 text-right">Qty</TableHead>
                      <TableHead className="w-32 text-right">Unit Price</TableHead>
                      <TableHead className="text-right">Line Total</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {items.map((it, idx) => (
                      <TableRow key={idx}>
                        <TableCell>
                          <div className="text-sm font-medium">{it.name}</div>
                          <div className="text-xs text-neutral-500">{it.articleNumber}</div>
                        </TableCell>
                        <TableCell className="text-right">
                          <Input
                            type="number"
                            min={1}
                            value={it.quantity}
                            onChange={(e) => updateItemQty(idx, Number(e.target.value) || 1)}
                            className="w-20 text-right h-8"
                          />
                        </TableCell>
                        <TableCell className="text-right">
                          <MoneyInput value={it.unitPriceMinor} onCommit={(v) => updateItemPrice(idx, v)} className="w-28 h-8" />
                        </TableCell>
                        <TableCell className="text-right tabular-nums">{formatPkr(it.unitPriceMinor * it.quantity)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-1.5">
                <Label>Discount</Label>
                <MoneyInput value={discountMinor} onCommit={setDiscountMinor} />
              </div>
              <div className="grid gap-1.5">
                <Label>Delivery charge</Label>
                <MoneyInput value={deliveryChargeMinor} onCommit={setDeliveryChargeMinor} />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-1.5">
                <Label>Received amount</Label>
                <MoneyInput value={paidMinor} onCommit={setPaidMinor} />
              </div>
              <div className="grid gap-1.5">
                <Label>Payment method</Label>
                <Select value={paymentMethodId ? String(paymentMethodId) : ""} onValueChange={(v) => setPaymentMethodId(v ? Number(v) : null)}>
                  <SelectTrigger><SelectValue placeholder="Select method" /></SelectTrigger>
                  <SelectContent>
                    {(methods.data ?? []).map((m) => (
                      <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-4 grid grid-cols-4 gap-4 text-sm">
              <div>
                <div className="text-neutral-500">Subtotal</div>
                <div className="font-semibold tabular-nums">{formatPkr(subtotalMinor)}</div>
              </div>
              <div>
                <div className="text-neutral-500">Discount</div>
                <div className="font-semibold tabular-nums text-rose-600">{discountMinor > 0 ? `- ${formatPkr(discountMinor)}` : "—"}</div>
              </div>
              <div>
                <div className="text-neutral-500">Delivery</div>
                <div className="font-semibold tabular-nums">{deliveryChargeMinor > 0 ? formatPkr(deliveryChargeMinor) : "—"}</div>
              </div>
              <div>
                <div className="text-neutral-500">Due</div>
                <div className="font-semibold tabular-nums text-amber-600">{dueMinor > 0 ? formatPkr(dueMinor) : "Paid"}</div>
              </div>
            </div>
          </div>
        )}

        <DialogFooter className="gap-2">
          <Button variant="ghost" className="text-rose-600 mr-auto" onClick={() => setShowDeleteConfirm(true)}>
            Delete sale
          </Button>
          <Button variant="ghost" onClick={onClose} disabled={editMutation.isPending}>Cancel</Button>
          <Button onClick={() => editMutation.mutate()} disabled={editMutation.isPending || saleQuery.isLoading}>
            {editMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Save changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Customers
// ---------------------------------------------------------------------------

function CustomersTable({
  rows,
  loading,
  canManage,
  onLedger,
  onEdit,
}: {
  rows: CustomerDto[];
  loading: boolean;
  canManage: boolean;
  onLedger: (c: CustomerDto) => void;
  onEdit: (c: CustomerDto) => void;
}) {
  const [q, setQ] = React.useState("");
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No customers yet." />;
  const term = q.trim().toLowerCase();
  const filtered = term
    ? rows.filter(
        (c) =>
          c.name.toLowerCase().includes(term) ||
          (c.phone ?? "").toLowerCase().includes(term),
      )
    : rows;
  return (
    <div>
      <div className="relative mb-3 max-w-sm">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
        <Input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search name or phone…"
          className="pl-8"
        />
      </div>
      {filtered.length === 0 ? (
        <EmptyRow message="No customers match your search." />
      ) : (
        <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
          <div className="overflow-x-auto">
            <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Code</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Phone</TableHead>
              <TableHead className="text-right">Credit limit</TableHead>
              <TableHead className="text-right">Balance</TableHead>
              <TableHead className="text-right">Advance</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filtered.map((c) => (
              <TableRow key={c.id}>
                <TableCell className="font-medium text-neutral-900">{c.code}</TableCell>
                <TableCell>{c.name}</TableCell>
                <TableCell>{c.phone ?? "—"}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(c.creditLimitMinor)}</TableCell>
                <TableCell className="text-right tabular-nums">
                  <span className={c.balanceMinor > 0 ? "text-rose-600" : "text-emerald-700"}>{formatPkr(c.balanceMinor)}</span>
                </TableCell>
                <TableCell className="text-right tabular-nums text-forest-700">{formatPkr(Math.max(0, c.advanceMinor))}</TableCell>
                <TableCell>{c.isActive ? <Badge variant="success">Active</Badge> : <Badge variant="neutral">Inactive</Badge>}</TableCell>
                <TableCell className="text-right">
                  <div className="flex items-center justify-end gap-2">
                    <Button variant="outline" size="sm" onClick={() => onLedger(c)}>
                      Ledger
                    </Button>
                    {canManage && (
                      <Button variant="outline" size="sm" onClick={() => onEdit(c)}>
                        <Pencil className="mr-1 h-3.5 w-3.5" />
                        Edit
                      </Button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
      )}
    </div>
  );
}

function DueControlPanel({ session, onError }: { session: string; onError: (e: Error) => void }) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const { hasPermission } = useSession();
  const canReceive = hasPermission("payment.receive");

  const query = useQuery({
    queryKey: ["selling", "receivables"],
    queryFn: () => receivables(session),
    enabled: !!session,
  });
  const data = (query.data ?? null) as ReceivablesDto | null;

  const [searchQ, setSearchQ] = React.useState("");
  const [activeCustomer, setActiveCustomer] = React.useState<CustomerDto | null>(null);
  const [activeSale, setActiveSale] = React.useState<ReceivableSaleDto | null>(null);
  const [dialog, setDialog] = React.useState<null | "view" | "payment" | "history">(null);

  const openPayment = (customerId: number) => {
    customerGet(session, customerId).then((c) => {
      setActiveCustomer(c);
      setDialog("payment");
    }).catch(onError);
  };

  const openHistory = (customerId: number) => {
    customerGet(session, customerId).then((c) => {
      setActiveCustomer(c);
      setDialog("history");
    }).catch(onError);
  };

  const openView = (customerId: number) => {
    customerGet(session, customerId).then((c) => {
      setActiveCustomer(c);
      setDialog("view");
    }).catch(onError);
  };

  const afterPayment = () => {
    void queryClient.invalidateQueries({ queryKey: ["selling"] });
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
    toast({ variant: "success", title: "Payment recorded" });
    setDialog(null);
    setActiveCustomer(null);
    setActiveSale(null);
  };

  React.useEffect(() => {
    if (query.isError && query.error) onError(query.error as Error);
  }, [query.isError, query.error, onError]);

  if (query.isLoading) return <LoadingRow />;
  if (!data) return <EmptyRow message="Could not load receivable data." />;

  const term = searchQ.trim().toLowerCase();
  const matchesCustomer = (name: string, phone?: string | null) =>
    !term || name.toLowerCase().includes(term) || (phone ?? "").toLowerCase().includes(term);
  const overdue = data.overdue.filter((s) => matchesCustomer(s.customerName));
  const dueSoon = data.dueSoon.filter((s) => matchesCustomer(s.customerName));
  const highBalance = data.highBalance.filter((c) => matchesCustomer(c.customerName, c.phone));
  const creditLimitExceptions = data.creditLimitExceptions.filter((c) => matchesCustomer(c.customerName, c.phone));

  return (
    <div className="grid gap-6">
      <div className="relative max-w-sm">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
        <Input
          value={searchQ}
          onChange={(e) => setSearchQ(e.target.value)}
          placeholder="Search name or phone…"
          className="pl-8"
        />
      </div>
      <section className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
        <div className="flex items-center justify-between gap-2 border-b border-neutral-200 px-4 py-3">
          <h3 className="flex items-center gap-2 text-sm font-semibold text-rose-700">
            <TriangleAlert className="h-4 w-4" /> Overdue
          </h3>
          <span className="text-xs text-neutral-500">{overdue.length} unpaid past due</span>
        </div>
        {overdue.length === 0 ? (
          <EmptyRow message="No overdue invoices." />
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Customer</TableHead>
                  <TableHead>Invoice</TableHead>
                  <TableHead>Sale date</TableHead>
                  <TableHead>Due date</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead className="text-right">Paid</TableHead>
                  <TableHead className="text-right">Due</TableHead>
                  <TableHead className="text-right">Days</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {overdue.map((s) => (
                  <TableRow key={s.saleId}>
                    <TableCell>{s.customerName}</TableCell>
                    <TableCell className="font-medium text-neutral-900">{s.saleNumber ?? `#${s.saleId}`}</TableCell>
                    <TableCell className="whitespace-nowrap">{s.saleDate}</TableCell>
                    <TableCell className="whitespace-nowrap">{s.dueDate ?? "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(s.totalMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(s.paidMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums font-medium text-rose-600">{formatPkr(s.dueMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{s.days}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openView(s.customerId)}>
                          <Eye className="h-3.5 w-3.5" />
                        </Button>
                        {canReceive && (
                          <Button variant="ghost" size="sm" className="h-7 px-2 text-forest-700" onClick={() => openPayment(s.customerId)}>
                            <Wallet className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openHistory(s.customerId)}>
                          <History className="h-3.5 w-3.5" />
                        </Button>
                        <CopyReminderButton text={saleReminderText(s)} label="" />
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>

      <section className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
        <div className="flex items-center justify-between gap-2 border-b border-neutral-200 px-4 py-3">
          <h3 className="text-sm font-semibold text-neutral-900">Due in 7 days</h3>
          <span className="text-xs text-neutral-500">{dueSoon.length} approaching</span>
        </div>
        {dueSoon.length === 0 ? (
          <EmptyRow message="Nothing due in the next week." />
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Customer</TableHead>
                  <TableHead>Invoice</TableHead>
                  <TableHead>Sale date</TableHead>
                  <TableHead>Due date</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead className="text-right">Paid</TableHead>
                  <TableHead className="text-right">Due</TableHead>
                  <TableHead className="text-right">In days</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {dueSoon.map((s) => (
                  <TableRow key={s.saleId}>
                    <TableCell>{s.customerName}</TableCell>
                    <TableCell className="font-medium text-neutral-900">{s.saleNumber ?? `#${s.saleId}`}</TableCell>
                    <TableCell className="whitespace-nowrap">{s.saleDate}</TableCell>
                    <TableCell className="whitespace-nowrap">{s.dueDate ?? "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(s.totalMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(s.paidMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(s.dueMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{s.days}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openView(s.customerId)}>
                          <Eye className="h-3.5 w-3.5" />
                        </Button>
                        {canReceive && (
                          <Button variant="ghost" size="sm" className="h-7 px-2 text-forest-700" onClick={() => openPayment(s.customerId)}>
                            <Wallet className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openHistory(s.customerId)}>
                          <History className="h-3.5 w-3.5" />
                        </Button>
                        <CopyReminderButton text={saleReminderText(s)} label="" />
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>

      <section className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
        <div className="flex items-center justify-between gap-2 border-b border-neutral-200 px-4 py-3">
          <h3 className="text-sm font-semibold text-neutral-900">Highest balances</h3>
          <span className="text-xs text-neutral-500">top {highBalance.length}</span>
        </div>
        {highBalance.length === 0 ? (
          <EmptyRow message="No customer balances." />
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Customer</TableHead>
                  <TableHead>Phone</TableHead>
                  <TableHead className="text-right">Balance</TableHead>
                  <TableHead className="text-right">Overdue</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {highBalance.map((c) => (
                  <TableRow key={c.customerId}>
                    <TableCell className="font-medium text-neutral-900">{c.customerName}</TableCell>
                    <TableCell>{c.phone ?? "—"}</TableCell>
                    <TableCell className="text-right tabular-nums text-rose-600">{formatPkr(c.balanceMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(c.overdueMinorTotal)}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openView(c.customerId)}>
                          <Eye className="h-3.5 w-3.5" />
                        </Button>
                        {canReceive && (
                          <Button variant="ghost" size="sm" className="h-7 px-2 text-forest-700" onClick={() => openPayment(c.customerId)}>
                            <Wallet className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openHistory(c.customerId)}>
                          <History className="h-3.5 w-3.5" />
                        </Button>
                        <CopyReminderButton text={customerReminderText(c)} label="" />
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>

      <section className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
        <div className="flex items-center justify-between gap-2 border-b border-neutral-200 px-4 py-3">
          <h3 className="text-sm font-semibold text-amber-700">Credit limit exceptions</h3>
          <span className="text-xs text-neutral-500">{creditLimitExceptions.length} over limit</span>
        </div>
        {creditLimitExceptions.length === 0 ? (
          <EmptyRow message="No customer is over their credit limit." />
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Customer</TableHead>
                  <TableHead>Phone</TableHead>
                  <TableHead className="text-right">Limit</TableHead>
                  <TableHead className="text-right">Balance</TableHead>
                  <TableHead className="text-right">Over</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {creditLimitExceptions.map((c) => (
                  <TableRow key={c.customerId}>
                    <TableCell className="font-medium text-neutral-900">{c.customerName}</TableCell>
                    <TableCell>{c.phone ?? "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{formatPkr(c.creditLimitMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums text-rose-600">{formatPkr(c.balanceMinor)}</TableCell>
                    <TableCell className="text-right tabular-nums font-medium text-amber-700">{formatPkr(Math.max(0, c.balanceMinor - c.creditLimitMinor))}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openView(c.customerId)}>
                          <Eye className="h-3.5 w-3.5" />
                        </Button>
                        {canReceive && (
                          <Button variant="ghost" size="sm" className="h-7 px-2 text-forest-700" onClick={() => openPayment(c.customerId)}>
                            <Wallet className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => openHistory(c.customerId)}>
                          <History className="h-3.5 w-3.5" />
                        </Button>
                        <CopyReminderButton text={customerReminderText(c)} label="" />
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>

      {dialog === "payment" && activeCustomer && (
        <DuePaymentDialog
          session={session}
          customer={activeCustomer}
          sale={activeSale}
          onClose={() => { setDialog(null); setActiveCustomer(null); setActiveSale(null); }}
          onDone={afterPayment}
          onError={onError}
        />
      )}
      {dialog === "history" && activeCustomer && (
        <DuePaymentHistoryDialog
          session={session}
          customer={activeCustomer}
          onClose={() => { setDialog(null); setActiveCustomer(null); }}
        />
      )}
      {dialog === "view" && activeCustomer && (
        <DueViewDialog
          session={session}
          customer={activeCustomer}
          onClose={() => { setDialog(null); setActiveCustomer(null); }}
        />
      )}
    </div>
  );
}

function DuePaymentDialog({
  session,
  customer,
  sale,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  customer: CustomerDto;
  sale?: ReceivableSaleDto | null;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [amount, setAmount] = React.useState(sale ? sale.dueMinor : 0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [paymentDate, setPaymentDate] = React.useState(todayIso());
  const [notes, setNotes] = React.useState("");
  const [allocations, setAllocations] = React.useState<CustomerReceiptAllocationInput[]>([]);

  const methodsQuery = useQuery({
    queryKey: ["selling", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session,
  });
  const accountsQuery = useQuery({
    queryKey: ["selling", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session,
  });
  const previewQuery = useQuery({
    queryKey: ["selling", "receipt-preview", customer.id, amount],
    queryFn: () => customerReceiptPreview(session, { customerId: customer.id, amountMinor: amount }),
    enabled: amount > 0,
  });

  React.useEffect(() => {
    if (previewQuery.data) {
      setAllocations(previewQuery.data.allocations.map((a) => ({ saleId: a.saleId, amountMinor: a.allocatedMinor })));
    }
  }, [previewQuery.data]);

  const previewData = (previewQuery.data ?? null) as ReceiptPreviewDto | null;
  const allocMap = React.useMemo(() => {
    const m = new Map<number, number>();
    for (const a of allocations) m.set(a.saleId, a.amountMinor);
    return m;
  }, [allocations]);
  const submitAllocations: CustomerReceiptAllocationInput[] | null =
    previewData?.allocations.map((a) => ({ saleId: a.saleId, amountMinor: allocMap.get(a.saleId) ?? a.allocatedMinor })) ?? null;
  const allocatedSum = (submitAllocations ?? []).reduce((s, a) => s + Math.max(0, a.amountMinor), 0);

  const mutation = useMutation({
    mutationFn: () =>
      customerReceiptCreate(session, {
        customerId: customer.id,
        paymentMethodId: methodId!,
        cashAccountId: accountId!,
        paymentDate,
        amountMinor: amount,
        notes: notes.trim() || null,
        allocations: submitAllocations ?? [],
        idempotencyKey: `receipt-${customer.id}-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  const previewReady = amount > 0 && previewQuery.isSuccess && !previewQuery.isFetching;
  const valid =
    amount > 0 &&
    methodId !== null &&
    accountId !== null &&
    paymentDate.length > 0 &&
    previewReady;

  const setAllocation = (saleId: number, v: number) =>
    setAllocations((prev) => [...prev.filter((x) => x.saleId !== saleId), { saleId, amountMinor: v }]);

  return (
    <FormDialog
      title="Receive payment"
      description={`Payment from ${customer.name}. Current due: ${formatPkr(customer.balanceMinor)}.`}
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Record payment"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-3 text-sm">
        <div className="flex items-center justify-between">
          <span className="text-neutral-500">Customer</span>
          <span className="font-medium">{customer.name}</span>
        </div>
        <div className="mt-1 flex items-center justify-between">
          <span className="text-neutral-500">Current due</span>
          <span className="font-medium text-rose-600">{formatPkr(customer.balanceMinor)}</span>
        </div>
        {sale && (
          <div className="mt-1 flex items-center justify-between">
            <span className="text-neutral-500">Invoice due</span>
            <span className="font-medium text-rose-600">{formatPkr(sale.dueMinor)}</span>
          </div>
        )}
      </div>
      <div className="grid gap-1.5">
        <Label>Amount received</Label>
        <MoneyInput value={amount} onCommit={(v) => setAmount(v < 0 ? 0 : v)} placeholder="0.00" />
      </div>
      {amount > 0 && previewReady && previewData && previewData.allocations.length > 0 && (
        <div className="overflow-hidden rounded-md border border-neutral-200">
          <div className="border-b border-neutral-200 bg-neutral-50 px-3 py-2 text-xs font-medium text-neutral-600">
            Apply to open invoices
          </div>
          <div className="divide-y divide-neutral-100">
            {previewData.allocations.map((a) => (
              <div key={a.saleId} className="flex items-center gap-2 px-3 py-2">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-neutral-900">{a.saleNumber ?? `Invoice #${a.saleId}`}</p>
                  <p className="text-[11px] text-neutral-500">Due {formatPkr(a.dueMinor)}</p>
                </div>
                <MoneyInput
                  className="w-28"
                  value={allocMap.get(a.saleId) ?? a.allocatedMinor}
                  onCommit={(v) => setAllocation(a.saleId, v)}
                />
              </div>
            ))}
          </div>
          <div className="flex items-center justify-between border-t border-neutral-200 px-3 py-2 text-sm">
            <span className="text-neutral-500">Applied to invoices</span>
            <span className="font-medium tabular-nums">{formatPkr(Math.min(allocatedSum, amount))}</span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 text-sm">
            <span className="text-neutral-500">Advances</span>
            <span className="font-medium tabular-nums text-forest-700">{formatPkr(Math.max(0, amount - allocatedSum))}</span>
          </div>
        </div>
      )}
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Payment method</Label>
          <Select value={methodId ? String(methodId) : ""} onValueChange={(v) => setMethodId(Number(v))}>
            <SelectTrigger><SelectValue placeholder="Select" /></SelectTrigger>
            <SelectContent>
              {(methodsQuery.data ?? []).map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label>Cash account</Label>
          <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
            <SelectTrigger><SelectValue placeholder="Select" /></SelectTrigger>
            <SelectContent>
              {(accountsQuery.data ?? []).map((a) => (
                <SelectItem key={a.id} value={String(a.id)}>{a.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Date</Label>
          <Input type="date" value={paymentDate} onChange={(e) => setPaymentDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Notes (optional)</Label>
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Reference" />
        </div>
      </div>
    </FormDialog>
  );
}

function DuePaymentHistoryDialog({
  session,
  customer,
  onClose,
}: {
  session: string;
  customer: CustomerDto;
  onClose: () => void;
}) {
  const receiptsQuery = useQuery({
    queryKey: ["selling", "receipts", customer.id],
    queryFn: () => customerReceiptList(session, customer.id),
    enabled: !!session,
  });
  const receipts = (receiptsQuery.data ?? []) as CustomerPaymentDto[];

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Payment history — {customer.name}</DialogTitle>
          <DialogDescription>
            Balance: {formatPkr(customer.balanceMinor)} · {receipts.length} payment{receipts.length === 1 ? "" : "s"}
          </DialogDescription>
        </DialogHeader>
        {receiptsQuery.isLoading ? (
          <div className="flex items-center justify-center py-8"><Loader2 className="h-6 w-6 animate-spin" /></div>
        ) : receipts.length === 0 ? (
          <EmptyRow message="No payments recorded yet." />
        ) : (
          <div className="overflow-hidden rounded-lg border border-neutral-200">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Date</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                  <TableHead>Method</TableHead>
                  <TableHead>Invoice</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {receipts.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell className="whitespace-nowrap">{r.paymentDate}</TableCell>
                    <TableCell className="text-right tabular-nums font-medium">{formatPkr(r.amountMinor)}</TableCell>
                    <TableCell>{r.paymentMethodName}</TableCell>
                    <TableCell>
                      {r.allocations.length > 0
                        ? r.allocations.map((a) => a.saleNumber ?? `#${a.saleId}`).join(", ")
                        : <span className="text-neutral-400">Advance</span>
                      }
                    </TableCell>
                    <TableCell>
                      <Badge variant={r.status === "posted" ? "success" : r.status === "voided" ? "danger" : "neutral"}>
                        {r.status}
                      </Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Close</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DueViewDialog({
  session,
  customer,
  onClose,
}: {
  session: string;
  customer: CustomerDto;
  onClose: () => void;
}) {
  const ledgerQuery = useQuery({
    queryKey: ["selling", "ledger", customer.id],
    queryFn: () => customerLedger(session, customer.id),
    enabled: !!session,
  });
  const ledger = (ledgerQuery.data ?? []) as CustomerLedgerEntryDto[];

  const ledgerLabels: Record<string, string> = {
    opening_balance: "Opening balance",
    sale: "Sale",
    payment: "Payment received",
    advance_used: "Advance used",
    advance_restore: "Advance restored",
    sale_cancellation: "Sale cancelled",
    payment_refund: "Payment refund",
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{customer.name}</DialogTitle>
          <DialogDescription>{customer.phone ?? "No phone"} · {customer.email ?? "No email"}</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-3 gap-3 text-sm">
          <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-3">
            <div className="text-neutral-500">Balance</div>
            <div className="text-lg font-semibold tabular-nums text-rose-600">{formatPkr(customer.balanceMinor)}</div>
          </div>
          <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-3">
            <div className="text-neutral-500">Advance</div>
            <div className="text-lg font-semibold tabular-nums text-forest-700">{formatPkr(customer.advanceMinor)}</div>
          </div>
          <div className="rounded-lg border border-neutral-200 bg-neutral-50 p-3">
            <div className="text-neutral-500">Credit limit</div>
            <div className="text-lg font-semibold tabular-nums">{formatPkr(customer.creditLimitMinor)}</div>
          </div>
        </div>
        {customer.address && (
          <p className="text-sm text-neutral-600">{customer.address}</p>
        )}
        <div className="grid gap-1.5">
          <Label>Ledger</Label>
          {ledgerQuery.isLoading ? (
            <div className="flex items-center justify-center py-4"><Loader2 className="h-4 w-4 animate-spin" /></div>
          ) : ledger.length === 0 ? (
            <p className="text-sm text-neutral-500">No ledger entries.</p>
          ) : (
            <div className="overflow-hidden rounded-lg border border-neutral-200">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead className="text-right">Amount</TableHead>
                    <TableHead className="text-right">Balance</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {ledger.map((e) => (
                    <TableRow key={e.id}>
                      <TableCell className="whitespace-nowrap text-xs">{e.createdAt.slice(0, 10)}</TableCell>
                      <TableCell className="text-xs">{ledgerLabels[e.entryType] ?? e.entryType}</TableCell>
                      <TableCell className={`text-right tabular-nums text-xs ${e.amountMinor >= 0 ? "text-forest-700" : "text-rose-600"}`}>
                        {e.amountMinor >= 0 ? "+" : ""}{formatPkr(e.amountMinor)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-xs">{formatPkr(e.balanceAfterMinor)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Close</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CustomerDialog({
  session,
  customer,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  customer: CustomerDto | null;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [name, setName] = React.useState(customer?.name ?? "");
  const [phone, setPhone] = React.useState(customer?.phone ?? "");
  const [email, setEmail] = React.useState(customer?.email ?? "");
  const [address, setAddress] = React.useState(customer?.address ?? "");
  const [isActive, setIsActive] = React.useState(customer?.isActive ?? true);

  const mutation = useMutation({
    mutationFn: () => {
      const code = customer?.code ?? "";
      const input: CustomerInput = {
        code,
        name: name.trim(),
        phone: phone.trim() || null,
        email: email.trim() || null,
        address: address.trim() || null,
        creditLimitMinor: customer?.creditLimitMinor ?? 0,
        creditDays: customer?.creditDays ?? 30,
      };
      if (customer) {
        return customerUpdate(session, customer.id, { ...input, isActive });
      }
      return customerCreate(session, { ...input, openingBalanceMinor: 0 });
    },
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title={customer ? `Edit customer ${customer.name}` : "New customer"}
      description="Customer balances and advances are used in credit sales and receipts."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel={customer ? "Save customer" : "Add customer"}
      onClose={onClose}
      submitDisabled={name.trim().length === 0}
    >
      <div className="grid gap-1.5">
        <Label>Name</Label>
        <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Ahmed Khan" />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Phone</Label>
          <Input value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="0300 1234567" />
        </div>
        <div className="grid gap-1.5">
          <Label>Email</Label>
          <Input value={email} onChange={(e) => setEmail(e.target.value)} placeholder="a.khan@example.com" />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Address</Label>
        <Input value={address} onChange={(e) => setAddress(e.target.value)} placeholder="House 4, Street 5, Gulberg" />
      </div>
      {customer && (
        <label className="flex items-center gap-2 text-sm text-neutral-700">
          <input
            type="checkbox"
            checked={isActive}
            onChange={(e) => setIsActive(e.target.checked)}
            className="h-4 w-4 rounded border-neutral-300"
          />
          Customer is active
        </label>
      )}
    </FormDialog>
  );
}

type LedgerMode =
  | { tab: "view" }
  | { tab: "receipt" }
  | { tab: "void"; payment: CustomerPaymentDto };

function LedgerDialog({
  session,
  customer,
  onClose,
  onError,
}: {
  session: string;
  customer: CustomerDto;
  onClose: () => void;
  onError: (e: Error) => void;
}) {
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const { hasPermission } = useSession();
  const canReceive = hasPermission("payment.receive");
  const [section, setSection] = React.useState<"ledger" | "receipts" | "statement">("ledger");
  const [mode, setMode] = React.useState<LedgerMode>({ tab: "view" });
  const [statementRange, setStatementRange] = React.useState<{ from: string; to: string }>(() => {
    const now = new Date();
    return { from: todayIso(new Date(now.getFullYear(), now.getMonth(), 1)), to: todayIso(now) };
  });

  const ledgerQuery = useQuery({
    queryKey: ["selling", "ledger", customer.id],
    queryFn: () => customerLedger(session, customer.id),
    enabled: !!session,
  });
  const receiptsQuery = useQuery({
    queryKey: ["selling", "receipts", customer.id],
    queryFn: () => customerReceiptList(session, customer.id),
    enabled: !!session,
  });
  const statementQuery = useQuery({
    queryKey: ["selling", "statement", customer.id, statementRange.from, statementRange.to],
    queryFn: () =>
      customerStatement(session, {
        customerId: customer.id,
        fromDate: statementRange.from,
        toDate: statementRange.to,
      }),
    enabled: !!session && statementRange.from.length > 0 && statementRange.to.length > 0 && statementRange.from <= statementRange.to,
  });

  const ledger = (ledgerQuery.data ?? []) as CustomerLedgerEntryDto[];
  const receipts = (receiptsQuery.data ?? []) as CustomerPaymentDto[];
  const statement = (statementQuery.data ?? null) as CustomerStatementDto | null;

  const afterLedgerChange = (message: string) => {
    void queryClient.invalidateQueries({ queryKey: ["selling"] });
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue"] });
    toast({ variant: "success", title: message });
    setMode({ tab: "view" });
  };

  if (mode.tab === "receipt") {
    return (
      <RecordReceiptDialog
        session={session}
        customer={customer}
        onClose={() => setMode({ tab: "view" })}
        onDone={() => afterLedgerChange("Receipt recorded")}
        onError={onError}
      />
    );
  }

  if (mode.tab === "void") {
    return (
      <VoidReceiptDialog
        session={session}
        payment={mode.payment}
        onClose={() => setMode({ tab: "view" })}
        onDone={() => afterLedgerChange("Receipt voided")}
        onError={onError}
      />
    );
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{customer.name}</DialogTitle>
          <DialogDescription>
            {customer.code} · credit limit {formatPkr(customer.creditLimitMinor)}
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-2 rounded-md bg-neutral-50 p-3 text-sm">
          <div>
            <p className="text-[11px] text-neutral-500">Balance</p>
            <p className="font-medium tabular-nums">{formatPkr(customer.balanceMinor)}</p>
          </div>
          <div>
            <p className="text-[11px] text-neutral-500">Advance</p>
            <p className="font-medium tabular-nums text-forest-700">{formatPkr(Math.max(0, customer.advanceMinor))}</p>
          </div>
        </div>

        <div className="flex items-center gap-1 border-b border-neutral-200">
          <TabButton active={section === "ledger"} onClick={() => setSection("ledger")} icon={<ClipboardList className="h-4 w-4" />}>
            Ledger
          </TabButton>
          <TabButton active={section === "receipts"} onClick={() => setSection("receipts")} icon={<Banknote className="h-4 w-4" />}>
            Receipts
          </TabButton>
          <TabButton active={section === "statement"} onClick={() => setSection("statement")} icon={<ClipboardList className="h-4 w-4" />}>
            Statement
          </TabButton>
          {canReceive && (
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="ml-auto mb-2"
              onClick={() => setMode({ tab: "receipt" })}
            >
              <Plus className="mr-1 h-3.5 w-3.5" />
              New receipt
            </Button>
          )}
        </div>

        <div className="max-h-80 overflow-y-auto">
          {section === "ledger" && (
            <>
              {ledgerQuery.isLoading ? (
                <LoadingRow />
              ) : ledger.length === 0 ? (
                <EmptyRow message="No ledger entries yet." />
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Date</TableHead>
                      <TableHead>Type</TableHead>
                      <TableHead className="text-right">Amount</TableHead>
                      <TableHead className="text-right">Balance</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {ledger.map((e) => (
                      <TableRow key={e.id}>
                        <TableCell className="whitespace-nowrap">{formatDateTime(e.createdAt)}</TableCell>
                        <TableCell>
                          <p className="text-sm text-neutral-900">{SALES_LEDGER_LABELS[e.entryType] ?? e.entryType}</p>
                          {e.notes && <p className="text-[11px] text-neutral-500">{e.notes}</p>}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">{formatPkr(e.amountMinor)}</TableCell>
                        <TableCell className="text-right tabular-nums">{formatPkr(e.balanceAfterMinor)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </>
          )}
          {section === "receipts" && (
            <>
              {receiptsQuery.isLoading ? (
                <LoadingRow />
              ) : receipts.length === 0 ? (
                <EmptyRow message="No receipts yet." />
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Receipt</TableHead>
                      <TableHead>Date</TableHead>
                      <TableHead>Method</TableHead>
                      <TableHead className="text-right">Amount</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {receipts.map((p) => (
                      <TableRow key={p.id}>
                        <TableCell className="font-medium text-neutral-900">
                          {p.receiptNumber ?? `#${p.id}`}
                          {p.saleId ? <span className="ml-1 text-[11px] text-neutral-400">(on sale)</span> : null}
                        </TableCell>
                        <TableCell className="whitespace-nowrap">{p.paymentDate}</TableCell>
                        <TableCell>{p.paymentMethodName}</TableCell>
                        <TableCell className="text-right tabular-nums">{formatPkr(p.amountMinor)}</TableCell>
                        <TableCell>
                          {p.status === "posted" ? <Badge variant="success">Posted</Badge> : <Badge variant="danger">Voided</Badge>}
                        </TableCell>
                        <TableCell className="text-right">
                          {canReceive && p.status === "posted" && (
                            <div className="flex items-center justify-end gap-2">
                              <PrintReceiptButton session={session} payment={p} onError={onError} />
                              <Button
                                variant="outline"
                                size="sm"
                                className="text-rose-600"
                                onClick={() => setMode({ tab: "void", payment: p })}
                              >
                                Void
                              </Button>
                            </div>
                          )}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </>
          )}
        {section === "statement" && (
            <>
              <div className="flex flex-wrap items-end gap-2 pb-2">
                <div className="grid gap-1">
                  <Label className="text-[11px]">From</Label>
                  <Input type="date" value={statementRange.from} onChange={(e) => setStatementRange({ ...statementRange, from: e.target.value })} />
                </div>
                <div className="grid gap-1">
                  <Label className="text-[11px]">To</Label>
                  <Input type="date" value={statementRange.to} onChange={(e) => setStatementRange({ ...statementRange, to: e.target.value })} />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setSection("ledger")}
                >
                  View full ledger
                </Button>
              </div>
              {statementQuery.isLoading ? (
                <LoadingRow />
              ) : !statement ? (
                <EmptyRow message="No statement for this range." />
              ) : (
                <>
                  <div className="grid grid-cols-3 gap-2 rounded-md bg-neutral-50 p-3 text-sm">
                    <div>
                      <p className="text-[11px] text-neutral-500">Opening</p>
                      <p className="font-medium tabular-nums">{formatPkr(statement.openingBalanceMinor)}</p>
                    </div>
                    <div>
                      <p className="text-[11px] text-neutral-500">Movement</p>
                      <p className="font-medium tabular-nums">{formatPkr(statement.closingBalanceMinor - statement.openingBalanceMinor)}</p>
                    </div>
                    <div>
                      <p className="text-[11px] text-neutral-500">Closing</p>
                      <p className="font-medium tabular-nums text-forest-700">{formatPkr(Math.max(0, statement.closingBalanceMinor))}</p>
                    </div>
                  </div>
                  {statement.entries.length === 0 ? (
                    <EmptyRow message="No entries in this range." />
                  ) : (
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead>Date</TableHead>
                          <TableHead>Type</TableHead>
                          <TableHead className="text-right">Amount</TableHead>
                          <TableHead className="text-right">Balance</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {statement.entries.map((e) => (
                          <TableRow key={e.id}>
                            <TableCell className="whitespace-nowrap">{formatDateTime(e.createdAt)}</TableCell>
                            <TableCell>
                              <p className="text-sm text-neutral-900">{SALES_LEDGER_LABELS[e.entryType] ?? e.entryType}</p>
                              {e.notes && <p className="text-[11px] text-neutral-500">{e.notes}</p>}
                            </TableCell>
                            <TableCell className="text-right tabular-nums">{formatPkr(e.amountMinor)}</TableCell>
                            <TableCell className="text-right tabular-nums">{formatPkr(e.balanceAfterMinor)}</TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  )}
                  <div className="flex justify-end pt-2">
                    <CopyReminderButton
                      text={statementReminderText(customer, Math.max(0, statement.closingBalanceMinor), statementRange.to)}
                      label="Copy reminder"
                    />
                  </div>
                </>
              )}
            </>
          )}
        </div>

        <DialogFooter className="items-center justify-between">
          <p className="text-xs text-neutral-400">
            {receipts.filter((r) => r.status === "posted").reduce((acc, r) => acc + r.amountMinor, 0) > 0
              ? `Received ${formatPkr(receipts.filter((r) => r.status === "posted").reduce((acc, r) => acc + r.amountMinor, 0))}`
              : "No posted receipts."}
          </p>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RecordReceiptDialog({
  session,
  customer,
  onClose,
  onDone,
  onError,
}: DialogProps & { customer: CustomerDto }) {
  const [amount, setAmount] = React.useState(0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [paymentDate, setPaymentDate] = React.useState(todayIso());
  const [notes, setNotes] = React.useState("");
  const [allocations, setAllocations] = React.useState<CustomerReceiptAllocationInput[]>([]);

  const methodsQuery = useQuery({
    queryKey: ["selling", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session,
  });
  const accountsQuery = useQuery({
    queryKey: ["selling", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session,
  });
  const previewQuery = useQuery({
    queryKey: ["selling", "receipt-preview", customer.id, amount],
    queryFn: () => customerReceiptPreview(session, { customerId: customer.id, amountMinor: amount }),
    enabled: amount > 0,
  });

  React.useEffect(() => {
    if (previewQuery.data) {
      setAllocations(previewQuery.data.allocations.map((a) => ({ saleId: a.saleId, amountMinor: a.allocatedMinor })));
    }
  }, [previewQuery.data]);

  const previewData = (previewQuery.data ?? null) as ReceiptPreviewDto | null;
  const dueMap = React.useMemo(() => {
    const m = new Map<number, number>();
    for (const a of previewData?.allocations ?? []) m.set(a.saleId, a.dueMinor);
    return m;
  }, [previewData]);
  const allocMap = React.useMemo(() => {
    const m = new Map<number, number>();
    for (const a of allocations) m.set(a.saleId, a.amountMinor);
    return m;
  }, [allocations]);
  const submitAllocations: CustomerReceiptAllocationInput[] | null =
    previewData?.allocations.map((a) => ({ saleId: a.saleId, amountMinor: allocMap.get(a.saleId) ?? a.allocatedMinor })) ?? null;
  const allocatedSum = (submitAllocations ?? []).reduce((s, a) => s + Math.max(0, a.amountMinor), 0);
  const overDue = (submitAllocations ?? []).some((a) => a.amountMinor > (dueMap.get(a.saleId) ?? 0));

  const mutation = useMutation({
    mutationFn: () =>
      customerReceiptCreate(session, {
        customerId: customer.id,
        paymentMethodId: methodId!,
        cashAccountId: accountId!,
        paymentDate,
        amountMinor: amount,
        notes: notes.trim() || null,
        allocations: submitAllocations ?? [],
        idempotencyKey: `receipt-${customer.id}-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  const previewReady = amount > 0 && previewQuery.isSuccess && !previewQuery.isFetching;
  const valid =
    amount > 0 &&
    methodId !== null &&
    accountId !== null &&
    paymentDate.length > 0 &&
    previewReady &&
    allocatedSum <= amount &&
    !overDue;

  const setAllocation = (saleId: number, v: number) =>
    setAllocations((prev) => [...prev.filter((x) => x.saleId !== saleId), { saleId, amountMinor: v }]);

  return (
    <FormDialog
      title="Record receipt"
      description={`Receive payment from ${customer.name} (balance ${formatPkr(customer.balanceMinor)}).`}
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Record receipt"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="grid gap-1.5">
        <Label>Amount</Label>
        <MoneyInput value={amount} onCommit={(v) => setAmount(v < 0 ? 0 : v)} placeholder="0.00" />
      </div>
      {amount > 0 && (previewQuery.isPending || previewQuery.isFetching) ? (
        <div className="flex items-center gap-2 text-sm text-neutral-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          Calculating allocation…
        </div>
      ) : null}
      {amount > 0 && previewQuery.isError ? (
        <p className="text-xs text-rose-600">Could not preview allocation. Please try again.</p>
      ) : null}
      {previewReady ? (
        <div className="overflow-hidden rounded-md border border-neutral-200">
          <div className="border-b border-neutral-200 bg-neutral-50 px-3 py-2 text-xs font-medium text-neutral-600">
            Apply to open invoices
          </div>
          <div className="divide-y divide-neutral-100">
            {previewData!.allocations.length === 0 ? (
              <p className="px-3 py-2 text-sm text-neutral-500">No open invoices — the full amount will be held as advance.</p>
            ) : (
              previewData!.allocations.map((a) => (
                <div key={a.saleId} className="flex items-center gap-2 px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium text-neutral-900">{a.saleNumber ?? `Invoice #${a.saleId}`}</p>
                    <p className="text-[11px] text-neutral-500">Due {formatPkr(a.dueMinor)}</p>
                  </div>
                  <MoneyInput
                    className="w-32"
                    value={allocMap.get(a.saleId) ?? a.allocatedMinor}
                    onCommit={(v) => setAllocation(a.saleId, v)}
                  />
                </div>
              ))
            )}
          </div>
          <div className="flex items-center justify-between border-t border-neutral-200 px-3 py-2 text-sm">
            <span className="text-neutral-500">Applied to invoices</span>
            <span className="font-medium tabular-nums">{formatPkr(Math.min(allocatedSum, amount))}</span>
          </div>
          <div className="flex items-center justify-between px-3 py-2 text-sm">
            <span className="text-neutral-500">Advances</span>
            <span className="font-medium tabular-nums text-forest-700">{formatPkr(Math.max(0, amount - allocatedSum))}</span>
          </div>
          {overDue ? (
            <p className="px-3 pb-2 text-xs text-rose-600">One or more lines exceed the amount due on that invoice.</p>
          ) : allocatedSum > amount ? (
            <p className="px-3 pb-2 text-xs text-rose-600">Allocations exceed the receipt amount.</p>
          ) : null}
        </div>
      ) : null}
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Payment method</Label>
          <Select value={methodId ? String(methodId) : ""} onValueChange={(v) => setMethodId(Number(v))}>
            <SelectTrigger>
              <SelectValue placeholder="Select a method" />
            </SelectTrigger>
            <SelectContent>
              {(methodsQuery.data ?? []).map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>
                  {m.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label>Cash account</Label>
          <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
            <SelectTrigger>
              <SelectValue placeholder="Select an account" />
            </SelectTrigger>
            <SelectContent>
              {(accountsQuery.data ?? []).map((a) => (
                <SelectItem key={a.id} value={String(a.id)}>
                  {a.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Payment date</Label>
        <Input type="date" value={paymentDate} onChange={(e) => setPaymentDate(e.target.value)} />
      </div>
      <div className="grid gap-1.5">
        <Label>Notes (optional)</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Deposit for dining set" />
      </div>
    </FormDialog>
  );
}

function VoidReceiptDialog({
  session,
  payment,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  payment: CustomerPaymentDto;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [reason, setReason] = React.useState("");
  const mutation = useMutation({
    mutationFn: () =>
      customerReceiptVoid(session, { paymentId: payment.id, reason: reason.trim() || null }),
    onSuccess: onDone,
    onError,
  });
  return (
    <FormDialog
      title={`Void receipt ${payment.receiptNumber ?? `#${payment.id}`}`}
      description="The receipt will be reversed and the customer account and cash corrected."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Void receipt"
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Reason</Label>
        <textarea
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="Entered by mistake…"
          rows={3}
          className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm"
        />
      </div>
    </FormDialog>
  );
}

// ---------------------------------------------------------------------------
// Furniture sets (bundles)
// ---------------------------------------------------------------------------

export function SetsTable({
  rows,
  loading,
  canManage,
  onEdit,
}: {
  rows: BundleDto[];
  loading: boolean;
  canManage: boolean;
  onEdit: (b: BundleDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No furniture sets yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12">Image</TableHead>
              <TableHead>Code</TableHead>
              <TableHead>Name</TableHead>
              <TableHead>Description</TableHead>
              <TableHead className="text-right">Default price</TableHead>
              <TableHead className="text-right">Cost estimate</TableHead>
              <TableHead className="text-right">Items</TableHead>
              <TableHead>Status</TableHead>
              {canManage && <TableHead className="text-right">Actions</TableHead>}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((b) => (
              <TableRow key={b.id}>
                <TableCell>
                  <div className="h-10 w-10 overflow-hidden rounded-md border border-neutral-200 bg-neutral-100">
                    {b.coverImagePath ? (
                      <StoredImage path={b.coverImagePath} className="h-full w-full object-cover" />
                    ) : (
                      <div className="flex h-full w-full items-center justify-center">
                        <Layers className="h-4 w-4 text-neutral-300" />
                      </div>
                    )}
                  </div>
                </TableCell>
                <TableCell className="font-medium text-neutral-900">{b.code}</TableCell>
                <TableCell>{b.name}</TableCell>
                <TableCell className="max-w-xs truncate">{b.description ?? "—"}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(b.defaultPriceMinor)}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(b.costEstimateMinor)}</TableCell>
                <TableCell className="text-right tabular-nums">{b.items.length}</TableCell>
                <TableCell>{b.isActive ? <Badge variant="success">Active</Badge> : <Badge variant="neutral">Inactive</Badge>}</TableCell>
                {canManage && (
                  <TableCell className="text-right">
                    <Button variant="outline" size="sm" onClick={() => onEdit(b)}>
                      <Pencil className="mr-1 h-3.5 w-3.5" />
                      Edit
                    </Button>
                  </TableCell>
                )}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

type BundleLine = { key: number; productId: number | null; quantity: string };
// initialProductName is carried alongside productId so the picker can pre-display it in edit mode
type BundleLineWithName = BundleLine & { initialProductName?: string; initialArticle?: string };

export function BundleDialog({
  session,
  bundle,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  bundle: BundleDto | null;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [code, setCode] = React.useState(bundle?.code ?? "");
  const [name, setName] = React.useState(bundle?.name ?? "");
  const [description, setDescription] = React.useState(bundle?.description ?? "");
  // When editing, coverImagePath is already a full absolute path returned by the backend.
  // When a new image is chosen, it is the raw filesystem path the user picked.
  const [coverImagePath, setCoverImagePath] = React.useState<string | null>(bundle?.coverImagePath ?? null);
  const [price, setPrice] = React.useState(bundle?.defaultPriceMinor ?? 0);
  const [isActive, setIsActive] = React.useState(bundle?.isActive ?? true);
  const [lines, setLines] = React.useState<BundleLineWithName[]>(
    bundle
      ? bundle.items.map((i, idx) => ({
          key: idx + 1,
          productId: i.productId,
          quantity: String(i.quantity),
          initialProductName: i.productName,
          initialArticle: i.articleNumber,
        }))
      : [{ key: 1, productId: null, quantity: "1" }],
  );
  const [saveError, setSaveError] = React.useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => {
      const input = {
        code: code.trim().toUpperCase(),
        name: name.trim(),
        description: description.trim() || null,
        // Pass the raw path; the backend detects whether it is a new filesystem
        // path (contains a separator) or a stored filename and handles accordingly.
        coverImagePath,
        defaultPriceMinor: price,
        isActive,
        items: lines
          .filter((l) => l.productId !== null && (Number(l.quantity) || 0) > 0)
          .map(
            (l): BundleItemInput => ({
              productId: l.productId!,
              quantity: Number(l.quantity),
            }),
          ),
      };
      if (bundle) {
        return bundleUpdate(session, bundle.id, input);
      }
      return bundleCreate(session, input);
    },
    onSuccess: () => {
      setSaveError(null);
      onDone();
    },
    onError: (e: Error) => {
      // Show the error inline without closing the form so the user can fix it
      setSaveError(commandErrorMessage(e));
      onError(e);
    },
  });

  const missingCode = code.trim().length === 0;
  const missingName = name.trim().length === 0;
  const missingItem = !lines.some((l) => l.productId !== null && (Number(l.quantity) || 0) > 0);
  const valid = !missingCode && !missingName && !missingItem;

  const validationHint = missingCode
    ? "Enter a set code."
    : missingName
    ? "Enter a set name."
    : missingItem
    ? "Search and select at least one product in the Items section, then confirm its quantity."
    : null;

  return (
    <FormDialog
      title={bundle ? `Edit set ${bundle.name}` : "New furniture set"}
      description="A set groups products that are sold together as one line."
      onSubmit={() => { setSaveError(null); mutation.mutate(); }}
      busy={mutation.isPending}
      submitLabel={bundle ? "Save set" : "Add set"}
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Code</Label>
          <Input value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="SET-001" />
        </div>
        <div className="grid gap-1.5">
          <Label>Name</Label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Bedroom suite" />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Description (optional)</Label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={2}
          placeholder="What the set includes…"
          className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm"
        />
      </div>
      <div className="grid gap-1.5">
        <Label>Full set image (optional)</Label>
        <div className="flex flex-wrap items-center gap-4">
          {coverImagePath && (
            <div className="relative h-20 w-20 overflow-hidden rounded-md border border-neutral-200 bg-neutral-100">
              {/* New pick: raw filesystem path (has separators). Existing: full abs path from backend — also has separators, so convertFileSrc works for preview. After save the backend stores and returns the managed path, so StoredImage takes over on next open. */}
              {coverImagePath.includes("/") || coverImagePath.includes("\\") ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img src={convertFileSrc(coverImagePath)} alt="Furniture set" className="h-full w-full object-cover" />
              ) : (
                <StoredImage path={coverImagePath} className="h-full w-full object-cover" />
              )}
              <button
                type="button"
                aria-label="Remove full set image"
                onClick={() => setCoverImagePath(null)}
                className="absolute right-1 top-1 rounded-full bg-white/80 p-1 text-neutral-600 hover:text-red-600"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          )}
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={async () => {
              const { open } = await import("@tauri-apps/plugin-dialog");
              const path = await open({
                multiple: false,
                filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] }],
              });
              if (typeof path === "string") setCoverImagePath(path);
            }}
          >
            <Upload className="mr-2 h-4 w-4" />
            Choose image
          </Button>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Default price</Label>
          <MoneyInput value={price} onCommit={(v) => setPrice(v < 0 ? 0 : v)} placeholder="0.00" />
        </div>
        <div className="grid gap-1.5">
          <Label>Status</Label>
          <Select value={isActive ? "active" : "inactive"} onValueChange={(v) => setIsActive(v === "active")}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="active">Active</SelectItem>
              <SelectItem value="inactive">Inactive</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      <BundleLinesEditor session={session} lines={lines} onChange={setLines} />
      {/* Inline guidance — shown below the items list, just above the footer buttons */}
      {validationHint && (
        <p className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          {validationHint}
        </p>
      )}
      {saveError && (
        <p className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700">
          Save failed: {saveError}
        </p>
      )}
    </FormDialog>
  );
}

function BundleLinesEditor({
  session,
  lines,
  onChange,
}: {
  session: string;
  lines: BundleLineWithName[];
  onChange: (lines: BundleLineWithName[]) => void;
}) {
  const nextKey = React.useRef(Date.now() + 1);
  const addLine = () => onChange([...lines, { key: nextKey.current++, productId: null, quantity: "1" }]);
  const update = (key: number, next: BundleLineWithName) => onChange(lines.map((l) => (l.key === key ? next : l)));
  const remove = (key: number) => onChange(lines.filter((l) => l.key !== key));

  return (
    <div className="grid gap-2">
      <Label>Items</Label>
      {lines.map((l) => (
        <BundleLineEditor key={l.key} session={session} value={l} onChange={(next) => update(l.key, next)} onRemove={() => remove(l.key)} />
      ))}
      <Button type="button" variant="outline" size="sm" onClick={addLine}>
        <Plus className="h-3.5 w-3.5" />
        Add item
      </Button>
    </div>
  );
}

function BundleLineEditor({
  session,
  value,
  onChange,
  onRemove,
}: {
  session: string;
  value: BundleLineWithName;
  onChange: (next: BundleLineWithName) => void;
  onRemove: () => void;
}) {
  // Build the initial product stub from the names carried in the line (edit mode).
  // useProductPicker will pre-select it so the picker shows the product name and
  // the line's productId is never cleared on mount.
  const initialProduct = React.useMemo<ProductListItemDto | null>(() => {
    if (value.productId !== null && value.initialProductName) {
      return {
        id: value.productId,
        name: value.initialProductName,
        articleNumber: value.initialArticle ?? "",
        // These fields are not needed for display; fill with safe defaults.
        categoryId: 0,
        category: "",
        productTypeId: null,
        productType: null,
        unit: null,
        salePriceMinor: 0,
        costMinor: null,
        minimumStock: 0,
        trackStock: false,
        isActive: true,
        archivedAt: null,
        primaryThumbnailPath: null,
      } as ProductListItemDto;
    }
    return null;
  }, []);

  const picker = useProductPicker(session, initialProduct);

  // Sync picker selection → line.productId.
  // Only update when picker.selected actually changes; do NOT clear productId
  // when picker.selected is still null (i.e. on first render before any interaction).
  const prevSelectedIdRef = React.useRef<number | null | undefined>(undefined);
  React.useEffect(() => {
    const newId = picker.selected?.id ?? null;
    if (prevSelectedIdRef.current === undefined) {
      // First render — picker was just initialised; record the starting id
      // but do not write back (avoids clearing a pre-populated productId).
      prevSelectedIdRef.current = newId;
      return;
    }
    if (newId !== prevSelectedIdRef.current) {
      prevSelectedIdRef.current = newId;
      onChange({ ...value, productId: newId });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [picker.selected]);

  return (
    <div className="grid gap-2 rounded-md border border-neutral-200 p-3">
      <div className="grid grid-cols-[1fr_auto] items-start gap-2">
        <ProductPicker picker={picker} label="Product" />
        <button type="button" onClick={onRemove} className="mt-5 text-xs text-neutral-400 underline hover:text-rose-600">
          remove
        </button>
      </div>
      <div className="grid grid-cols-[1fr_auto] items-end gap-2">
        <div className="grid gap-1.5">
          <Label>Quantity per set</Label>
          <Input
            inputMode="numeric"
            value={value.quantity}
            onChange={(e) => onChange({ ...value, quantity: e.target.value.replace(/[^0-9]/g, "") })}
            placeholder="1"
          />
        </div>
        <p className="pb-1 text-xs text-neutral-500">per set</p>
      </div>
    </div>
  );
}

export function BundleAvailabilityDialog({
  session,
  bundles,
  locations,
  onClose,
}: {
  session: string;
  bundles: BundleDto[];
  locations: LocationDto[];
  onClose: () => void;
}) {
  const [bundleId, setBundleId] = React.useState<number | null>(null);
  const locationId = locations[0]?.id ?? null;

  const availabilityQuery = useQuery({
    queryKey: ["selling", "bundle-availability", bundleId, locationId],
    queryFn: () => bundleAvailability(session, bundleId!, locationId!),
    enabled: !!session && bundleId !== null && locationId !== null,
  });

  const availability = availabilityQuery.data ?? null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Set availability</DialogTitle>
          <DialogDescription>Check how many complete sets can be built in the showroom.</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-2">
          <div className="grid gap-1.5">
            <Label>Furniture set</Label>
            <Select value={bundleId ? String(bundleId) : ""} onValueChange={(v) => setBundleId(Number(v))}>
              <SelectTrigger>
                <SelectValue placeholder="Select a set" />
              </SelectTrigger>
              <SelectContent>
                {bundles.map((b) => (
                  <SelectItem key={b.id} value={String(b.id)}>
                    {b.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-1.5">
            <Label>Showroom</Label>
            <div className="flex h-9 items-center rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm text-neutral-700">
              {locations[0]?.name ?? "Loading…"}
            </div>
          </div>
        </div>
        {availabilityQuery.isLoading && <LoadingRow />}
        {availabilityQuery.isError && (
          <p className="text-sm text-rose-600">{commandErrorMessage(availabilityQuery.error)}</p>
        )}
        {availability && (
          <div className="grid gap-2 rounded-md bg-neutral-50 p-3 text-sm">
            <div className="flex items-center justify-between">
              <span className="text-neutral-600">Complete sets available</span>
              <span className={cn("text-lg font-semibold tabular-nums", availability.availableCount > 0 ? "text-emerald-700" : "text-rose-600")}>
                {availability.availableCount}
              </span>
            </div>
            {availability.availableCount === 0 && availability.limitingProductName && (
              <p className="text-xs text-neutral-500">
                Limited by {availability.limitingProductName} — only {availability.limitingAvailable} available
                (needs {availability.limitingNeededPerSet} per set).
              </p>
            )}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

type DialogProps = {
  session: string;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
};

function FormDialog({
  title,
  description,
  children,
  onSubmit,
  busy,
  submitLabel = "Save",
  onClose,
  submitDisabled,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
  onSubmit: () => void;
  busy: boolean;
  submitLabel?: string;
  onClose: () => void;
  submitDisabled?: boolean;
}) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <form
          className="grid gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (!busy && !submitDisabled) onSubmit();
          }}
        >
          {children}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || submitDisabled}>
              {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {busy ? "Working…" : submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function SummaryCard({
  label,
  value,
  money,
  accent = "neutral",
}: {
  label: string;
  value: number;
  money?: boolean;
  accent?: "gold" | "rose" | "green" | "forest" | "neutral";
}) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-3 shadow-sm">
      <p className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">{label}</p>
      <p
        className={cn(
          "mt-1 text-xl font-semibold tabular-nums",
          accent === "gold" && "text-amber-600",
          accent === "rose" && "text-rose-600",
          accent === "green" && "text-emerald-600",
          accent === "forest" && "text-forest-700",
        )}
      >
        {money ? formatPkr(value) : value.toLocaleString()}
      </p>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  children,
}: {
  active: boolean;
  onClick: () => void;
  icon?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 whitespace-nowrap border-b-2 px-3 py-2 text-sm font-medium transition-colors",
        active
          ? "border-forest-600 text-forest-700"
          : "border-transparent text-neutral-500 hover:text-neutral-800",
      )}
    >
      {icon}
      {children}
    </button>
  );
}

function LoadingRow() {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-neutral-200 bg-white p-10 text-sm text-neutral-500">
      <Loader2 className="h-4 w-4 animate-spin" />
      Loading…
    </div>
  );
}

function EmptyRow({ message }: { message: string }) {
  return (
    <div className="rounded-lg border border-dashed border-neutral-300 bg-white p-10 text-center text-sm text-neutral-500">
      {message}
    </div>
  );
}

function useProductPicker(session: string, initialProduct: ProductListItemDto | null = null) {
  const [q, setQ] = React.useState("");
  const [results, setResults] = React.useState<ProductListItemDto[]>([]);
  // Pre-populate with the initial product so edit-mode rows show the selected
  // product name immediately without an extra API call.
  const [selected, setSelected] = React.useState<ProductListItemDto | null>(initialProduct);
  const [open, setOpen] = React.useState(false);

  React.useEffect(() => {
    if (!selected && q.trim()) {
      const t = window.setTimeout(() => {
        productList(session, { scope: "active", q: q.trim() })
          .then(setResults)
          .catch(() => setResults([]));
      }, 250);
      return () => window.clearTimeout(t);
    }
    if (!q.trim()) setResults([]);
  }, [q, selected, session]);

  return { q, setQ, results, selected, setSelected, open, setOpen };
}

function ProductPicker({
  picker,
  label,
}: {
  picker: ReturnType<typeof useProductPicker>;
  label: string;
}) {
  const { q, setQ, results, selected, setSelected, open, setOpen } = picker;
  const wrapperRef = React.useRef<HTMLDivElement>(null);

  // Close dropdown when user clicks outside this component.
  React.useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open, setOpen]);

  return (
    <div className="grid gap-1.5">
      <Label>{label}</Label>
      {selected ? (
        <div className="flex items-center justify-between gap-2 rounded-md border border-forest-200 bg-forest-50 px-3 py-2">
          <div className="grid gap-0.5">
            <span className="text-sm font-medium text-forest-800">{selected.name}</span>
            <span className="text-[11px] text-forest-600">{selected.articleNumber}</span>
          </div>
          <button
            type="button"
            className="text-xs text-neutral-500 underline hover:text-neutral-800"
            onClick={() => {
              setSelected(null);
              setQ("");
            }}
          >
            change
          </button>
        </div>
      ) : (
        <div className="relative" ref={wrapperRef}>
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setOpen(true);
            }}
            onFocus={() => setOpen(true)}
            placeholder="Search product…"
            className="pl-8"
          />
          {open && q.trim() && (
            <div className="absolute left-0 right-0 top-full z-20 mt-1 max-h-56 overflow-y-auto rounded-md border border-neutral-200 bg-white shadow-lg">
              {results.length === 0 ? (
                <p className="px-3 py-2 text-sm text-neutral-500">No matching products — keep typing…</p>
              ) : (
                results.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    className="flex w-full items-start gap-2 px-3 py-2 text-left hover:bg-neutral-50"
                    onMouseDown={(e) => {
                      // Use mousedown so the selection fires before the input loses focus
                      e.preventDefault();
                      setSelected(p);
                      setOpen(false);
                    }}
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-neutral-900">{p.name}</span>
                      <span className="block text-[11px] text-neutral-500">{p.articleNumber}</span>
                    </span>
                  </button>
                ))
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

