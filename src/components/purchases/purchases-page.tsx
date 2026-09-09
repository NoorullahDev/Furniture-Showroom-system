"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDownCircle,
  Banknote,
  ClipboardList,
  FileText,
  Loader2,
  Plus,
  Search,
  Truck,
  Undo2,
  UserPlus,
  Wallet,
} from "lucide-react";

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
import {
  cashAccountCreate,
  cashAccountList,
  cashEntryList,
  locationList,
  payableAging,
  paymentMethodList,
  productList,
  purchaseCreate,
  purchasePost,
  purchaseList,
  supplierCreate,
  supplierLedger,
  supplierList,
  supplierPaymentCreate,
  supplierPaymentList,
  supplierPaymentVoid,
  supplierReturnCreate,
  supplierReturnList,
  supplierReturnPost,
  type CashAccountDto,
  type CashEntryDto,
  type LocationDto,
  type PaymentMethodDto,
  type PayableAgingRowDto,
  type ProductListItemDto,
  type PurchaseDto,
  type SupplierDto,
  type SupplierPaymentDto,
  type SupplierReturnDto,
} from "@/lib/tauri/api";
import { formatDateTime, formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";

type Tab = "purchases" | "suppliers" | "payables" | "payments" | "returns" | "cash";

const LEDGER_ENTRY_LABELS: Record<string, string> = {
  opening_balance: "Opening balance",
  invoice: "Invoice",
  payment: "Payment",
  return: "Supplier return",
  void: "Payment void",
};

const CASH_ENTRY_LABELS: Record<string, string> = {
  opening: "Opening balance",
  purchase_payment: "Purchase payment",
  payment_void: "Payment void",
  supplier_refund: "Supplier refund",
};

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

type PurchaseLine = { key: number; productId: number | null; quantity: string; unitCostMinor: number };

export function PurchasesPage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canRecordPurchase = hasPermission("purchase.create");
  const canCreateSupplier = hasPermission("supplier.create");
  const canPay = hasPermission("supplier.pay");
  const canReturn = hasPermission("supplier.return");
  const canView = hasPermission("payable.view");

  const [view, setView] = React.useState<Tab>("purchases");
  const [dialog, setDialog] = React.useState<
    null | "purchase" | "post-purchase" | "supplier" | "pay" | "return" | "post-return" | "cash-account" | "ledger" | "purchase-detail"
  >(null);
  const [activePurchase, setActivePurchase] = React.useState<PurchaseDto | null>(null);
  const [activeReturn, setActiveReturn] = React.useState<SupplierReturnDto | null>(null);
  const [ledgerSupplierId, setLedgerSupplierId] = React.useState<number | null>(null);
  const [cashAccountFilter, setCashAccountFilter] = React.useState<number | null>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["purchasing"] });
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
  };

  const locationsQuery = useQuery({
    queryKey: ["purchasing", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session,
  });
  const suppliersQuery = useQuery({
    queryKey: ["purchasing", "suppliers"],
    queryFn: () => supplierList(session),
    enabled: !!session && canView,
  });
  const purchasesQuery = useQuery({
    queryKey: ["purchasing", "purchases"],
    queryFn: () => purchaseList(session),
    enabled: !!session && canView,
  });
  const payablesQuery = useQuery({
    queryKey: ["purchasing", "aging"],
    queryFn: () => payableAging(session),
    enabled: !!session && canView,
  });
  const methodsQuery = useQuery({
    queryKey: ["purchasing", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session && canView,
  });
  const accountsQuery = useQuery({
    queryKey: ["purchasing", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session && canView,
  });
  const paymentsQuery = useQuery({
    queryKey: ["purchasing", "payments"],
    queryFn: () => supplierPaymentList(session),
    enabled: !!session && canView,
  });
  const returnsQuery = useQuery({
    queryKey: ["purchasing", "returns"],
    queryFn: () => supplierReturnList(session),
    enabled: !!session && canView,
  });
  const cashEntriesQuery = useQuery({
    queryKey: ["purchasing", "cash-entries", cashAccountFilter],
    queryFn: () => cashEntryList(session, cashAccountFilter, 100),
    enabled: !!session && view === "cash" && canView,
  });

  const locations = locationsQuery.data ?? [];
  const suppliers = suppliersQuery.data ?? [];
  const purchases = purchasesQuery.data ?? [];
  const aging = payablesQuery.data ?? [];
  const methods = methodsQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];
  const payments = paymentsQuery.data ?? [];
  const returns = returnsQuery.data ?? [];
  const cashEntries = cashEntriesQuery.data ?? [];

  const posted = purchases.filter((p) => p.status === "posted");
  const totalPurchases = posted.reduce((acc, p) => acc + p.totalMinor, 0);
  const totalPaid = posted.reduce((acc, p) => acc + p.paidMinor, 0);
  const outstanding = aging.reduce((acc, r) => acc + r.dueMinor, 0);
  const cashOnHand = accounts.reduce((acc, a) => acc + a.balanceMinor, 0);

  const done = (message: string) => () => {
    invalidate();
    setDialog(null);
    setActivePurchase(null);
    setActiveReturn(null);
    toast({ variant: "success", title: message });
  };
  const failed = (e: Error) => {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    toast({ variant: "error", title: "Operation failed", description: commandErrorMessage(e) });
  };

  return (
    <div>
      <PageHeader
        title="Purchases"
        subtitle="Suppliers, purchases, payables and supplier cash movements."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            {canView && (
              <Select
                value={cashAccountFilter ? String(cashAccountFilter) : "all"}
                onValueChange={(v) => setCashAccountFilter(v === "all" ? null : Number(v))}
              >
                <SelectTrigger className="w-44">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All cash accounts</SelectItem>
                  {accounts.map((a) => (
                    <SelectItem key={a.id} value={String(a.id)}>
                      {a.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {canCreateSupplier && (
              <Button variant="outline" onClick={() => setDialog("supplier")}>
                <UserPlus className="h-4 w-4" />
                New supplier
              </Button>
            )}
            {canPay && (
              <Button variant="outline" onClick={() => setDialog("pay")}>
                <Banknote className="h-4 w-4" />
                Pay supplier
              </Button>
            )}
            {canReturn && (
              <Button variant="outline" onClick={() => setDialog("return")}>
                <ArrowDownCircle className="h-4 w-4" />
                Register return
              </Button>
            )}
            {canRecordPurchase && (
              <Button onClick={() => setDialog("purchase")}>
                <Plus className="h-4 w-4" />
                Record purchase
              </Button>
            )}
          </div>
        }
      />

      <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
        <SummaryCard label="Total purchases" value={totalPurchases} money accent="forest" />
        <SummaryCard label="Total paid" value={totalPaid} money accent="green" />
        <SummaryCard label="Outstanding payables" value={outstanding} money accent="rose" />
        <SummaryCard label="Cash on hand" value={cashOnHand} money accent="gold" />
        <SummaryCard label="Suppliers" value={suppliers.length} accent="neutral" />
        <SummaryCard label="Payments" value={payments.length} accent="neutral" />
      </div>

      <div className="mt-5 flex items-center gap-1 overflow-x-auto border-b border-neutral-200">
        <TabButton active={view === "purchases"} onClick={() => setView("purchases")} icon={<Truck className="h-4 w-4" />}>
          Purchases
        </TabButton>
        <TabButton active={view === "suppliers"} onClick={() => setView("suppliers")} icon={<ClipboardList className="h-4 w-4" />}>
          Suppliers
        </TabButton>
        <TabButton active={view === "payables"} onClick={() => setView("payables")} icon={<Search className="h-4 w-4" />}>
          Payables {outstanding > 0 && <Badge variant="danger" className="ml-1 h-5 px-1.5 text-xs">{formatPkr(outstanding)}</Badge>}
        </TabButton>
        <TabButton active={view === "payments"} onClick={() => setView("payments")} icon={<Banknote className="h-4 w-4" />}>
          Payments
        </TabButton>
        <TabButton active={view === "returns"} onClick={() => setView("returns")} icon={<ArrowDownCircle className="h-4 w-4" />}>
          Returns
        </TabButton>
        <TabButton active={view === "cash"} onClick={() => setView("cash")} icon={<Wallet className="h-4 w-4" />}>
          Cash accounts
        </TabButton>
      </div>

      <div className="mt-5">
        {view === "purchases" && (
          <PurchasesTable
            rows={purchases}
            loading={purchasesQuery.isLoading}
            canPost={canRecordPurchase}
            onView={(p) => {
              setActivePurchase(p);
              setDialog("purchase-detail");
            }}
            onPost={(p) => {
              setActivePurchase(p);
              setDialog("post-purchase");
            }}
          />
        )}
        {view === "suppliers" && (
          <SuppliersTable
            rows={suppliers}
            loading={suppliersQuery.isLoading}
            canViewLedger={canView}
            onLedger={(id) => {
              setLedgerSupplierId(id);
              setDialog("ledger");
            }}
          />
        )}
        {view === "payables" && <AgingTable rows={aging} loading={payablesQuery.isLoading} />}
        {view === "payments" && (
          <PaymentsTable
            rows={payments}
            loading={paymentsQuery.isLoading}
            session={session}
            canVoid={canPay}
            onVoided={() => invalidate()}
          />
        )}
        {view === "returns" && (
          <ReturnsTable
            rows={returns}
            loading={returnsQuery.isLoading}
            canPost={canReturn}
            onPost={(r) => {
              setActiveReturn(r);
              setDialog("post-return");
            }}
          />
        )}
        {view === "cash" && (
          <CashView
            accounts={accounts}
            loading={accountsQuery.isLoading}
            canCreate={canRecordPurchase}
            entries={cashEntries}
            entriesLoading={cashEntriesQuery.isLoading}
            filter={cashAccountFilter}
            onFilter={setCashAccountFilter}
            onCreate={() => setDialog("cash-account")}
          />
        )}
      </div>

      {dialog === "purchase" && canRecordPurchase && (
        <RecordPurchaseDialog
          session={session}
          suppliers={suppliers}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={done("Purchase recorded")}
          onError={failed}
        />
      )}
      {dialog === "post-purchase" && canRecordPurchase && activePurchase && (
        <PostPurchaseDialog
          session={session}
          purchase={activePurchase}
          methods={methods}
          accounts={accounts}
          onClose={() => {
            setDialog(null);
            setActivePurchase(null);
          }}
          onDone={done("Purchase posted")}
          onError={failed}
        />
      )}
      {dialog === "purchase-detail" && activePurchase && (
        <PurchaseDetailDialog
          purchase={activePurchase}
          onClose={() => {
            setDialog(null);
            setActivePurchase(null);
          }}
        />
      )}
      {dialog === "supplier" && canCreateSupplier && (
        <SupplierDialog
          session={session}
          onClose={() => setDialog(null)}
          onDone={() => done("Supplier added")()}
          onError={failed}
        />
      )}
      {dialog === "ledger" && ledgerSupplierId !== null && (
        <LedgerDialog
          session={session}
          suppliers={suppliers}
          supplierId={ledgerSupplierId}
          onClose={() => {
            setDialog(null);
            setLedgerSupplierId(null);
          }}
        />
      )}
      {dialog === "pay" && canPay && (
        <PaySupplierDialog
          session={session}
          suppliers={suppliers}
          methods={methods}
          accounts={accounts}
          onClose={() => setDialog(null)}
          onDone={() => done("Payment recorded")()}
          onError={failed}
        />
      )}
      {dialog === "return" && canReturn && (
        <ReturnDialog
          session={session}
          suppliers={suppliers}
          purchases={posted}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={done("Return registered")}
          onError={failed}
        />
      )}
      {dialog === "post-return" && canReturn && activeReturn && (
        <PostReturnDialog
          session={session}
          returnItem={activeReturn}
          onClose={() => {
            setDialog(null);
            setActiveReturn(null);
          }}
          onDone={() => done("Return posted")()}
          onError={failed}
        />
      )}
      {dialog === "cash-account" && canRecordPurchase && (
        <CashAccountDialog
          session={session}
          onClose={() => setDialog(null)}
          onDone={() => done("Cash account added")()}
          onError={failed}
        />
      )}
    </div>
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

function StatusBadge({ status }: { status: string }) {
  if (status === "posted") return <Badge variant="success">Posted</Badge>;
  if (status === "voided") return <Badge variant="danger">Voided</Badge>;
  return <Badge variant="neutral">Draft</Badge>;
}

function PurchasesTable({
  rows,
  loading,
  canPost,
  onView,
  onPost,
}: {
  rows: PurchaseDto[];
  loading: boolean;
  canPost: boolean;
  onView: (p: PurchaseDto) => void;
  onPost: (p: PurchaseDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No purchases recorded yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Invoice</TableHead>
              <TableHead>Supplier</TableHead>
              <TableHead>Date</TableHead>
              <TableHead className="text-right">Total</TableHead>
              <TableHead className="text-right">Paid</TableHead>
              <TableHead className="text-right">Due</TableHead>
              <TableHead className="text-right">Status</TableHead>
              <TableHead className="w-28" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((p) => (
              <TableRow key={p.id}>
                <TableCell>
                  <button
                    type="button"
                    onClick={() => onView(p)}
                    className="text-left font-medium text-neutral-900 hover:text-forest-700 hover:underline"
                  >
                    {p.purchaseNumber ?? p.invoiceNumber}
                  </button>
                  <span className="block text-[11px] text-neutral-500">{p.invoiceNumber}</span>
                </TableCell>
                <TableCell>
                  <span className="font-medium text-neutral-700">{p.supplierName}</span>
                </TableCell>
                <TableCell className="whitespace-nowrap text-neutral-500">{p.invoiceDate}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(p.totalMinor)}</TableCell>
                <TableCell className="text-right tabular-nums text-emerald-700">
                  {formatPkr(p.paidMinor)}
                </TableCell>
                <TableCell className="text-right font-semibold tabular-nums text-rose-600">
                  {formatPkr(p.dueMinor)}
                </TableCell>
                <TableCell className="text-right">
                  <StatusBadge status={p.status} />
                </TableCell>
                <TableCell className="text-right">
                  {canPost && p.status === "draft" && (
                    <Button variant="outline" size="sm" onClick={() => onPost(p)}>
                      Post
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function SuppliersTable({
  rows,
  loading,
  canViewLedger,
  onLedger,
}: {
  rows: SupplierDto[];
  loading: boolean;
  canViewLedger: boolean;
  onLedger: (id: number) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No suppliers yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Supplier</TableHead>
              <TableHead>Contact</TableHead>
              <TableHead className="text-right">Balance</TableHead>
              <TableHead>Active</TableHead>
              {canViewLedger && <TableHead className="w-20" />}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((s) => (
              <TableRow key={s.id}>
                <TableCell>
                  <span className="font-medium text-neutral-900">{s.name}</span>
                  <span className="block text-[11px] text-neutral-500">{s.code}</span>
                </TableCell>
                <TableCell className="text-neutral-600">
                  <span className="block">{s.phone ?? "—"}</span>
                  <span className="block text-[11px] text-neutral-500">{s.email ?? s.address ?? ""}</span>
                </TableCell>
                <TableCell
                  className={cn(
                    "text-right font-semibold tabular-nums",
                    s.balanceMinor > 0 ? "text-rose-600" : "text-emerald-700",
                  )}
                >
                  {formatPkr(s.balanceMinor)}
                </TableCell>
                <TableCell>
                  <Badge variant={s.isActive ? "success" : "neutral"}>
                    {s.isActive ? "Active" : "Inactive"}
                  </Badge>
                </TableCell>
                {canViewLedger && (
                  <TableCell className="text-right">
                    <Button variant="ghost" size="sm" onClick={() => onLedger(s.id)}>
                      <FileText className="h-3.5 w-3.5" />
                      Ledger
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

function AgingTable({ rows, loading }: { rows: PayableAgingRowDto[]; loading: boolean }) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No outstanding payables." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Invoice</TableHead>
              <TableHead>Supplier</TableHead>
              <TableHead>Invoice date</TableHead>
              <TableHead className="text-right">Age</TableHead>
              <TableHead>Bucket</TableHead>
              <TableHead className="text-right">Due</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((r) => (
              <TableRow key={r.purchaseId}>
                <TableCell>
                  <span className="font-medium text-neutral-900">{r.purchaseNumber ?? `#${r.purchaseId}`}</span>
                </TableCell>
                <TableCell className="font-medium text-neutral-700">{r.supplierName}</TableCell>
                <TableCell className="whitespace-nowrap text-neutral-500">{r.invoiceDate}</TableCell>
                <TableCell className="text-right tabular-nums text-neutral-600">{r.ageDays} days</TableCell>
                <TableCell>
                  <Badge
                    variant={
                      r.bucket === "90+" ? "danger" : r.bucket === "31-60" ? "warning" : r.bucket === "1-30" ? "info" : "success"
                    }
                  >
                    {r.bucket}
                  </Badge>
                </TableCell>
                <TableCell className="text-right font-semibold tabular-nums text-rose-600">
                  {formatPkr(r.dueMinor)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function PaymentsTable({
  rows,
  loading,
  session,
  canVoid,
  onVoided,
}: {
  rows: SupplierPaymentDto[];
  loading: boolean;
  session: string;
  canVoid: boolean;
  onVoided: () => void;
}) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const voidMutation = useMutation({
    mutationFn: (paymentId: number) =>
      supplierPaymentVoid(session, { paymentId, reason: "manually voided" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["purchasing"] });
      onVoided();
      toast({ variant: "success", title: "Payment voided" });
    },
    onError: (e: Error) => {
      toast({ variant: "error", title: "Void failed", description: commandErrorMessage(e) });
    },
  });

  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No supplier payments yet." />;

  const voidPayment = (paymentId: number) => {
    if (!window.confirm("Void this payment? Cash, payables and supplier balance will be reversed.")) return;
    voidMutation.mutate(paymentId);
  };

  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Payment</TableHead>
              <TableHead>Supplier</TableHead>
              <TableHead>Date</TableHead>
              <TableHead>Method</TableHead>
              <TableHead className="text-right">Amount</TableHead>
              <TableHead>Status</TableHead>
              {canVoid && <TableHead className="w-16" />}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((p) => (
              <TableRow key={p.id}>
                <TableCell>
                  <span className="font-medium text-neutral-900">{p.paymentNumber ?? `#${p.id}`}</span>
                </TableCell>
                <TableCell className="font-medium text-neutral-700">{p.supplierName}</TableCell>
                <TableCell className="whitespace-nowrap text-neutral-500">{p.paymentDate}</TableCell>
                <TableCell className="text-neutral-600">{p.paymentMethodName}</TableCell>
                <TableCell className="text-right font-semibold tabular-nums">{formatPkr(p.amountMinor)}</TableCell>
                <TableCell>
                  <StatusBadge status={p.status} />
                </TableCell>
                {canVoid && (
                  <TableCell className="text-right">
                    {p.status === "posted" && (
                      <Button
                        variant="ghost"
                        size="icon"
                        title="Void this payment"
                        onClick={() => voidPayment(p.id)}
                      >
                        <Undo2 className="h-3.5 w-3.5 text-neutral-500" />
                      </Button>
                    )}
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

function ReturnsTable({
  rows,
  loading,
  canPost,
  onPost,
}: {
  rows: SupplierReturnDto[];
  loading: boolean;
  canPost: boolean;
  onPost: (r: SupplierReturnDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No supplier returns registered." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Return</TableHead>
              <TableHead>Supplier</TableHead>
              <TableHead>Date</TableHead>
              <TableHead className="text-right">Total</TableHead>
              <TableHead className="text-right">Refund</TableHead>
              <TableHead className="text-right">Due reduction</TableHead>
              <TableHead className="text-right">Status</TableHead>
              {canPost && <TableHead className="w-28" />}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((r) => (
              <TableRow key={r.id}>
                <TableCell className="font-medium text-neutral-900">
                  {r.returnNumber ?? `#${r.id}`}
                </TableCell>
                <TableCell className="font-medium text-neutral-700">{r.supplierName}</TableCell>
                <TableCell className="whitespace-nowrap text-neutral-500">{r.returnDate}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(r.totalMinor)}</TableCell>
                <TableCell className="text-right tabular-nums text-emerald-700">{formatPkr(r.refundMinor)}</TableCell>
                <TableCell className="text-right tabular-nums text-rose-600">{formatPkr(r.dueReductionMinor)}</TableCell>
                <TableCell className="text-right">
                  <StatusBadge status={r.status} />
                </TableCell>
                {canPost && (
                  <TableCell className="text-right">
                    {r.status === "draft" && (
                      <Button variant="outline" size="sm" onClick={() => onPost(r)}>
                        Post
                      </Button>
                    )}
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

function CashView({
  accounts,
  loading,
  canCreate,
  entries,
  entriesLoading,
  filter,
  onFilter,
  onCreate,
}: {
  accounts: CashAccountDto[];
  loading: boolean;
  canCreate: boolean;
  entries: CashEntryDto[];
  entriesLoading: boolean;
  filter: number | null;
  onFilter: (id: number | null) => void;
  onCreate: () => void;
}) {
  if (loading) return <LoadingRow />;
  const active = filter
    ? accounts.find((a) => a.id === filter)
    : accounts.length === 1
      ? accounts[0]
      : null;

  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {accounts.length === 0 && (
          <div className="col-span-full">
            <EmptyRow message="No cash accounts yet." />
          </div>
        )}
        {accounts.map((a) => (
          <button
            key={a.id}
            type="button"
            onClick={() => onFilter(a.id === filter ? null : a.id)}
            className={cn(
              "rounded-lg border bg-white p-3 text-left shadow-sm transition-colors",
              a.id === filter ? "border-forest-400 ring-1 ring-forest-200" : "border-neutral-200",
            )}
          >
            <p className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">{a.name}</p>
            <p className="mt-1 text-lg font-semibold tabular-nums text-forest-700">{formatPkr(a.balanceMinor)}</p>
            <p className="text-[11px] capitalize text-neutral-500">{a.kind}</p>
          </button>
        ))}
      </div>
      {canCreate && (
        <div className="flex justify-end">
          <Button variant="outline" onClick={onCreate}>
            <Plus className="h-4 w-4" />
            New cash account
          </Button>
        </div>
      )}
      <div>
        <p className="mb-2 text-sm font-medium text-neutral-700">
          Cash entries{active ? ` — ${active.name}` : ""}
        </p>
        {entriesLoading ? (
          <LoadingRow />
        ) : entries.length === 0 ? (
          <EmptyRow message="No cash entries for this account." />
        ) : (
          <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead>Reference</TableHead>
                    <TableHead>Reason</TableHead>
                    <TableHead className="text-right">Amount</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {entries.map((e) => (
                    <TableRow key={e.id}>
                      <TableCell className="whitespace-nowrap text-neutral-500">
                        {formatDateTime(e.createdAt)}
                      </TableCell>
                      <TableCell>
                        <Badge variant={e.amountMinor >= 0 ? "success" : "warning"}>
                          {CASH_ENTRY_LABELS[e.entryType] ?? e.entryType}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-neutral-600">
                        {e.referenceType ? `${e.referenceType} #${e.referenceId ?? ""}` : "—"}
                      </TableCell>
                      <TableCell className="max-w-56 truncate text-neutral-500">{e.reason}</TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-semibold tabular-nums",
                          e.amountMinor >= 0 ? "text-emerald-700" : "text-rose-600",
                        )}
                      >
                        {e.amountMinor >= 0 ? "+" : ""}
                        {formatPkr(e.amountMinor)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

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

function useProductPicker(session: string) {
  const [q, setQ] = React.useState("");
  const [results, setResults] = React.useState<ProductListItemDto[]>([]);
  const [selected, setSelected] = React.useState<ProductListItemDto | null>(null);
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
        <div className="relative">
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
              {results.length === 0 && (
                <p className="px-3 py-2 text-sm text-neutral-500">No matching products</p>
              )}
              {results.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className="flex w-full items-start gap-2 px-3 py-2 text-left hover:bg-neutral-50"
                  onClick={() => {
                    setSelected(p);
                    setOpen(false);
                  }}
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-neutral-900">{p.name}</span>
                    <span className="block text-[11px] text-neutral-500">{p.articleNumber}</span>
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function LineEditor({
  session,
  value,
  onChange,
  onRemove,
}: {
  session: string;
  value: PurchaseLine;
  onChange: (next: PurchaseLine) => void;
  onRemove: () => void;
}) {
  const picker = useProductPicker(session);

  React.useEffect(() => {
    if (picker.selected && picker.selected.id !== value.productId) {
      onChange({ ...value, productId: picker.selected.id });
    } else if (!picker.selected && value.productId !== null) {
      onChange({ ...value, productId: null });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [picker.selected]);

  const qty = Number(value.quantity) || 0;
  const lineTotal = value.productId !== null ? qty * value.unitCostMinor : 0;

  return (
    <div className="grid gap-3 rounded-md border border-neutral-200 p-3">
      <div className="grid grid-cols-[1fr_auto] items-start gap-2">
        <ProductPicker picker={picker} label="Product" />
        <button
          type="button"
          onClick={onRemove}
          className="mt-5 text-xs text-neutral-400 underline hover:text-rose-600"
        >
          remove
        </button>
      </div>
      {value.productId !== null && (
        <>
          <div className="grid grid-cols-2 gap-2">
            <div className="grid gap-1.5">
              <Label>Quantity</Label>
              <Input
                inputMode="numeric"
                value={value.quantity}
                onChange={(e) => onChange({ ...value, quantity: e.target.value.replace(/[^0-9]/g, "") })}
                placeholder="10"
              />
            </div>
            <div className="grid gap-1.5">
              <Label>Unit cost</Label>
              <MoneyInput
                value={value.unitCostMinor}
                onCommit={(v) => onChange({ ...value, unitCostMinor: v })}
                placeholder="0.00"
              />
            </div>
          </div>
          <p className="text-right text-sm font-medium text-neutral-700">{formatPkr(lineTotal)}</p>
        </>
      )}
    </div>
  );
}

function PurchaseLinesEditor({
  session,
  lines,
  onChange,
}: {
  session: string;
  lines: PurchaseLine[];
  onChange: (lines: PurchaseLine[]) => void;
}) {
  const nextKey = React.useRef(Date.now() + 1);
  const addLine = () =>
    onChange([...lines, { key: nextKey.current++, productId: null, quantity: "1", unitCostMinor: 0 }]);
  const update = (key: number, next: PurchaseLine) =>
    onChange(lines.map((l) => (l.key === key ? next : l)));
  const remove = (key: number) => onChange(lines.filter((l) => l.key !== key));

  const total = lines.reduce(
    (acc, l) => acc + (l.productId !== null ? (Number(l.quantity) || 0) * l.unitCostMinor : 0),
    0,
  );

  return (
    <div className="grid gap-2">
      <Label>Items</Label>
      {lines.map((l) => (
        <LineEditor
          key={l.key}
          session={session}
          value={l}
          onChange={(next) => update(l.key, next)}
          onRemove={() => remove(l.key)}
        />
      ))}
      <div className="flex items-center justify-between">
        <Button type="button" variant="outline" size="sm" onClick={addLine}>
          <Plus className="h-3.5 w-3.5" />
          Add item
        </Button>
        {total > 0 && <p className="text-sm font-semibold text-neutral-800">{formatPkr(total)}</p>}
      </div>
    </div>
  );
}

type DialogProps = {
  session: string;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
};

function RecordPurchaseDialog({
  session,
  suppliers,
  locations,
  onClose,
  onDone,
  onError,
}: DialogProps & { suppliers: SupplierDto[]; locations: LocationDto[] }) {
  const [supplierId, setSupplierId] = React.useState<number | null>(null);
  const [locationId, setLocationId] = React.useState<number | null>(null);
  const [invoiceNumber, setInvoiceNumber] = React.useState("");
  const [invoiceDate, setInvoiceDate] = React.useState(todayIso());
  const [purchaseDate, setPurchaseDate] = React.useState(todayIso());
  const [notes, setNotes] = React.useState("");
  const [lines, setLines] = React.useState<PurchaseLine[]>([
    { key: 1, productId: null, quantity: "1", unitCostMinor: 0 },
  ]);

  const mutation = useMutation({
    mutationFn: () =>
      purchaseCreate(session, {
        supplierId: supplierId!,
        locationId: locationId!,
        invoiceNumber: invoiceNumber.trim(),
        invoiceDate,
        purchaseDate,
        notes: notes.trim() || null,
        items: lines
          .filter((l) => l.productId !== null && (Number(l.quantity) || 0) > 0)
          .map((l) => ({
            productId: l.productId!,
            quantity: Number(l.quantity),
            unitCostMinor: l.unitCostMinor,
          })),
      }),
    onSuccess: onDone,
    onError,
  });

  const valid =
    supplierId !== null &&
    locationId !== null &&
    invoiceNumber.trim().length > 0 &&
    lines.some((l) => l.productId !== null && (Number(l.quantity) || 0) > 0);

  return (
    <FormDialog
      title="Record purchase"
      description="A draft purchase invoice is saved. Post it afterwards to move stock and create the payable/cash effect."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Save draft"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <SupplierSelect suppliers={suppliers} value={supplierId} onChange={setSupplierId} />
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
      <div className="grid gap-1.5">
        <Label>Supplier invoice number</Label>
        <Input value={invoiceNumber} onChange={(e) => setInvoiceNumber(e.target.value)} placeholder="INV-1024" />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Invoice date</Label>
          <Input type="date" value={invoiceDate} onChange={(e) => setInvoiceDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Purchase date</Label>
          <Input type="date" value={purchaseDate} onChange={(e) => setPurchaseDate(e.target.value)} />
        </div>
      </div>
      <PurchaseLinesEditor session={session} lines={lines} onChange={setLines} />
      <div className="grid gap-1.5">
        <Label>Notes (optional)</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Special terms" />
      </div>
    </FormDialog>
  );
}

function PostPurchaseDialog({
  session,
  purchase,
  methods,
  accounts,
  onClose,
  onDone,
  onError,
}: DialogProps & { purchase: PurchaseDto; methods: PaymentMethodDto[]; accounts: CashAccountDto[] }) {
  const [paid, setPaid] = React.useState(0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [accountId, setAccountId] = React.useState<number | null>(null);

  const mutation = useMutation({
    mutationFn: () =>
      purchasePost(session, {
        purchaseId: purchase.id,
        idempotencyKey: `purchase-post-${purchase.id}-${Date.now()}`,
        paidMinor: paid,
        cashAccountId: paid > 0 ? accountId : null,
        paymentMethodId: paid > 0 ? methodId : null,
      }),
    onSuccess: onDone,
    onError,
  });

  const needsPaymentCtx = paid > 0;
  const valid = paid >= 0 && (!needsPaymentCtx || (methodId !== null && accountId !== null));

  return (
    <FormDialog
      title={`Post purchase ${purchase.purchaseNumber ?? purchase.invoiceNumber}`}
      description="Posts stock in, records the cost layers and creates the supplier payable."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Post purchase"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="grid gap-1.5">
        <Label>Supplier</Label>
        <Input readOnly value={purchase.supplierName} />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Invoice total</Label>
          <Input readOnly value={formatPkr(purchase.totalMinor)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Already paid</Label>
          <Input readOnly value={formatPkr(purchase.paidMinor)} />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Pay now</Label>
        <MoneyInput value={paid} onCommit={(v) => setPaid(v < 0 ? 0 : v)} placeholder="0.00" />
        <p className="text-[11px] text-neutral-500">
          Leave at 0.00 to book the full amount as credit payable.
        </p>
      </div>
      {needsPaymentCtx && (
        <>
          <div className="grid gap-1.5">
            <Label>Payment method</Label>
            <Select value={methodId ? String(methodId) : ""} onValueChange={(v) => setMethodId(Number(v))}>
              <SelectTrigger>
                <SelectValue placeholder="Select a method" />
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
          <div className="grid gap-1.5">
            <Label>Cash account</Label>
            <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
              <SelectTrigger>
                <SelectValue placeholder="Select an account" />
              </SelectTrigger>
              <SelectContent>
                {accounts.map((a) => (
                  <SelectItem key={a.id} value={String(a.id)}>
                    {a.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </>
      )}
    </FormDialog>
  );
}

function PurchaseDetailDialog({ purchase, onClose }: { purchase: PurchaseDto; onClose: () => void }) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{purchase.purchaseNumber ?? purchase.invoiceNumber}</DialogTitle>
          <DialogDescription>
            {purchase.supplierName} — invoice {purchase.invoiceNumber} on {purchase.invoiceDate}
          </DialogDescription>
        </DialogHeader>
        <ItemsTable rows={purchase.items.map((i) => ({ ...i, name: i.productName, article: i.articleNumber }))} />
        <DialogFooter className="items-center justify-between">
          <p className="text-sm">
            Total <span className="font-semibold tabular-nums">{formatPkr(purchase.totalMinor)}</span>
          </p>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ItemsTable({ rows }: { rows: { name: string; article: string; quantity: number; unitCostMinor: number; lineTotalMinor: number }[] }) {
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Product</TableHead>
              <TableHead className="text-right">Qty</TableHead>
              <TableHead className="text-right">Unit cost</TableHead>
              <TableHead className="text-right">Total</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((r, i) => (
              <TableRow key={i}>
                <TableCell>
                  <span className="font-medium text-neutral-900">{r.name}</span>
                  <span className="block text-[11px] text-neutral-500">{r.article}</span>
                </TableCell>
                <TableCell className="text-right tabular-nums">{r.quantity}</TableCell>
                <TableCell className="text-right tabular-nums">{formatPkr(r.unitCostMinor)}</TableCell>
                <TableCell className="text-right font-medium tabular-nums">{formatPkr(r.lineTotalMinor)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function SupplierDialog({ session, onClose, onDone, onError }: DialogProps) {
  const [code, setCode] = React.useState("");
  const [name, setName] = React.useState("");
  const [phone, setPhone] = React.useState("");
  const [email, setEmail] = React.useState("");
  const [address, setAddress] = React.useState("");
  const [openingBalance, setOpeningBalance] = React.useState(0);
  const [isActive, setIsActive] = React.useState(true);

  const mutation = useMutation({
    mutationFn: () =>
      supplierCreate(session, {
        code: code.trim(),
        name: name.trim(),
        phone: phone.trim() || null,
        email: email.trim() || null,
        address: address.trim() || null,
        openingBalanceMinor: openingBalance,
        isActive,
      }),
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title="New supplier"
      description="Suppliers can be billed with purchase invoices. An opening balance seeds the ledger as credit owed."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Add supplier"
      onClose={onClose}
      submitDisabled={code.trim().length === 0 || name.trim().length === 0}
    >
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Code</Label>
          <Input value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="SUP-001" />
        </div>
        <div className="grid gap-1.5">
          <Label>Name</Label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Woodcraft Mills" />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Phone</Label>
          <Input value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="0300 1234567" />
        </div>
        <div className="grid gap-1.5">
          <Label>Email</Label>
          <Input type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="billing@example.com" />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Address</Label>
        <Input value={address} onChange={(e) => setAddress(e.target.value)} placeholder="Industrial Area, Lahore" />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Opening balance (credit)</Label>
          <MoneyInput value={openingBalance} onCommit={(v) => setOpeningBalance(v < 0 ? 0 : v)} placeholder="0.00" />
        </div>
        <div className="grid gap-1.5">
          <Label>Active</Label>
          <Select value={isActive ? "true" : "false"} onValueChange={(v) => setIsActive(v === "true")}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="true">Active</SelectItem>
              <SelectItem value="false">Inactive</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </FormDialog>
  );
}

function LedgerDialog({
  session,
  suppliers,
  supplierId,
  onClose,
}: {
  session: string;
  suppliers: SupplierDto[];
  supplierId: number;
  onClose: () => void;
}) {
  const { data, isLoading } = useQuery({
    queryKey: ["purchasing", "supplier-ledger", supplierId],
    queryFn: () => supplierLedger(session, supplierId),
    enabled: !!session,
  });
  const supplier = suppliers.find((s) => s.id === supplierId);
  const rows = data ?? [];

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{supplier?.name ?? `Supplier #${supplierId}`}</DialogTitle>
          <DialogDescription>Statement of transactions for this supplier.</DialogDescription>
        </DialogHeader>
        {isLoading ? (
          <LoadingRow />
        ) : rows.length === 0 ? (
          <EmptyRow message="No ledger entries for this supplier." />
        ) : (
          <div className="overflow-hidden rounded-lg border border-neutral-200">
            <div className="overflow-x-auto">
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
                  {rows.map((e) => (
                    <TableRow key={e.id}>
                      <TableCell className="whitespace-nowrap text-neutral-500">{formatDateTime(e.createdAt)}</TableCell>
                      <TableCell>
                        <Badge variant={e.amountMinor >= 0 ? "danger" : "success"}>
                          {LEDGER_ENTRY_LABELS[e.entryType] ?? e.entryType}
                        </Badge>
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-medium tabular-nums",
                          e.amountMinor >= 0 ? "text-rose-600" : "text-emerald-700",
                        )}
                      >
                        {e.amountMinor >= 0 ? "+" : ""}
                        {formatPkr(e.amountMinor)}
                      </TableCell>
                      <TableCell className="text-right font-semibold tabular-nums">
                        {formatPkr(e.balanceAfterMinor)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
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

function PaySupplierDialog({
  session,
  suppliers,
  methods,
  accounts,
  onClose,
  onDone,
  onError,
}: DialogProps & { suppliers: SupplierDto[]; methods: PaymentMethodDto[]; accounts: CashAccountDto[] }) {
  const [supplierId, setSupplierId] = React.useState<number | null>(null);
  const [amount, setAmount] = React.useState(0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [paymentDate, setPaymentDate] = React.useState(todayIso());
  const [notes, setNotes] = React.useState("");

  const supplier = suppliers.find((s) => s.id === supplierId) ?? null;

  const mutation = useMutation({
    mutationFn: () =>
      supplierPaymentCreate(session, {
        supplierId: supplierId!,
        paymentMethodId: methodId!,
        cashAccountId: accountId!,
        paymentDate,
        amountMinor: amount,
        notes: notes.trim() || null,
        idempotencyKey: `pay-${supplierId}-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  const valid = supplierId !== null && amount > 0 && methodId !== null && accountId !== null;

  return (
    <FormDialog
      title="Pay supplier"
      description="A payment is allocated to the oldest posted purchase invoices first."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Record payment"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <SupplierSelect suppliers={suppliers} value={supplierId} onChange={setSupplierId} />
      {supplier && (
        <p className="text-[11px] text-neutral-500">
          Outstanding balance:{" "}
          <span className="font-semibold text-rose-600">{formatPkr(supplier.balanceMinor)}</span>
        </p>
      )}
      <div className="grid gap-1.5">
        <Label>Amount</Label>
        <MoneyInput value={amount} onCommit={(v) => setAmount(v < 0 ? 0 : v)} placeholder="0.00" />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Payment method</Label>
          <Select value={methodId ? String(methodId) : ""} onValueChange={(v) => setMethodId(Number(v))}>
            <SelectTrigger>
              <SelectValue placeholder="Select" />
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
        <div className="grid gap-1.5">
          <Label>Cash account</Label>
          <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
            <SelectTrigger>
              <SelectValue placeholder="Select" />
            </SelectTrigger>
            <SelectContent>
              {accounts.map((a) => (
                <SelectItem key={a.id} value={String(a.id)}>
                  {a.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Payment date</Label>
          <Input type="date" value={paymentDate} onChange={(e) => setPaymentDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Notes (optional)</Label>
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Cheque #" />
        </div>
      </div>
    </FormDialog>
  );
}

function ReturnDialog({
  session,
  suppliers,
  purchases,
  locations,
  onClose,
  onDone,
  onError,
}: DialogProps & {
  suppliers: SupplierDto[];
  purchases: PurchaseDto[];
  locations: LocationDto[];
}) {
  const [supplierId, setSupplierId] = React.useState<number | null>(null);
  const [purchaseId, setPurchaseId] = React.useState<number | null>(null);
  const [locationId, setLocationId] = React.useState<number | null>(null);
  const [returnDate, setReturnDate] = React.useState(todayIso());
  const [refund, setRefund] = React.useState(0);
  const [notes, setNotes] = React.useState("");
  const [lines, setLines] = React.useState<PurchaseLine[]>([
    { key: 1, productId: null, quantity: "1", unitCostMinor: 0 },
  ]);

  const supplierPurchases = purchases.filter((p) => p.supplierId === supplierId);
  const activePurchases = supplierId === null ? [] : supplierPurchases;

  const mutation = useMutation({
    mutationFn: () =>
      supplierReturnCreate(session, {
        supplierId: supplierId!,
        purchaseId,
        locationId: locationId!,
        returnDate,
        refundMinor: refund > 0 ? refund : null,
        notes: notes.trim() || null,
        items: lines
          .filter((l) => l.productId !== null && (Number(l.quantity) || 0) > 0)
          .map((l) => ({
            productId: l.productId!,
            quantity: Number(l.quantity),
            unitCostMinor: l.unitCostMinor,
          })),
      }),
    onSuccess: onDone,
    onError,
  });

  const valid =
    supplierId !== null &&
    locationId !== null &&
    lines.some((l) => l.productId !== null && (Number(l.quantity) || 0) > 0);

  return (
    <FormDialog
      title="Register supplier return"
      description="Returned goods reduce stock and the supplier balance. Post the draft to apply the effect."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Save draft"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <SupplierSelect suppliers={suppliers} value={supplierId} onChange={(id) => { setSupplierId(id); setPurchaseId(null); }} />
      {supplierId !== null && activePurchases.length > 0 && (
        <div className="grid gap-1.5">
          <Label>Original purchase (optional)</Label>
          <Select value={purchaseId ? String(purchaseId) : "none"} onValueChange={(v) => setPurchaseId(v === "none" ? null : Number(v))}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">Not tied to a purchase</SelectItem>
              {activePurchases.map((p) => (
                <SelectItem key={p.id} value={String(p.id)}>
                  {p.purchaseNumber ?? p.invoiceNumber} — {formatPkr(p.totalMinor)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Return date</Label>
          <Input type="date" value={returnDate} onChange={(e) => setReturnDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Refund from supplier</Label>
          <MoneyInput value={refund} onCommit={(v) => setRefund(v < 0 ? 0 : v)} placeholder="0.00" />
          <p className="text-[11px] text-neutral-500">Cash received back (optional).</p>
        </div>
      </div>
      <PurchaseLinesEditor session={session} lines={lines} onChange={setLines} />
      <div className="grid gap-1.5">
        <Label>Notes (optional)</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Defective batch" />
      </div>
    </FormDialog>
  );
}

function PostReturnDialog({
  session,
  returnItem,
  onClose,
  onDone,
  onError,
}: DialogProps & { returnItem: SupplierReturnDto }) {
  const mutation = useMutation({
    mutationFn: () =>
      supplierReturnPost(session, {
        returnId: returnItem.id,
        idempotencyKey: `return-post-${returnItem.id}-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title={`Post return ${returnItem.returnNumber ?? `#${returnItem.id}`}`}
      description="Removes returned stock from inventory and adjusts the supplier balance and payables."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Post return"
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Supplier</Label>
        <Input readOnly value={returnItem.supplierName} />
      </div>
      <ItemsTable
        rows={returnItem.items.map((i) => ({
          name: i.productName,
          article: i.articleNumber,
          quantity: i.quantity,
          unitCostMinor: i.unitCostMinor,
          lineTotalMinor: i.lineTotalMinor,
        }))}
      />
      <p className="text-sm">
        Total return{" "}
        <span className="font-semibold tabular-nums">{formatPkr(returnItem.totalMinor)}</span>
        {returnItem.refundMinor > 0 && (
          <span className="ml-3 text-emerald-700">Refund {formatPkr(returnItem.refundMinor)}</span>
        )}
      </p>
    </FormDialog>
  );
}

function CashAccountDialog({ session, onClose, onDone, onError }: DialogProps) {
  const [code, setCode] = React.useState("");
  const [name, setName] = React.useState("");
  const [kind, setKind] = React.useState("cash");
  const [openingBalance, setOpeningBalance] = React.useState(0);

  const mutation = useMutation({
    mutationFn: () =>
      cashAccountCreate(session, {
        code: code.trim(),
        name: name.trim(),
        kind,
        openingBalanceMinor: openingBalance,
      }),
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title="New cash account"
      description="Cash accounts track payments made to suppliers and other cash movements."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Add account"
      onClose={onClose}
      submitDisabled={code.trim().length === 0 || name.trim().length === 0}
    >
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Code</Label>
          <Input value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="CASH-01" />
        </div>
        <div className="grid gap-1.5">
          <Label>Name</Label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Main cash drawer" />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Kind</Label>
          <Select value={kind} onValueChange={setKind}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="cash">Cash</SelectItem>
              <SelectItem value="bank">Bank</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label>Opening balance</Label>
          <MoneyInput value={openingBalance} onCommit={(v) => setOpeningBalance(v < 0 ? 0 : v)} placeholder="0.00" />
        </div>
      </div>
    </FormDialog>
  );
}

function SupplierSelect({
  suppliers,
  value,
  onChange,
}: {
  suppliers: SupplierDto[];
  value: number | null;
  onChange: (id: number | null) => void;
}) {
  return (
    <div className="grid gap-1.5">
      <Label>Supplier</Label>
      <Select value={value ? String(value) : ""} onValueChange={(v) => onChange(Number(v))}>
        <SelectTrigger>
          <SelectValue placeholder="Select a supplier" />
        </SelectTrigger>
        <SelectContent>
          {suppliers.map((s) => (
            <SelectItem key={s.id} value={String(s.id)}>
              {s.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function LocationSelect({
  locations,
  value,
  onChange,
}: {
  locations: LocationDto[];
  value: number | null;
  onChange: (id: number | null) => void;
}) {
  return (
    <div className="grid gap-1.5">
      <Label>Location</Label>
      <Select value={value ? String(value) : ""} onValueChange={(v) => onChange(Number(v))}>
        <SelectTrigger>
          <SelectValue placeholder="Select a location" />
        </SelectTrigger>
        <SelectContent>
          {locations.map((l) => (
            <SelectItem key={l.id} value={String(l.id)}>
              {l.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}