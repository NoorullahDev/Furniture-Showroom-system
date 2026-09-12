"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Boxes,
  CheckCircle,
  ClipboardList,
  FileSpreadsheet,
  Loader2,
  PackageOpen,
  Plus,
  Search,
  Scale,
  Undo2,
  Wrench,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { StoredImage } from "@/components/catalogue/stored-image";
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
  locationList,
  productList,
  stockAdjust,
  stockBalanceList,
  stockCountLines,
  stockCountLineUpdate,
  stockCountList,
  stockCountPost,
  stockCountStart,
  stockDamage,
  stockLowList,
  stockMovementList,
  stockOpening,
  stockOpeningBatch,
  stockRelease,
  stockRepair,
  stockReserve,
  stockReverse,
  stockValuation,
  type CountSessionDto,
  type LocationDto,
  type LowStockItemDto,
  type OpeningBatchInput,
  type OpeningBatchResultDto,
  type ProductListItemDto,
  type StockBalanceDto,
  type StockMovementDto,
  type ValuationLineDto,
} from "@/lib/tauri/api";
import { formatDateTime, formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";
import { takeDashboardTarget } from "@/lib/dashboard-navigation";

type Tab = "stock" | "ledger" | "valuation" | "low" | "count";

const MOVEMENT_LABELS: Record<string, string> = {
  opening: "Opening stock",
  purchase_receipt: "Purchase receipt",
  sale_issue: "Sale issue",
  customer_return: "Customer return",
  supplier_return: "Supplier return",
  transfer_out: "Transfer out",
  transfer_in: "Transfer in",
  reservation: "Reservation",
  release: "Release",
  damage: "Damage",
  repair_recovery: "Repair recovery",
  adjustment: "Adjustment",
  cancellation_reversal: "Cancellation reversal",
  stock_count_correction: "Stock count",
};

export function InventoryPage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canMutate = hasPermission("inventory.create");
  const canViewValuation = hasPermission("inventory.valuation");

  const dashboardTarget = React.useMemo(() => takeDashboardTarget("inventory"), []);
  const [view, setView] = React.useState<Tab>(dashboardTarget?.target === "low-stock" ? "low" : "stock");
  const [historyProductId, setHistoryProductId] = React.useState<number | null>(null);
  const [dialog, setDialog] = React.useState<
    null | "opening" | "adjust" | "damage" | "reserve" | "count" | "import"
  >(null);

  const locationsQuery = useQuery({
    queryKey: ["inventory", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session,
  });
  const locationId = locationsQuery.data?.[0]?.id ?? null;

  const balancesQuery = useQuery({
    queryKey: ["inventory", "balances", locationId],
    queryFn: () => stockBalanceList(session, locationId),
    enabled: !!session && locationId !== null,
  });

  const movementsQuery = useQuery({
    queryKey: ["inventory", "movements", locationId, historyProductId],
    queryFn: () =>
      stockMovementList(session, { productId: historyProductId, locationId, limit: 200 }),
    enabled: !!session && locationId !== null,
  });

  const valuationQuery = useQuery({
    queryKey: ["inventory", "valuation"],
    queryFn: () => stockValuation(session),
    enabled: !!session && canViewValuation,
  });

  const canCount = hasPermission("inventory.count");
  const lowStockQuery = useQuery({
    queryKey: ["inventory", "low-stock"],
    queryFn: () => stockLowList(session),
    enabled: !!session,
  });
  const countSessionsQuery = useQuery({
    queryKey: ["inventory", "count-sessions"],
    queryFn: () => stockCountList(session, locationId),
    enabled: !!session && canCount && locationId !== null,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "products"] });
  };

  const locations = locationsQuery.data ?? [];
  const balances = balancesQuery.data ?? [];
  const movements = movementsQuery.data ?? [];
  const valuation = valuationQuery.data ?? [];
  const lowStock = lowStockQuery.data ?? [];
  const countSessions = countSessionsQuery.data ?? [];

  const historyProduct = React.useMemo(() => {
    if (historyProductId === null) return null;
    const balancesData = balancesQuery.data ?? [];
    const movementsData = movementsQuery.data ?? [];
    const b = balancesData.find((x) => x.productId === historyProductId);
    if (b) {
      return { name: b.productName, article: b.articleNumber, thumb: b.thumbnailPath };
    }
    const m = movementsData.find((x) => x.productId === historyProductId);
    if (m) {
      return { name: m.productName ?? "Product", article: m.articleNumber ?? "", thumb: null };
    }
    return { name: "Product", article: "", thumb: null };
  }, [historyProductId, balancesQuery.data, movementsQuery.data]);

  const totals = React.useMemo(() => {
    const t = (balancesQuery.data ?? []).reduce(
      (acc, b) => {
        acc.onHand += b.onHand;
        acc.reserved += b.reserved;
        acc.damaged += b.damaged;
        return acc;
      },
      { onHand: 0, reserved: 0, damaged: 0, available: 0 },
    );
    t.available = t.onHand - t.reserved - t.damaged;
    return t;
  }, [balancesQuery.data]);

  const totalValue = valuation.reduce((acc, v) => acc + v.valueMinor, 0);

  return (
    <div>
      <PageHeader
        title="Inventory"
        subtitle="Stock balances, movement ledger and FIFO valuation."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex h-9 items-center rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm text-neutral-700">
              {locations[0]?.name ?? "Loading…"}
            </div>
            {canMutate && (
              <>
                <Button variant="outline" onClick={() => setDialog("adjust")}>
                  <Scale className="h-4 w-4" />
                  Adjust
                </Button>
                <Button variant="outline" onClick={() => setDialog("damage")}>
                  <Wrench className="h-4 w-4" />
                  Damage
                </Button>
                <Button variant="outline" onClick={() => setDialog("reserve")}>
                  <ClipboardList className="h-4 w-4" />
                  Reserve
                </Button>
                <Button onClick={() => setDialog("opening")}>
                  <Plus className="h-4 w-4" />
                  Opening stock
                </Button>
                <Button variant="outline" onClick={() => setDialog("import")}>
                  <FileSpreadsheet className="h-4 w-4" />
                  Import openings
                </Button>
                <Button variant="outline" onClick={() => setDialog("count")}>
                  <CheckCircle className="h-4 w-4" />
                  New count
                </Button>
              </>
            )}
          </div>
        }
      />

      <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
        <SummaryCard label="On hand" value={totals.onHand} />
        <SummaryCard label="Reserved" value={totals.reserved} accent="gold" />
        <SummaryCard label="Damaged" value={totals.damaged} accent="rose" />
        <SummaryCard label="Available" value={totals.available} accent="green" />
        <SummaryCard label="Stock value" value={totalValue} money accent="forest" span />
        <SummaryCard label="Ledger entries" value={movements.length} accent="neutral" />
      </div>

      <div className="mt-5 flex items-center gap-1 border-b border-neutral-200">
        <TabButton active={view === "stock"} onClick={() => setView("stock")} icon={<Boxes className="h-4 w-4" />}>
          Stock balances
        </TabButton>
        <TabButton active={view === "ledger"} onClick={() => setView("ledger")} icon={<ClipboardList className="h-4 w-4" />}>
          Movement ledger
        </TabButton>
        <TabButton active={view === "low"} onClick={() => setView("low")} icon={<Search className="h-4 w-4" />}>
          Low stock {lowStock.length > 0 && <Badge variant="danger" className="ml-1 h-5 px-1.5 text-xs">{lowStock.length}</Badge>}
        </TabButton>
        {canCount && (
          <TabButton active={view === "count"} onClick={() => setView("count")} icon={<CheckCircle className="h-4 w-4" />}>
            Count sessions
          </TabButton>
        )}
        {canViewValuation && (
          <TabButton active={view === "valuation"} onClick={() => setView("valuation")} icon={<PackageOpen className="h-4 w-4" />}>
            Valuation (FIFO)
          </TabButton>
        )}
      </div>

      <div className="mt-4">
        {view === "stock" && (
          <BalancesTable
            rows={balances}
            loading={balancesQuery.isLoading}
            onViewHistory={(pid) => {
              setHistoryProductId(pid);
              setView("ledger");
            }}
          />
        )}
        {view === "ledger" && (
          <>
            {historyProduct && (
              <div className="mb-3 flex items-center justify-between rounded-lg border border-forest-200 bg-forest-50 px-3 py-2">
                <div className="flex items-center gap-3">
                  <StoredImage path={historyProduct.thumb} className="h-8 w-8 rounded-md object-cover" />
                  <div className="grid gap-0">
                    <span className="text-sm font-medium text-forest-800">{historyProduct.name}</span>
                    <span className="text-[11px] text-forest-600">{historyProduct.article} · stock history</span>
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setHistoryProductId(null);
                    setView("ledger");
                  }}
                >
                  Clear filter
                </Button>
              </div>
            )}
            <LedgerTable
              rows={movements}
              loading={movementsQuery.isLoading}
              session={session}
              canMutate={canMutate}
              onReverse={() => invalidate()}
              onViewHistory={(pid) => setHistoryProductId(pid)}
            />
          </>
        )}
        {view === "low" && (
          <LowStockTable rows={lowStock} loading={lowStockQuery.isLoading} />
        )}
        {view === "count" && canCount && (
          <CountSessionsView
            sessions={countSessions}
            loading={countSessionsQuery.isLoading}
            session={session}
            canMutate={canMutate}
          />
        )}
        {view === "valuation" && canViewValuation && (
          <ValuationTable rows={valuation} loading={valuationQuery.isLoading} />
        )}
      </div>

      {dialog === "opening" && canMutate && (
        <OpeningDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Opening stock posted" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Opening failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "import" && canMutate && (
        <OpeningImportDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={(result) => {
            invalidate();
            setDialog(null);
            if (result.errors.length > 0) {
              toast({
                variant: "error",
                title: "Import finished with errors",
                description: `${result.postedCount} posted, ${result.errors.length} errors`,
              });
            } else {
              toast({ variant: "success", title: "Opening stock imported", description: `${result.postedCount} rows posted` });
            }
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Import failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "adjust" && canMutate && (
        <AdjustDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Stock adjusted" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Adjustment failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "damage" && canMutate && (
        <DamageDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Damage classification updated" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Damage action failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "reserve" && canMutate && (
        <ReserveDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Reservation updated" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Reservation failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "count" && canCount && (
        <CountSessionDialog
          session={session}
          locationId={locationId}
          onClose={() => setDialog(null)}
          onDone={() => {
            void queryClient.invalidateQueries({ queryKey: ["inventory", "count-sessions"] });
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Count session created" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Count session failed", description: commandErrorMessage(e) });
          }}
        />
      )}
    </div>
  );
}

type SummaryCardProps = {
  label: string;
  value: number;
  money?: boolean;
  accent?: "gold" | "rose" | "green" | "forest" | "neutral";
  span?: boolean;
};

function SummaryCard({ label, value, money, accent = "neutral", span }: SummaryCardProps) {
  return (
    <div
      className={cn(
        "rounded-lg border border-neutral-200 bg-white p-3 shadow-sm",
        span && "col-span-2",
      )}
    >
      <p className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">{label}</p>
      <p
        className={cn(
          "mt-1 text-xl font-semibold tabular-nums",
          accent === "gold" && "text-amber-600",
          accent === "rose" && "text-rose-600",
          accent === "green" && "text-emerald-600",
          accent === "forest" && "text-forest-700",
          accent === "neutral" && "text-neutral-800",
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
        "flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-medium transition-colors",
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

function BalancesTable({
  rows,
  loading,
  onViewHistory,
}: {
  rows: StockBalanceDto[];
  loading: boolean;
  onViewHistory: (productId: number) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) {
    return <EmptyRow message="No stock has been posted yet. Use Opening stock to begin." />;
  }
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Product</TableHead>
              <TableHead>Location</TableHead>
              <TableHead className="text-right">On hand</TableHead>
              <TableHead className="text-right">Reserved</TableHead>
              <TableHead className="text-right">Damaged</TableHead>
              <TableHead className="text-right">Available</TableHead>
              <TableHead className="text-right">Minimum</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((b) => (
              <TableRow key={`${b.productId}-${b.locationId}`}>
                <TableCell>
                  <div className="flex items-center gap-3">
                    <StoredImage path={b.thumbnailPath} className="h-10 w-10 rounded-md object-cover" />
                    <div className="grid gap-0.5">
                      <button
                        type="button"
                        onClick={() => onViewHistory(b.productId)}
                        className="text-left font-medium text-neutral-900 hover:text-forest-700 hover:underline"
                      >
                        {b.productName}
                      </button>
                      <span className="text-[11px] text-neutral-500">{b.articleNumber}</span>
                    </div>
                  </div>
                </TableCell>
                <TableCell>{b.locationName}</TableCell>
                <TableCell className="text-right tabular-nums">{b.onHand}</TableCell>
                <TableCell className="text-right tabular-nums">{b.reserved}</TableCell>
                <TableCell className="text-right tabular-nums text-rose-600">{b.damaged}</TableCell>
                <TableCell
                  className={cn(
                    "text-right font-semibold tabular-nums",
                    b.available < 0 ? "text-rose-600" : b.available < b.minimumStock ? "text-amber-600" : "text-forest-700",
                  )}
                >
                  {b.available}
                </TableCell>
                <TableCell className="text-right tabular-nums text-neutral-500">
                  {b.minimumStock}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function LedgerTable({
  rows,
  loading,
  session,
  canMutate,
  onReverse,
  onViewHistory,
}: {
  rows: StockMovementDto[];
  loading: boolean;
  session: string;
  canMutate: boolean;
  onReverse: () => void;
  onViewHistory: (productId: number) => void;
}) {
  const queryClient = useQueryClient();
  const [reversing, setReversing] = React.useState(false);
  const { toast } = useToast();

  const reverseMutation = useMutation({
    mutationFn: (movementId: number) =>
      stockReverse(session, { movementId, reason: "manual reversal" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["inventory"] });
      setReversing(false);
      onReverse();
      toast({ variant: "success", title: "Movement reversed" });
    },
    onError: (e: Error) => {
      setReversing(false);
      toast({ variant: "error", title: "Reversal failed", description: e.message });
    },
  });

  if (loading) return <LoadingRow />;
  if (rows.length === 0) {
    return <EmptyRow message="No stock movements recorded yet." />;
  }
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Date</TableHead>
              <TableHead>Reference</TableHead>
              <TableHead>Type</TableHead>
              <TableHead>Product</TableHead>
              <TableHead>Location</TableHead>
              <TableHead className="text-right">Qty</TableHead>
              <TableHead>Reason</TableHead>
              {canMutate && <TableHead className="w-20" />}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((m) => {
              const positive = m.quantityDelta >= 0;
              const isReversed = rows.some((r) => r.reversalOfId === m.id);
              return (
                <TableRow key={m.id}>
                  <TableCell className="whitespace-nowrap text-neutral-500">
                    {formatDateTime(m.createdAt)}
                  </TableCell>
                  <TableCell className="font-medium text-neutral-700">
                    {m.moveNumber ?? `#${m.id}`}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5">
                      <Badge variant={positive ? "success" : "warning"}>
                        {MOVEMENT_LABELS[m.movementType] ?? m.movementType}
                      </Badge>
                      {isReversed && <Badge variant="neutral" className="text-[10px]">Reversed</Badge>}
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="grid gap-0.5">
                      <button
                        type="button"
                        onClick={() => onViewHistory(m.productId)}
                        className="text-left font-medium text-neutral-900 hover:text-forest-700 hover:underline"
                      >
                        {m.productName}
                      </button>
                      <span className="text-[11px] text-neutral-500">{m.articleNumber}</span>
                    </div>
                  </TableCell>
                  <TableCell>{m.locationName}</TableCell>
                  <TableCell
                    className={cn(
                      "text-right font-semibold tabular-nums",
                      positive ? "text-forest-700" : "text-rose-600",
                    )}
                  >
                    {positive ? "+" : ""}
                    {m.quantityDelta}
                  </TableCell>
                  <TableCell className="max-w-52 truncate text-neutral-500">{m.reason}</TableCell>
                  {canMutate && (
                    <TableCell>
                      {!isReversed && !m.reversalOfId && m.movementType !== "cancellation_reversal" && (
                        <Button
                          variant="ghost"
                          size="icon"
                          title="Reverse this movement"
                          disabled={reversing}
                          onClick={() => {
                            setReversing(true);
                            reverseMutation.mutate(m.id);
                          }}
                        >
                          <Undo2 className="h-3.5 w-3.5 text-neutral-500" />
                        </Button>
                      )}
                    </TableCell>
                  )}
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function ValuationTable({ rows, loading }: { rows: ValuationLineDto[]; loading: boolean }) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="Nothing is being valued yet (no sellable stock)." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Product</TableHead>
              <TableHead className="text-right">Sellable qty</TableHead>
              <TableHead className="text-right">Unit cost (oldest FIFO)</TableHead>
              <TableHead className="text-right">Current value</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((v) => (
              <TableRow key={v.productId}>
                <TableCell>
                  <div className="grid gap-0.5">
                    <span className="font-medium text-neutral-900">{v.productName}</span>
                    <span className="text-[11px] text-neutral-500">{v.articleNumber}</span>
                  </div>
                </TableCell>
                <TableCell className="text-right tabular-nums">{v.sellableQty}</TableCell>
                <TableCell className="text-right tabular-nums text-neutral-600">
                  {formatPkr(v.unitCostMinor)}
                </TableCell>
                <TableCell className="text-right font-semibold tabular-nums text-forest-700">
                  {formatPkr(v.valueMinor)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function LowStockTable({ rows, loading }: { rows: LowStockItemDto[]; loading: boolean }) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No products are below their minimum stock level." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Product</TableHead>
              <TableHead className="text-right">Minimum</TableHead>
              <TableHead className="text-right">Available</TableHead>
              <TableHead className="text-right">On hand</TableHead>
              <TableHead className="text-right">Short by</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((item) => (
              <TableRow key={item.productId}>
                <TableCell>
                  <div className="flex items-center gap-3">
                    <StoredImage path={item.thumbnailPath} className="h-10 w-10 rounded-md object-cover" />
                    <div className="grid gap-0.5">
                      <span className="font-medium text-neutral-900">{item.productName}</span>
                      <span className="text-[11px] text-neutral-500">{item.articleNumber}</span>
                    </div>
                  </div>
                </TableCell>
                <TableCell className="text-right tabular-nums">{item.minimumStock}</TableCell>
                <TableCell className="text-right tabular-nums text-rose-600 font-semibold">{item.totalAvailable}</TableCell>
                <TableCell className="text-right tabular-nums">{item.totalOnHand}</TableCell>
                <TableCell className="text-right tabular-nums font-semibold text-rose-600">
                  {item.minimumStock - item.totalAvailable}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function CountSessionsView({
  sessions,
  loading,
  session: sessionToken,
  canMutate,
}: {
  sessions: CountSessionDto[];
  loading: boolean;
  session: string;
  canMutate: boolean;
}) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [selectedSession, setSelectedSession] = React.useState<number | null>(null);

  const postMutation = useMutation({
    mutationFn: (sessionId: number) => stockCountPost(sessionToken, { sessionId }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["inventory"] });
      toast({ variant: "success", title: "Count corrections posted" });
    },
    onError: (e: Error) => {
      toast({ variant: "error", title: "Post count failed", description: e.message });
    },
  });

if (selectedSession) {
    const current = sessions.find((s) => s.id === selectedSession);
    return (
      <CountLinesPanel
        session={current ?? null}
        sessionId={selectedSession}
        sessionToken={sessionToken}
        back={() => setSelectedSession(null)}
        onDone={() => {
          void queryClient.invalidateQueries({ queryKey: ["inventory"] });
          setSelectedSession(null);
          toast({ variant: "success", title: "Count corrections posted" });
        }}
      />
    );
  }

  if (loading) return <LoadingRow />;
  if (sessions.length === 0) {
    return (
      <EmptyRow message="No count sessions found. Start a new physical count to compare expected vs. actual stock." />
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Session</TableHead>
              <TableHead>Location</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Created</TableHead>
              <TableHead>Posted</TableHead>
              <TableHead className="w-32">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sessions.map((s) => (
              <TableRow key={s.id}>
                <TableCell className="font-medium text-neutral-700">
                  {s.sessionNumber ?? `#${s.id}`}
                </TableCell>
                <TableCell>{s.locationName}</TableCell>
                <TableCell>
                  <Badge variant={s.status === "posted" ? "success" : s.status === "open" ? "warning" : "neutral"}>
                    {s.status}
                  </Badge>
                </TableCell>
                <TableCell className="text-neutral-500">{formatDateTime(s.createdAt)}</TableCell>
                <TableCell className="text-neutral-500">
                  {s.postedAt ? formatDateTime(s.postedAt) : "—"}
                </TableCell>
                <TableCell>
                  <div className="flex items-center gap-1.5">
                    <Button variant="outline" size="sm" onClick={() => setSelectedSession(s.id)}>
                      {s.status === "open" ? "Enter counts" : "View"}
                    </Button>
                    {canMutate && s.status === "open" && (
                      <Button
                        variant="primary"
                        size="sm"
                        disabled={postMutation.isPending}
                        onClick={() => postMutation.mutate(s.id)}
                      >
                        Post
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

function CountLinesPanel({
  session,
  sessionId,
  sessionToken,
  onDone,
  back,
}: {
  session: CountSessionDto | null;
  sessionId: number;
  sessionToken: string;
  onDone: () => void;
  back: () => void;
}) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [counts, setCounts] = React.useState<Record<number, number>>({});

  const linesQuery = useQuery({
    queryKey: ["inventory", "count-lines", sessionId],
    queryFn: () => stockCountLines(sessionToken, sessionId),
    enabled: !!sessionToken,
  });
  const lines = linesQuery.data ?? [];

  const mutation = useMutation({
    mutationFn: ({ productId, countedQty }: { productId: number; countedQty: number }) =>
      stockCountLineUpdate(sessionToken, { sessionId, productId, countedQty }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["inventory", "count-lines", sessionId] });
    },
    onError: (e: Error) => {
      toast({ variant: "error", title: "Failed to save count", description: e.message });
    },
  });

  const postMutation = useMutation({
    mutationFn: () => stockCountPost(sessionToken, { sessionId }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["inventory"] });
      toast({ variant: "success", title: "Count corrections posted" });
      onDone();
    },
    onError: (e: Error) => {
      toast({ variant: "error", title: "Post count failed", description: e.message });
    },
  });

  if (linesQuery.isLoading) return <LoadingRow />;

  const isOpen = session?.status === "open";

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-base font-semibold text-neutral-900">
            {session?.sessionNumber ?? `Session #${sessionId}`}
          </h3>
          <p className="text-sm text-neutral-500">
            {session?.locationName ?? ""} · {isOpen ? "counting in progress" : session?.status}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={back}>
            Back to sessions
          </Button>
          {isOpen && (
            <Button variant="primary" size="sm" disabled={postMutation.isPending} onClick={() => postMutation.mutate()}>
              Post corrections
            </Button>
          )}
        </div>
      </div>

      {lines.length === 0 ? (
        <EmptyRow message="No products have stock at this location to count." />
      ) : (
        <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Product</TableHead>
                  <TableHead className="text-right">Expected</TableHead>
                  {isOpen && <TableHead className="text-right">Counted</TableHead>}
                  <TableHead className="text-right">Variance</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {lines.map((line) => {
                  const counted = counts[line.productId] ?? line.countedQty;
                  return (
                    <TableRow key={line.id}>
                      <TableCell>
                        <div className="grid gap-0.5">
                          <span className="font-medium text-neutral-900">{line.productName}</span>
                          <span className="text-[11px] text-neutral-500">{line.articleNumber}</span>
                        </div>
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{line.expectedQty}</TableCell>
                      {isOpen && (
                        <TableCell className="text-right">
                          <Input
                            type="number"
                            min={0}
                            className="ml-auto h-8 w-20 text-right tabular-nums"
                            value={String(counted)}
                            onChange={(e) => {
                              const v = Number(e.target.value);
                              setCounts((prev) => ({ ...prev, [line.productId]: v }));
                              if (Number.isFinite(v) && v >= 0) {
                                mutation.mutate({ productId: line.productId, countedQty: v });
                              }
                            }}
                          />
                        </TableCell>
                      )}
                      <TableCell
                        className={cn(
                          "text-right font-semibold tabular-nums",
                          (counted - line.expectedQty) < 0 ? "text-rose-600" : (counted - line.expectedQty) > 0 ? "text-forest-700" : "text-neutral-500",
                        )}
                      >
                        {counted - line.expectedQty > 0 ? "+" : ""}
                        {counted - line.expectedQty}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </div>
      )}
    </div>
  );
}

function CountSessionDialog({
  session,
  locationId,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  locationId: number | null;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [notes, setNotes] = React.useState("");
  const mutation = useMutation({
    mutationFn: () =>
      stockCountStart(session, { locationId: locationId!, notes: notes || undefined }),
    onSuccess: () => onDone(),
    onError,
  });

  return (
    <StockDialog
      title="Start stock count"
      description="Create a new physical count session. Expected quantities will be pre-populated from current stock balances."
      onSubmit={() => {
        if (locationId) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel="Start count"
      onClose={onClose}
    >
      <div className="grid gap-4">
        <div>
          <Label>Notes (optional)</Label>
          <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="e.g. weekly count" />
        </div>
      </div>
    </StockDialog>
  );
}

function OpeningImportDialog({
  session,
  locations,
  onClose,
  onDone,
  onError,
}: Omit<DialogProps, "onDone"> & { onDone: (result: OpeningBatchResultDto) => void }) {
  const [raw, setRaw] = React.useState("");
  const locationId = locations[0]?.id ?? null;
  const [result, setResult] = React.useState<OpeningBatchResultDto | null>(null);

  const parsed = React.useMemo(() => {
    return raw
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line, i) => {
        const parts = line.split(",").map((p) => p.trim());
        return {
          rowIndex: i,
          articleNumber: parts[0] ?? "",
          locationId,
          quantity: parts[1] === undefined ? 0 : Number(parts[1]),
          unitCostMinor: parts[2] === undefined || parts[2] === "" ? undefined : Number(parts[2]),
        };
      });
  }, [raw, locationId]);

  const validationErrors = React.useMemo(() => {
    const errors: { rowIndex: number; message: string }[] = [];
    parsed.forEach((r) => {
      if (!r.articleNumber) errors.push({ rowIndex: r.rowIndex, message: "missing article number" });
      else if (!/^[A-Za-z0-9 ._-]+$/.test(r.articleNumber))
        errors.push({ rowIndex: r.rowIndex, message: "invalid article number" });
      if (!Number.isFinite(r.quantity) || r.quantity <= 0)
        errors.push({ rowIndex: r.rowIndex, message: "quantity must be a positive number" });
      if (r.unitCostMinor !== undefined && (!Number.isFinite(r.unitCostMinor) || r.unitCostMinor < 0))
        errors.push({ rowIndex: r.rowIndex, message: "unit cost must be zero or positive" });
    });
    return errors;
  }, [parsed, locationId]);

  const payload = React.useMemo<OpeningBatchInput | null>(() => {
    if (validationErrors.length > 0 || parsed.length === 0) return null;
    return {
      rows: parsed.map((r) => ({
        articleNumber: r.articleNumber,
        locationId: locationId!,
        quantity: r.quantity,
        unitCostMinor: r.unitCostMinor,
      })),
    };
  }, [parsed, validationErrors, locationId]);

  const submitMutation = useMutation({
    mutationFn: () => stockOpeningBatch(session, payload!),
    onSuccess: (res) => {
      setResult(res);
      if (res.errors.length === 0) onDone(res);
    },
    onError,
  });

  return (
    <StockDialog
      title="Import opening stock"
      description="One row per line: ArticleNumber, Quantity, UnitCostMinor (optional). All rows use the showroom."
      onSubmit={() => submitMutation.mutate()}
      busy={submitMutation.isPending}
      submitLabel="Post rows"
      onClose={onClose}
      submitDisabled={!payload || submitMutation.isPending}
    >
      <div className="grid gap-4">
        <div className="grid gap-1.5">
          <Label>Rows</Label>
          <textarea
            value={raw}
            onChange={(e) => setRaw(e.target.value)}
            placeholder={"CHAIR-001,5,2500\nSOFA-200,1,18000"}
            rows={6}
            className="w-full rounded-md border border-neutral-300 bg-white px-3 py-2 font-mono text-xs text-neutral-900 shadow-sm focus:outline-none focus:ring-2 focus:ring-forest-500"
          />
          <p className="text-xs text-neutral-500">
            Columns: articleNumber, quantity, unitCostMinor. Only existing active products are matched.
          </p>
        </div>
        {validationErrors.length > 0 && (
          <div className="rounded-md border border-rose-200 bg-rose-50 p-3 text-xs">
            <p className="font-medium text-rose-700">Fix before posting</p>
            <ul className="mt-1 list-inside list-disc space-y-0.5 text-rose-600">
              {validationErrors.slice(0, 6).map((v) => (
                <li key={v.rowIndex}>
                  {v.rowIndex === -1 ? "—" : `Row ${v.rowIndex + 1}`}: {v.message}
                </li>
              ))}
              {validationErrors.length > 6 && <li>…and {validationErrors.length - 6} more</li>}
            </ul>
          </div>
        )}
        {payload && payload.rows.length > 0 && (
          <div className="rounded-md border border-neutral-200 bg-neutral-50 p-3 text-xs">
            <p className="font-medium text-neutral-700">
              {payload.rows.length} row{payload.rows.length === 1 ? "" : "s"} ready to post
            </p>
            <ul className="mt-1 max-h-32 list-inside list-disc space-y-0.5 text-neutral-600">
              {payload.rows.slice(0, 8).map((r, i) => (
                <li key={i}>
                  {r.articleNumber} × {r.quantity} @ {r.unitCostMinor == null ? "—" : `${(r.unitCostMinor / 100).toFixed(2)}`}
                </li>
              ))}
              {payload.rows.length > 8 && <li>…and {payload.rows.length - 8} more</li>}
            </ul>
          </div>
        )}
        {result && result.errors.length > 0 && (
          <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-xs">
            <p className="font-medium text-amber-700">
              {result.postedCount} posted, {result.errors.length} failed
            </p>
            <ul className="mt-1 list-inside list-disc space-y-0.5 text-amber-600">
              {result.errors.slice(0, 6).map((e, i) => (
                <li key={e.rowIndex + i}>
                  Row {e.rowIndex + 1} ({e.articleNumber}): {e.error}
                </li>
              ))}
              {result.errors.length > 6 && <li>…and {result.errors.length - 6} more</li>}
            </ul>
          </div>
        )}
      </div>
    </StockDialog>
  );
}

function LoadingRow() {
  return (
    <div className="flex items-center justify-center gap-2 rounded-lg border border-neutral-200 bg-white p-10 text-neutral-500">
      <Loader2 className="h-4 w-4 animate-spin" />
      Loadingâ€¦
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

type DialogProps = {
  session: string;
  locations: LocationDto[];
  onClose: () => void;
  onDone: () => void;
  onError: (e: unknown) => void;
};

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
            placeholder="Search productâ€¦"
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
                    <span className="block truncate text-sm font-medium text-neutral-900">
                      {p.name}
                    </span>
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

function OpeningDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const locationId = locations[0]?.id ?? null;
  const [qty, setQty] = React.useState("1");
  const [cost, setCost] = React.useState(0);
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      stockOpening(session, {
        productId: picker.selected!.id,
        locationId: locationId!,
        quantity: Number(qty),
        unitCostMinor: cost,
        reason: reason || null,
      }),
    onSuccess: onDone,
    onError,
  });

  return (
    <StockDialog
      title="Post opening stock"
      description="Set the starting showroom balance for a product. A positive unit cost seeds the FIFO cost layer."
      onSubmit={() => {
        if (picker.selected && locationId && Number(qty) > 0) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel="Post opening"
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <div className="grid gap-1.5">
        <Label>Quantity</Label>
        <Input
          inputMode="numeric"
          value={qty}
          onChange={(e) => setQty(e.target.value.replace(/[^0-9]/g, ""))}
          placeholder="10"
        />
      </div>
      <div className="grid gap-1.5">
        <Label>Unit cost (optional)</Label>
        <MoneyInput value={cost} onCommit={setCost} placeholder="0.00" />
      </div>
      <div className="grid gap-1.5">
        <Label>Reason (optional)</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Initial stock" />
      </div>
    </StockDialog>
  );
}

function AdjustDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const locationId = locations[0]?.id ?? null;
  const [qty, setQty] = React.useState("");
  const [cost, setCost] = React.useState(0);
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      stockAdjust(session, {
        productId: picker.selected!.id,
        locationId: locationId!,
        adjustmentQty: Number(qty),
        unitCostMinor: cost,
        reason: reason || null,
      }),
    onSuccess: onDone,
    onError,
  });

  const qtyNum = Number(qty);
  const validQty = Number.isFinite(qtyNum) && qtyNum !== 0;

  return (
    <StockDialog
      title="Adjust stock"
      description="Manual count correction. Use a positive number to add stock, a negative number to remove it. Below-zero stock is blocked unless the shop setting allows it."
      onSubmit={() => {
        if (picker.selected && locationId && validQty) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel="Post adjustment"
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <div className="grid gap-1.5">
        <Label>Adjustment (+add / âˆ’remove)</Label>
        <Input
          inputMode="numeric"
          value={qty}
          onChange={(e) => setQty(e.target.value.replace(/[^0-9-]/g, ""))}
          placeholder="+3 or -2"
        />
      </div>
      {Number(qty) > 0 && (
        <div className="grid gap-1.5">
          <Label>Unit cost for added stock (optional)</Label>
          <MoneyInput value={cost} onCommit={setCost} placeholder="0.00" />
        </div>
      )}
      <div className="grid gap-1.5">
        <Label>Reason (optional)</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Count correction" />
      </div>
    </StockDialog>
  );
}

function DamageDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const locationId = locations[0]?.id ?? null;
  const [mode, setMode] = React.useState<"damage" | "repair">("damage");
  const [qty, setQty] = React.useState("");
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () => {
      const input = {
        productId: picker.selected!.id,
        locationId: locationId!,
        quantity: Number(qty),
        reason: reason || null,
      };
      return mode === "damage" ? stockDamage(session, input) : stockRepair(session, input);
    },
    onSuccess: onDone,
    onError,
  });

  return (
    <StockDialog
      title="Classify damage / repair"
      description="Move sellable units into the damaged count, or recover repaired units back to sellable."
      onSubmit={() => {
        if (picker.selected && locationId && Number(qty) > 0) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel={mode === "damage" ? "Mark as damaged" : "Recover from damaged"}
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <div className="grid gap-1.5">
        <Label>Action</Label>
        <div className="flex gap-2">
          <Button type="button" variant={mode === "damage" ? "primary" : "outline"} size="sm" onClick={() => setMode("damage")}>
            Damage
          </Button>
          <Button type="button" variant={mode === "repair" ? "primary" : "outline"} size="sm" onClick={() => setMode("repair")}>
            Repair recovery
          </Button>
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Quantity</Label>
        <Input inputMode="numeric" value={qty} onChange={(e) => setQty(e.target.value.replace(/[^0-9]/g, ""))} placeholder="1" />
      </div>
      <div className="grid gap-1.5">
        <Label>Reason (optional)</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Damaged in transit" />
      </div>
    </StockDialog>
  );
}

function ReserveDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const locationId = locations[0]?.id ?? null;
  const [mode, setMode] = React.useState<"reserve" | "release">("reserve");
  const [qty, setQty] = React.useState("");
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () => {
      const input = {
        productId: picker.selected!.id,
        locationId: locationId!,
        quantity: Number(qty),
        reason: reason || null,
      };
      return mode === "reserve" ? stockReserve(session, input) : stockRelease(session, input);
    },
    onSuccess: onDone,
    onError,
  });

  return (
    <StockDialog
      title="Reserve / release stock"
      description="Hold sellable units for a customer order, or release a previous reservation back to available."
      onSubmit={() => {
        if (picker.selected && locationId && Number(qty) > 0) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel={mode === "reserve" ? "Reserve" : "Release"}
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <div className="grid gap-1.5">
        <Label>Action</Label>
        <div className="flex gap-2">
          <Button type="button" variant={mode === "reserve" ? "primary" : "outline"} size="sm" onClick={() => setMode("reserve")}>
            Reserve
          </Button>
          <Button type="button" variant={mode === "release" ? "primary" : "outline"} size="sm" onClick={() => setMode("release")}>
            Release
          </Button>
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Quantity</Label>
        <Input inputMode="numeric" value={qty} onChange={(e) => setQty(e.target.value.replace(/[^0-9]/g, ""))} placeholder="1" />
      </div>
      <div className="grid gap-1.5">
        <Label>Reason (optional)</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Customer hold" />
      </div>
    </StockDialog>
  );
}

function StockDialog({
  title,
  description,
  children,
  onSubmit,
  busy,
  submitLabel,
  onClose,
  submitDisabled,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
  onSubmit: () => void;
  busy: boolean;
  submitLabel: string;
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
            onSubmit();
          }}
        >
          {children}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || submitDisabled}>
              {busy && <Loader2 className="mr-1 h-4 w-4 animate-spin" />}
              {submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
