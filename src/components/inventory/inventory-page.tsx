"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeftRight,
  Boxes,
  ClipboardList,
  Loader2,
  PackageOpen,
  Plus,
  Search,
  Scale,
  Wrench,
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
  locationList,
  productList,
  stockAdjust,
  stockBalanceList,
  stockDamage,
  stockMovementList,
  stockOpening,
  stockRelease,
  stockRepair,
  stockReserve,
  stockTransfer,
  stockValuation,
  type LocationDto,
  type ProductListItemDto,
  type StockBalanceDto,
  type StockMovementDto,
  type ValuationLineDto,
} from "@/lib/tauri/api";
import { formatDateTime, formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";

type Tab = "stock" | "ledger" | "valuation";

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

  const [view, setView] = React.useState<Tab>("stock");
  const [locationId, setLocationId] = React.useState<number | null>(null);
  const [dialog, setDialog] = React.useState<
    null | "opening" | "transfer" | "adjust" | "damage" | "reserve"
  >(null);

  const locationsQuery = useQuery({
    queryKey: ["inventory", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session,
  });

  const balancesQuery = useQuery({
    queryKey: ["inventory", "balances", locationId],
    queryFn: () => stockBalanceList(session, locationId),
    enabled: !!session,
  });

  const movementsQuery = useQuery({
    queryKey: ["inventory", "movements", locationId],
    queryFn: () =>
      stockMovementList(session, { locationId, limit: 200 }),
    enabled: !!session,
  });

  const valuationQuery = useQuery({
    queryKey: ["inventory", "valuation"],
    queryFn: () => stockValuation(session),
    enabled: !!session && canViewValuation,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "products"] });
  };

  const locations = locationsQuery.data ?? [];
  const balances = balancesQuery.data ?? [];
  const movements = movementsQuery.data ?? [];
  const valuation = valuationQuery.data ?? [];

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
            <Select
              value={locationId ? String(locationId) : "all"}
              onValueChange={(v) => setLocationId(v === "all" ? null : Number(v))}
            >
              <SelectTrigger className="w-40">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All locations</SelectItem>
                {locations.map((l) => (
                  <SelectItem key={l.id} value={String(l.id)}>
                    {l.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {canMutate && (
              <>
                <Button variant="outline" onClick={() => setDialog("transfer")}>
                  <ArrowLeftRight className="h-4 w-4" />
                  Transfer
                </Button>
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
        {canViewValuation && (
          <TabButton active={view === "valuation"} onClick={() => setView("valuation")} icon={<PackageOpen className="h-4 w-4" />}>
            Valuation (FIFO)
          </TabButton>
        )}
      </div>

      <div className="mt-4">
        {view === "stock" && (
          <BalancesTable rows={balances} loading={balancesQuery.isLoading} />
        )}
        {view === "ledger" && <LedgerTable rows={movements} loading={movementsQuery.isLoading} />}
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
      {dialog === "transfer" && canMutate && (
        <TransferDialog
          session={session}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            toast({ variant: "success", title: "Stock transferred" });
          }}
          onError={(e) => {
            if (isSessionError(e)) {
              refresh();
              return;
            }
            toast({ variant: "error", title: "Transfer failed", description: commandErrorMessage(e) });
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
}: {
  rows: StockBalanceDto[];
  loading: boolean;
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
                  <div className="grid gap-0.5">
                    <span className="font-medium text-neutral-900">{b.productName}</span>
                    <span className="text-[11px] text-neutral-500">{b.articleNumber}</span>
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

function LedgerTable({ rows, loading }: { rows: StockMovementDto[]; loading: boolean }) {
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
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((m) => {
              const positive = m.quantityDelta >= 0;
              return (
                <TableRow key={m.id}>
                  <TableCell className="whitespace-nowrap text-neutral-500">
                    {formatDateTime(m.createdAt)}
                  </TableCell>
                  <TableCell className="font-medium text-neutral-700">
                    {m.moveNumber ?? `#${m.id}`}
                  </TableCell>
                  <TableCell>
                    <Badge variant={positive ? "success" : "warning"}>
                      {MOVEMENT_LABELS[m.movementType] ?? m.movementType}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <div className="grid gap-0.5">
                      <span className="font-medium text-neutral-900">{m.productName}</span>
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
  const [locationId, setLocationId] = React.useState<number | null>(null);
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
      description="Set the starting balance for a product at a location. A positive unit cost seeds the FIFO cost layer."
      onSubmit={() => {
        if (picker.selected && locationId && Number(qty) > 0) mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel="Post opening"
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
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

function TransferDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const [fromId, setFromId] = React.useState<number | null>(null);
  const [toId, setToId] = React.useState<number | null>(null);
  const [qty, setQty] = React.useState("");
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      stockTransfer(session, {
        productId: picker.selected!.id,
        fromLocationId: fromId!,
        toLocationId: toId!,
        quantity: Number(qty),
        reason: reason || null,
      }),
    onSuccess: onDone,
    onError,
  });

  return (
    <StockDialog
      title="Transfer stock"
      description="Move sellable stock from one location to another. Both an out and an in movement are posted."
      onSubmit={() => {
        if (picker.selected && fromId && toId && fromId !== toId && Number(qty) > 0)
          mutation.mutate();
      }}
      busy={mutation.isPending}
      submitLabel="Transfer"
      onClose={onClose}
    >
      <ProductPicker picker={picker} label="Product" />
      <div className="grid grid-cols-2 gap-3">
        <LocationSelect
          locations={locations.filter((l) => l.id !== toId)}
          value={fromId}
          onChange={setFromId}
          label="From"
        />
        <LocationSelect
          locations={locations.filter((l) => l.id !== fromId)}
          value={toId}
          onChange={setToId}
          label="To"
        />
      </div>
      <div className="grid gap-1.5">
        <Label>Quantity</Label>
        <Input inputMode="numeric" value={qty} onChange={(e) => setQty(e.target.value.replace(/[^0-9]/g, ""))} placeholder="3" />
      </div>
      <div className="grid gap-1.5">
        <Label>Reason (optional)</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Replenish showroom" />
      </div>
    </StockDialog>
  );
}

function AdjustDialog({ session, locations, onClose, onDone, onError }: DialogProps) {
  const picker = useProductPicker(session);
  const [locationId, setLocationId] = React.useState<number | null>(null);
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
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
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
  const [locationId, setLocationId] = React.useState<number | null>(null);
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
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
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
  const [locationId, setLocationId] = React.useState<number | null>(null);
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
      <LocationSelect locations={locations} value={locationId} onChange={setLocationId} />
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

function LocationSelect({
  locations,
  value,
  onChange,
  label = "Location",
}: {
  locations: LocationDto[];
  value: number | null;
  onChange: (id: number | null) => void;
  label?: string;
}) {
  return (
    <div className="grid gap-1.5">
      <Label>{label}</Label>
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

function StockDialog({
  title,
  description,
  children,
  onSubmit,
  busy,
  submitLabel,
  onClose,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
  onSubmit: () => void;
  busy: boolean;
  submitLabel: string;
  onClose: () => void;
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
            <Button type="submit" disabled={busy}>
              {busy && <Loader2 className="mr-1 h-4 w-4 animate-spin" />}
              {submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
