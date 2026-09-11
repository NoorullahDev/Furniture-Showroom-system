"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  ClipboardList,
  Loader2,
  PackagePlus,
  Printer,
  RefreshCw,
  Truck,
  Undo2,
  Wrench,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { takeDashboardTarget } from "@/lib/dashboard-navigation";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MoneyInput } from "@/components/ui/money-input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { PageHeader } from "@/components/page-header";
import { useToast } from "@/components/ui/toast";
import { useSession, isSessionError } from "@/components/session/session-provider";
import { formatDateTime, formatPkr } from "@/lib/format";
import {
  cashAccountList,
  CreditNoteDto,
  creditNoteList,
  creditNotePdf,
  CashAccountDto,
  damageDecide,
  damageList,
  damageRecord,
  DamageRecordDto,
  deliveryCreate,
  deliveryGet,
  deliveryList,
  deliveryNotePdf,
  deliveryTransition,
  DeliveryDto,
  locationList,
  LocationDto,
  openFile,
  productList,
  ProductListItemDto,
  saleReturnList,
  saleReturnPost,
  saleReturnVoid,
  SaleDto,
  SaleReturnDto,
  saleList,
} from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";

type Tab = "deliveries" | "returns" | "damage";

type PageDialog =
  | null
  | { kind: "create-delivery" }
  | { kind: "delivery-detail"; delivery: DeliveryDto }
  | { kind: "transition"; delivery: DeliveryDto }
  | { kind: "post-return" }
  | { kind: "void-return"; ret: SaleReturnDto }
  | { kind: "record-damage" }
  | { kind: "decide-damage"; damage: DamageRecordDto };

function today(): string {
  const d = new Date();
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function StatusBadge({ status }: { status: string }) {
  const variant =
    status === "pending" || status === "open"
      ? "warning"
      : status === "ready"
        ? "info"
        : status === "dispatched" || status === "posted"
          ? "info"
          : status === "delivered" || status === "resolved" || status === "applied"
            ? "success"
            : status === "failed" || status === "voided" || status === "cancelled"
              ? "danger"
              : "neutral";
  return <Badge variant={variant}>{status}</Badge>;
}

function deliveryActionsFor(status: string): string[] {
  switch (status) {
    case "pending":
      return ["ready", "failed", "cancelled"];
    case "ready":
      return ["dispatched", "failed", "cancelled"];
    case "dispatched":
      return ["delivered", "failed"];
    case "failed":
      return ["cancelled"];
    default:
      return [];
  }
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

function PrintButton({
  onClick,
  label,
  className,
}: {
  onClick: () => void;
  label: string;
  className?: string;
}) {
  const { toast } = useToast();
  const [busy, setBusy] = React.useState(false);
  const run = async () => {
    setBusy(true);
    try {
      await onClick();
    } catch (e) {
      toast({ variant: "error", title: "Could not generate PDF", description: commandErrorMessage(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Button type="button" variant="outline" size="sm" onClick={() => void run()} disabled={busy} className={className}>
      {busy ? <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" /> : <Printer className="mr-1 h-3.5 w-3.5" />}
      {label}
    </Button>
  );
}

function DeliveryPrint({ session, deliveryId }: { session: string; deliveryId: number }) {
  return (
    <PrintButton
      label="Delivery note"
      onClick={async () => {
        const pdf = await deliveryNotePdf(session, deliveryId);
        await openFile(session, pdf.reportPath);
      }}
    />
  );
}

function CreditNotePrint({ session, creditId }: { session: string; creditId: number }) {
  return (
    <PrintButton
      label="Credit note"
      onClick={async () => {
        const pdf = await creditNotePdf(session, creditId);
        await openFile(session, pdf.reportPath);
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// Deliveries
// ---------------------------------------------------------------------------

function DeliveriesView({
  session,
  deliveries,
  loading,
  canCreate,
  canUpdate,
  onTransition,
  onDetail,
}: {
  session: string;
  deliveries: DeliveryDto[];
  loading: boolean;
  canCreate: boolean;
  canUpdate: boolean;
  onTransition: (d: DeliveryDto) => void;
  onDetail: (d: DeliveryDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (deliveries.length === 0)
    return (
      <EmptyRow
        message={
          canCreate
            ? "No deliveries yet. Create one from a confirmed sale."
            : "No deliveries available."
        }
      />
    );
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-neutral-200 bg-neutral-50 text-left text-[11px] font-semibold uppercase tracking-wide text-neutral-500">
              <th className="px-4 py-2">Number</th>
              <th className="px-4 py-2">Sale</th>
              <th className="px-4 py-2">Items</th>
              <th className="px-4 py-2">Charge</th>
              <th className="px-4 py-2">Status</th>
              <th className="px-4 py-2">Scheduled</th>
              <th className="px-4 py-2 text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {deliveries.map((d) => (
              <tr key={d.id} className="border-b border-neutral-100 last:border-0">
                <td className="px-4 py-2 font-medium">{d.deliveryNumber ?? `#${d.id}`}</td>
                <td className="px-4 py-2">
                  <span className="text-neutral-700">{d.saleNumber ?? `#${d.saleId}`}</span>
                  {d.customerName && (
                    <span className="block text-[11px] text-neutral-500">{d.customerName}</span>
                  )}
                </td>
                <td className="px-4 py-2">
                  {d.items.length > 0 ? (
                    <div className="grid gap-0.5">
                      {d.items.map((i) => (
                        <span key={i.id} className="text-xs text-neutral-600">
                          {i.quantity} × {i.productName}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <span className="text-neutral-400">—</span>
                  )}
                </td>
                <td className="px-4 py-2 tabular-nums">
                  {d.deliveryChargeMinor > 0 ? formatPkr(d.deliveryChargeMinor) : "—"}
                </td>
                <td className="px-4 py-2">
                  <StatusBadge status={d.status} />
                </td>
                <td className="px-4 py-2 text-xs text-neutral-500">
                  {d.scheduledAt ? d.scheduledAt.slice(0, 10) : "—"}
                  {d.rescheduleCount > 0 && (
                    <span className="ml-1 text-amber-600">×{d.rescheduleCount}</span>
                  )}
                </td>
                <td className="px-4 py-2">
                  <div className="flex items-center justify-end gap-1">
                    <DeliveryPrint session={session} deliveryId={d.id} />
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => onDetail(d)}
                    >
                      View
                    </Button>
                    {canUpdate && deliveryActionsFor(d.status).length > 0 && (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => onTransition(d)}
                      >
                        Update
                      </Button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Returns + credit notes
// ---------------------------------------------------------------------------

function ReturnsView({
  session,
  returns,
  creditNotes,
  loading,
  canUseCredit,
  onVoid,
}: {
  session: string;
  returns: SaleReturnDto[];
  creditNotes: CreditNoteDto[];
  loading: boolean;
  canUseCredit: boolean;
  onVoid: (r: SaleReturnDto) => void;
}) {
  if (loading) return <LoadingRow />;
  return (
    <div className="grid gap-6">
      <section className="grid gap-2">
        <h3 className="text-sm font-semibold text-neutral-700">
          Sales returns <span className="ml-1 text-xs font-normal text-neutral-400">({returns.length})</span>
        </h3>
        {returns.length === 0 && <EmptyRow message="No returns posted yet." />}
        {returns.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-neutral-200 bg-neutral-50 text-left text-[11px] font-semibold uppercase tracking-wide text-neutral-500">
                    <th className="px-4 py-2">Return</th>
                    <th className="px-4 py-2">Sale</th>
                    <th className="px-4 py-2">Method</th>
                    <th className="px-4 py-2 text-right">Refund</th>
                    <th className="px-4 py-2">Status</th>
                    <th className="px-4 py-2 text-right">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {returns.map((r) => (
                    <tr key={r.id} className="border-b border-neutral-100 last:border-0">
                      <td className="px-4 py-2 font-medium">
                        {r.returnNumber ?? `#${r.id}`}
                        <span className="block text-[11px] text-neutral-500">{r.returnDate}</span>
                      </td>
                      <td className="px-4 py-2 text-neutral-700">{r.saleNumber ?? `#${r.saleId}`}</td>
                      <td className="px-4 py-2 capitalize">{r.refundType}</td>
                      <td className="px-4 py-2 text-right tabular-nums">{formatPkr(r.totalRefundMinor)}</td>
                      <td className="px-4 py-2">
                        <StatusBadge status={r.status} />
                      </td>
                      <td className="px-4 py-2">
                        <div className="flex items-center justify-end gap-1">
                          {r.status === "posted" && (
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              onClick={() => onVoid(r)}
                            >
                              <Undo2 className="mr-1 h-3.5 w-3.5" />
                              Void
                            </Button>
                          )}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </section>

      {canUseCredit && (
        <section className="grid gap-2">
          <h3 className="text-sm font-semibold text-neutral-700">
            Credit notes <span className="ml-1 text-xs font-normal text-neutral-400">({creditNotes.length})</span>
          </h3>
          {creditNotes.length === 0 && <EmptyRow message="No credit notes issued." />}
          {creditNotes.length > 0 && (
            <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-neutral-200 bg-neutral-50 text-left text-[11px] font-semibold uppercase tracking-wide text-neutral-500">
                      <th className="px-4 py-2">Number</th>
                      <th className="px-4 py-2 text-right">Amount</th>
                      <th className="px-4 py-2">Status</th>
                      <th className="px-4 py-2">Applied</th>
                      <th className="px-4 py-2 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {creditNotes.map((c) => (
                      <tr key={c.id} className="border-b border-neutral-100 last:border-0">
                        <td className="px-4 py-2 font-medium">{c.creditNumber ?? `#${c.id}`}</td>
                        <td className="px-4 py-2 text-right tabular-nums">{formatPkr(c.amountMinor)}</td>
                        <td className="px-4 py-2">
                          <StatusBadge status={c.status} />
                        </td>
                        <td className="px-4 py-2 text-xs text-neutral-500">
                          {c.appliedAt ? formatDateTime(c.appliedAt) : "—"}
                        </td>
                        <td className="px-4 py-2">
                          <div className="flex items-center justify-end gap-1">
                            <CreditNotePrint session={session} creditId={c.id} />
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </section>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Damage records
// ---------------------------------------------------------------------------

function DamageView({
  records,
  loading,
  onDecide,
}: {
  records: DamageRecordDto[];
  loading: boolean;
  onDecide: (d: DamageRecordDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (records.length === 0) return <EmptyRow message="No damage records yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-neutral-200 bg-neutral-50 text-left text-[11px] font-semibold uppercase tracking-wide text-neutral-500">
              <th className="px-4 py-2">Record</th>
              <th className="px-4 py-2">Product</th>
              <th className="px-4 py-2">Qty</th>
              <th className="px-4 py-2">Source</th>
              <th className="px-4 py-2 text-right">Loss</th>
              <th className="px-4 py-2">Status</th>
              <th className="px-4 py-2">Decision</th>
              <th className="px-4 py-2 text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {records.map((d) => (
              <tr key={d.id} className="border-b border-neutral-100 last:border-0">
                <td className="px-4 py-2 font-medium">
                  {d.damageNumber ?? `#${d.id}`}
                  <span className="block text-[11px] text-neutral-500">{d.damageDate}</span>
                </td>
                <td className="px-4 py-2 text-neutral-700">
                  {d.productName}
                  <span className="block text-[11px] text-neutral-500">{d.articleNumber}</span>
                </td>
                <td className="px-4 py-2">{d.quantity}</td>
                <td className="px-4 py-2 text-xs capitalize">{d.source.replace("_", " ")}</td>
                <td className="px-4 py-2 text-right tabular-nums">{formatPkr(d.estimatedLossMinor)}</td>
                <td className="px-4 py-2">
                  <StatusBadge status={d.status} />
                </td>
                <td className="px-4 py-2 text-xs text-neutral-600">
                  {d.decision ? d.decision.replace("_", " ") : "—"}
                </td>
                <td className="px-4 py-2">
                  <div className="flex items-center justify-end">
                    {d.status === "open" && (
                      <Button type="button" variant="outline" size="sm" onClick={() => onDecide(d)}>
                        <Wrench className="mr-1 h-3.5 w-3.5" />
                        Decide
                      </Button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------------

function CreateDeliveryDialog({
  session,
  sales,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  sales: SaleDto[];
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const confirmed = sales.filter((s) => s.status === "confirmed");
  const [saleId, setSaleId] = React.useState<number>(0);
  const [qtys, setQtys] = React.useState<Record<number, number>>({});
  const [charge, setCharge] = React.useState(0);
  const [scheduledAt, setScheduledAt] = React.useState("");
  const [contactName, setContactName] = React.useState("");
  const [address, setAddress] = React.useState("");
  const mutation = usePostMutation(
    () =>
      deliveryCreate(session, {
        saleId,
        scheduledAt: scheduledAt || null,
        address: address || null,
        contactName: contactName || null,
        contactPhone: null,
        driverNote: null,
        vehicleNote: null,
        deliveryChargeMinor: charge,
        notes: null,
        items: Object.entries(qtys)
          .filter(([, qty]) => qty > 0)
          .map(([id, qty]) => ({ saleItemId: Number(id), quantity: qty })),
      }),
    onDone,
    onError,
  );
  const sale = confirmed.find((s) => s.id === saleId);
  const hasItems = Object.values(qtys).some((q) => q > 0);

  return (
    <FormDialog
      title="Create delivery"
      description="Ship part or all of a confirmed sale to the customer."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Create delivery"
      submitDisabled={!sale || !hasItems}
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Sale</Label>
        <Select value={saleId ? String(saleId) : undefined} onValueChange={(v) => setSaleId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select a confirmed sale" />
          </SelectTrigger>
          <SelectContent>
            {confirmed.map((s) => (
              <SelectItem key={s.id} value={String(s.id)}>
                {s.saleNumber ?? `#${s.id}`} — {formatPkr(s.totalMinor)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {sale && (
        <div className="grid gap-2 rounded-md border border-neutral-200 p-3">
          {sale.items.map((line) => (
            <div key={line.id} className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-sm text-neutral-800">{line.productName}</p>
                <p className="text-[11px] text-neutral-500">
                  Max {line.quantity} · {formatPkr(line.unitPriceMinor)} each
                </p>
              </div>
              <Input
                type="number"
                min={0}
                max={line.quantity}
                value={qtys[line.id] ?? 0}
                onChange={(e) =>
                  setQtys({ ...qtys, [line.id]: Math.max(0, Math.min(line.quantity, Number(e.target.value))) })
                }
                className="w-24"
              />
            </div>
          ))}
        </div>
      )}

      <div className="grid gap-1.5">
        <Label>Delivery charge</Label>
        <MoneyInput value={charge} onCommit={setCharge} />
      </div>
      <div className="grid gap-1.5">
        <Label>Scheduled date</Label>
        <Input type="date" value={scheduledAt} onChange={(e) => setScheduledAt(e.target.value)} />
      </div>
      <div className="grid gap-1.5">
        <Label>Contact name</Label>
        <Input value={contactName} onChange={(e) => setContactName(e.target.value)} />
      </div>
      <div className="grid gap-1.5">
        <Label>Address</Label>
        <Input value={address} onChange={(e) => setAddress(e.target.value)} />
      </div>
    </FormDialog>
  );
}

function TransitionDialog({
  session,
  delivery,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  delivery: DeliveryDto;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const actions = [...deliveryActionsFor(delivery.status), "reschedule"];
  const [action, setAction] = React.useState<string>(actions[0] ?? "");
  const [reason, setReason] = React.useState("");
  const [receiver, setReceiver] = React.useState("");
  const [proof, setProof] = React.useState("");
  const [scheduledAt, setScheduledAt] = React.useState("");

  const mutation = usePostMutation(
    () =>
      deliveryTransition(session, {
        deliveryId: delivery.id,
        action,
        reason: reason || null,
        receiverName: action === "delivered" ? receiver || null : null,
        proofReference: action === "delivered" ? proof || null : null,
        scheduledAt: action === "reschedule" ? scheduledAt || null : null,
      }),
    onDone,
    onError,
  );
  const needsScheduled = action === "reschedule";
  const submitDisabled = (needsScheduled && !scheduledAt) || (!action && false);

  return (
    <FormDialog
      title={`Delivery ${delivery.deliveryNumber ?? `#${delivery.id}`}`}
      description={`Move delivery from "${delivery.status}".`}
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Confirm"
      submitDisabled={submitDisabled}
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Action</Label>
        <Select value={action} onValueChange={setAction}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {actions.map((a) => (
              <SelectItem key={a} value={a}>
                {a}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {needsScheduled && (
        <div className="grid gap-1.5">
          <Label>New scheduled date</Label>
          <Input type="date" value={scheduledAt} onChange={(e) => setScheduledAt(e.target.value)} />
        </div>
      )}
      {action === "delivered" && (
        <>
          <div className="grid gap-1.5">
            <Label>Receiver name</Label>
            <Input value={receiver} onChange={(e) => setReceiver(e.target.value)} />
          </div>
          <div className="grid gap-1.5">
            <Label>Proof reference</Label>
            <Input value={proof} onChange={(e) => setProof(e.target.value)} />
          </div>
        </>
      )}
      {(action === "failed" || action === "cancelled" || action === "reschedule") && (
        <div className="grid gap-1.5">
          <Label>Reason</Label>
          <Input value={reason} onChange={(e) => setReason(e.target.value)} />
        </div>
      )}
    </FormDialog>
  );
}

function DeliveryDetailDialog({
  session,
  deliveryId,
  onClose,
}: {
  session: string;
  deliveryId: number;
  onClose: () => void;
}) {
  const delivery = useQuery({
    queryKey: ["fulfilment", "delivery", deliveryId],
    queryFn: () => deliveryGet(session, deliveryId),
    enabled: session !== "",
  });
  const d = delivery.data;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{d?.deliveryNumber ?? `Delivery #${deliveryId}`}</DialogTitle>
          <DialogDescription>
            {d ? (
              <>
                Sale {d.saleNumber ?? `#${d.saleId}`}
                <span className="ml-2">
                  <StatusBadge status={d.status} />
                </span>
              </>
            ) : (
              "Loading…"
            )}
          </DialogDescription>
        </DialogHeader>
        {d && (
          <div className="grid max-h-[60vh] gap-4 overflow-y-auto">
            <div className="grid gap-2 text-sm">
              {d.customerName && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Customer</span>
                  <span className="font-medium">{d.customerName}</span>
                </div>
              )}
              {d.contactName && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Contact</span>
                  <span className="font-medium">{d.contactName}{d.contactPhone ? ` · ${d.contactPhone}` : ""}</span>
                </div>
              )}
              {d.address && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Address</span>
                  <span>{d.address}</span>
                </div>
              )}
              <div className="grid grid-cols-[120px_1fr]">
                <span className="text-neutral-500">Scheduled</span>
                <span className="font-medium">
                  {d.scheduledAt ? formatDateTime(d.scheduledAt) : "—"}
                  {d.rescheduleCount > 0 && (
                    <span className="ml-1 text-amber-600">rescheduled ×{d.rescheduleCount}</span>
                  )}
                </span>
              </div>
              <div className="grid grid-cols-[120px_1fr]">
                <span className="text-neutral-500">Charge</span>
                <span className="tabular-nums">{d.deliveryChargeMinor > 0 ? formatPkr(d.deliveryChargeMinor) : "—"}</span>
              </div>
              {d.driverNote && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Driver note</span>
                  <span>{d.driverNote}</span>
                </div>
              )}
              {d.vehicleNote && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Vehicle note</span>
                  <span>{d.vehicleNote}</span>
                </div>
              )}
              {d.receiverName && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Received by</span>
                  <span className="font-medium">{d.receiverName}{d.proofReference ? ` · ${d.proofReference}` : ""}</span>
                </div>
              )}
              {(d.failedReason || d.cancelledReason) && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Outcome note</span>
                  <span className="text-amber-700">{d.failedReason ?? d.cancelledReason}</span>
                </div>
              )}
              {d.notes && (
                <div className="grid grid-cols-[120px_1fr]">
                  <span className="text-neutral-500">Notes</span>
                  <span>{d.notes}</span>
                </div>
              )}
            </div>
            {d.items.length > 0 && (
              <div className="overflow-hidden rounded-lg border border-neutral-200">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b border-neutral-200 bg-neutral-50 text-left text-[11px] font-semibold uppercase tracking-wide text-neutral-500">
                      <th className="px-3 py-2">Item</th>
                      <th className="px-3 py-2 text-right">Qty</th>
                    </tr>
                  </thead>
                  <tbody>
                    {d.items.map((i) => (
                      <tr key={i.id} className="border-b border-neutral-100 last:border-0">
                        <td className="px-3 py-2">{i.productName}</td>
                        <td className="px-3 py-2 text-right tabular-nums">{i.quantity}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <div className="flex justify-end">
              <DeliveryPrint session={session} deliveryId={d.id} />
            </div>
          </div>
        )}
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PostReturnDialog({
  session,
  sales,
  accounts,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  sales: SaleDto[];
  accounts: CashAccountDto[];
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const confirmed = sales.filter((s) => s.status === "confirmed");
  const [saleId, setSaleId] = React.useState<number>(0);
  const [refundType, setRefundType] = React.useState<string>("credit");
  const [cashAccountId, setCashAccountId] = React.useState<number>(0);
  const [lines, setLines] = React.useState<Record<number, { qty: number; classification: string }>>({});
  const [notes, setNotes] = React.useState("");
  const sale = confirmed.find((s) => s.id === saleId);

  const setLine = (id: number, patch: Partial<{ qty: number; classification: string }>) => {
    const prev = lines[id] ?? { qty: 0, classification: "sellable" };
    setLines({ ...lines, [id]: { ...prev, ...patch } });
  };

  const mutation = usePostMutation(
    () =>
      saleReturnPost(session, {
        saleId,
        refundType,
        returnDate: today(),
        cashAccountId: refundType === "cash" ? cashAccountId || null : null,
        notes: notes || null,
        idempotencyKey: crypto.randomUUID(),
        items: Object.entries(lines)
          .filter(([, l]) => l.qty > 0)
          .map(([id, l]) => ({
            saleItemId: Number(id),
            quantity: l.qty,
            classification: l.classification,
          })),
      }),
    onDone,
    onError,
  );

  const hasLines = Object.values(lines).some((l) => l.qty > 0);
  const cashReady = refundType !== "cash" || cashAccountId > 0;

  return (
    <FormDialog
      title="Post return"
      description="Accept returned goods and issue a refund or credit note."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Post return"
      submitDisabled={!sale || !hasLines || !cashReady}
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Sale</Label>
        <Select value={saleId ? String(saleId) : undefined} onValueChange={(v) => setSaleId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select a confirmed sale" />
          </SelectTrigger>
          <SelectContent>
            {confirmed.map((s) => (
              <SelectItem key={s.id} value={String(s.id)}>
                {s.saleNumber ?? `#${s.id}`} — {formatPkr(s.totalMinor)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {sale && (
        <div className="grid gap-2 rounded-md border border-neutral-200 p-3">
          {sale.items.map((line) => {
            const state = lines[line.id] ?? { qty: 0, classification: "sellable" };
            return (
              <div key={line.id} className="grid gap-1">
                <div className="flex items-center justify-between gap-3">
                  <p className="truncate text-sm text-neutral-800">
                    {line.productName}
                    <span className="ml-1 text-[11px] text-neutral-500">{line.quantity} sold</span>
                  </p>
                  <Input
                    type="number"
                    min={0}
                    max={line.quantity}
                    value={state.qty}
                    onChange={(e) =>
                      setLine(line.id, {
                        qty: Math.max(0, Math.min(line.quantity, Number(e.target.value))),
                      })
                    }
                    className="w-24"
                  />
                </div>
                <Select
                  value={state.classification}
                  onValueChange={(v) => setLine(line.id, { classification: v })}
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="sellable">Sellable — restock</SelectItem>
                    <SelectItem value="repair">Damaged — repair</SelectItem>
                    <SelectItem value="damaged">Damaged — keep</SelectItem>
                    <SelectItem value="disposed">Damaged — dispose</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            );
          })}
        </div>
      )}

      <div className="grid gap-1.5">
        <Label>Refund method</Label>
        <Select value={refundType} onValueChange={setRefundType}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="credit">Credit to account</SelectItem>
            <SelectItem value="cash">Refund in cash</SelectItem>
            <SelectItem value="exchange">Exchange — credit note</SelectItem>
          </SelectContent>
        </Select>
      </div>
      {refundType === "cash" && (
        <div className="grid gap-1.5">
          <Label>Cash account</Label>
          <Select value={cashAccountId ? String(cashAccountId) : undefined} onValueChange={(v) => setCashAccountId(Number(v))}>
            <SelectTrigger>
              <SelectValue placeholder="Select a cash account" />
            </SelectTrigger>
            <SelectContent>
              {accounts.map((a) => (
                <SelectItem key={a.id} value={String(a.id)}>
                  {a.code} — {a.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      <div className="grid gap-1.5">
        <Label>Notes</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} />
      </div>
    </FormDialog>
  );
}

function VoidReturnDialog({
  session,
  ret,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  ret: SaleReturnDto;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [reason, setReason] = React.useState("");
  const mutation = usePostMutation(
    () => saleReturnVoid(session, { returnId: ret.id, reason: reason || null }),
    onDone,
    onError,
  );
  return (
    <FormDialog
      title={`Void return ${ret.returnNumber ?? `#${ret.id}`}`}
      description="Reverses the stock, cash and ledger effects of the return."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Void return"
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Reason</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} />
      </div>
    </FormDialog>
  );
}

function DamageRecordDialog({
  session,
  products,
  locations,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  products: ProductListItemDto[];
  locations: LocationDto[];
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [productId, setProductId] = React.useState<number>(0);
  const [locationId, setLocationId] = React.useState<number>(0);
  const [quantity, setQuantity] = React.useState(1);
  const [source, setSource] = React.useState<string>("in_hand");
  const [loss, setLoss] = React.useState(0);
  const [reason, setReason] = React.useState("");

  const mutation = usePostMutation(
    () =>
      damageRecord(session, {
        productId,
        locationId,
        quantity,
        damageDate: today(),
        source,
        reason: reason || null,
        estimatedLossMinor: loss,
        photoPath: null,
      }),
    onDone,
    onError,
  );

  return (
    <FormDialog
      title="Record damage"
      description="Move stock from sellable into the damaged bucket pending a decision."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Record damage"
      submitDisabled={!productId || !locationId || quantity <= 0}
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Product</Label>
        <Select value={productId ? String(productId) : undefined} onValueChange={(v) => setProductId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select a product" />
          </SelectTrigger>
          <SelectContent>
            {products.map((p) => (
              <SelectItem key={p.id} value={String(p.id)}>
                {p.articleNumber} — {p.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-1.5">
        <Label>Location</Label>
        <Select value={locationId ? String(locationId) : undefined} onValueChange={(v) => setLocationId(Number(v))}>
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
      <div className="grid gap-1.5">
        <Label>Quantity</Label>
        <Input type="number" min={1} value={quantity} onChange={(e) => setQuantity(Math.max(1, Number(e.target.value)))} />
      </div>
      <div className="grid gap-1.5">
        <Label>Source</Label>
        <Select value={source} onValueChange={setSource}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="in_hand">In hand</SelectItem>
            <SelectItem value="customer_return">Customer return</SelectItem>
            <SelectItem value="count">Stock count</SelectItem>
            <SelectItem value="other">Other</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-1.5">
        <Label>Estimated loss</Label>
        <MoneyInput value={loss} onCommit={setLoss} />
      </div>
      <div className="grid gap-1.5">
        <Label>Reason</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} />
      </div>
    </FormDialog>
  );
}

function DecideDamageDialog({
  session,
  damage,
  onClose,
  onDone,
  onError,
}: {
  session: string;
  damage: DamageRecordDto;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
}) {
  const [decision, setDecision] = React.useState<string>("repair");
  const [note, setNote] = React.useState("");
  const [linkedSale, setLinkedSale] = React.useState("");
  const mutation = usePostMutation(
    () =>
      damageDecide(session, {
        damageId: damage.id,
        decision,
        decisionNote: note || null,
        linkedSaleId:
          decision === "damaged_sale" ? Number(linkedSale) || null : null,
      }),
    onDone,
    onError,
  );
  return (
    <FormDialog
      title={`Resolve damage ${damage.damageNumber ?? `#${damage.id}`}`}
      description={`${damage.quantity} × ${damage.productName}. Choose the outcome.`}
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Resolve"
      onClose={onClose}
    >
      <div className="grid gap-1.5">
        <Label>Decision</Label>
        <Select value={decision} onValueChange={setDecision}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="repair">Repair — back to sellable</SelectItem>
            <SelectItem value="supplier_return">Return to supplier</SelectItem>
            <SelectItem value="damaged_sale">Sell at discount (linked sale)</SelectItem>
            <SelectItem value="write_off">Write off</SelectItem>
          </SelectContent>
        </Select>
      </div>
      {decision === "damaged_sale" && (
        <div className="grid gap-1.5">
          <Label>Linked sale ID</Label>
          <Input
            type="number"
            min={1}
            value={linkedSale}
            onChange={(e) => setLinkedSale(e.target.value)}
            placeholder="Sale ID confirming the damaged stock"
          />
        </div>
      )}
      <div className="grid gap-1.5">
        <Label>Decision note</Label>
        <Input value={note} onChange={(e) => setNote(e.target.value)} />
      </div>
    </FormDialog>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

function usePostMutation(
  mutate: () => Promise<unknown>,
  onDone: () => void,
  onError: (e: Error) => void,
) {
  return useMutation({ mutationFn: mutate, onSuccess: onDone, onError });
}

export function FulfilmentPage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canViewDeliveries = hasPermission("delivery.view");
  const canCreateDelivery = hasPermission("delivery.create");
  const canUpdateDelivery = hasPermission("delivery.update");
  const canReturn = hasPermission("sale.return");
  const canUseCredit = hasPermission("credit.note.use");
  const canDamage = hasPermission("damage.record");

  const dashboardTarget = React.useMemo(() => takeDashboardTarget("fulfilment"), []);
  const defaultTab: Tab = dashboardTarget?.target === "damage"
    ? "damage"
    : canViewDeliveries
    ? "deliveries"
    : canReturn
      ? "returns"
      : canDamage
        ? "damage"
        : "deliveries";
  const [view, setView] = React.useState<Tab>(defaultTab);
  const [dialog, setDialog] = React.useState<PageDialog>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["fulfilment"] });
    void queryClient.invalidateQueries({ queryKey: ["selling", "sales"] });
    void queryClient.invalidateQueries({ queryKey: ["inventory"] });
  };

  const deliveriesQuery = useQuery({
    queryKey: ["fulfilment", "deliveries"],
    queryFn: () => deliveryList(session, {}),
    enabled: !!session && canViewDeliveries,
  });
  const returnsQuery = useQuery({
    queryKey: ["fulfilment", "returns"],
    queryFn: () => saleReturnList(session, {}),
    enabled: !!session && canReturn,
  });
  const creditNotesQuery = useQuery({
    queryKey: ["fulfilment", "credit-notes"],
    queryFn: () => creditNoteList(session, {}),
    enabled: !!session && canUseCredit,
  });
  const damageQuery = useQuery({
    queryKey: ["fulfilment", "damage"],
    queryFn: () => damageList(session, {}),
    enabled: !!session && canDamage,
  });
  const salesQuery = useQuery({
    queryKey: ["fulfilment", "sales"],
    queryFn: () => saleList(session),
    enabled: !!session && (canCreateDelivery || canReturn),
  });
  const locationsQuery = useQuery({
    queryKey: ["fulfilment", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session && canDamage,
  });
  const accountsQuery = useQuery({
    queryKey: ["fulfilment", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session && canReturn,
  });
  const productsQuery = useQuery({
    queryKey: ["fulfilment", "products"],
    queryFn: () => productList(session, { scope: "active" }),
    enabled: !!session && canDamage,
  });

  const deliveries = deliveriesQuery.data ?? [];
  const returns = returnsQuery.data ?? [];
  const creditNotes = creditNotesQuery.data ?? [];
  const damageRecords = damageQuery.data ?? [];
  const sales = salesQuery.data ?? [];
  const locations = locationsQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];
  const products = productsQuery.data ?? [];

  React.useEffect(() => {
    if (dashboardTarget?.target !== "delivery" || !deliveriesQuery.data) return;
    const delivery = deliveriesQuery.data.find((candidate) => candidate.id === dashboardTarget.id);
    if (delivery) setDialog({ kind: "delivery-detail", delivery });
  }, [dashboardTarget, deliveriesQuery.data]);

  const done = (message: string) => () => {
    invalidate();
    setDialog(null);
    toast({ variant: "success", title: message });
  };
  const failed = (e: Error) => {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    toast({ variant: "error", title: "Operation failed", description: commandErrorMessage(e) });
  };

  const primaryActions: React.ReactNode[] = [];
  if (canViewDeliveries && canCreateDelivery)
    primaryActions.push(
      <Button key="delivery" type="button" onClick={() => setDialog({ kind: "create-delivery" })}>
        <PackagePlus className="mr-2 h-4 w-4" />
        New delivery
      </Button>,
    );
  if (canReturn)
    primaryActions.push(
      <Button key="return" type="button" onClick={() => setDialog({ kind: "post-return" })}>
        <Undo2 className="mr-2 h-4 w-4" />
        Post return
      </Button>,
    );
  if (canDamage)
    primaryActions.push(
      <Button key="damage" type="button" onClick={() => setDialog({ kind: "record-damage" })}>
        <AlertTriangle className="mr-2 h-4 w-4" />
        Record damage
      </Button>,
    );

  return (
    <div>
      <PageHeader
        title="Fulfilment"
        subtitle="Deliveries, sales returns, credit notes and damaged stock."
        actions={
          <>
            {primaryActions}
            <Button type="button" variant="outline" size="icon" onClick={() => void invalidate()} aria-label="Refresh">
              <RefreshCw className="h-4 w-4" />
            </Button>
          </>
        }
      />

      <div className="mt-5 flex items-center gap-1 border-b border-neutral-200">
        {canViewDeliveries && (
          <TabButton active={view === "deliveries"} onClick={() => setView("deliveries")} icon={<Truck className="h-4 w-4" />}>
            Deliveries
          </TabButton>
        )}
        {canReturn && (
          <TabButton active={view === "returns"} onClick={() => setView("returns")} icon={<ClipboardList className="h-4 w-4" />}>
            Returns & credit notes
          </TabButton>
        )}
        {canDamage && (
          <TabButton active={view === "damage"} onClick={() => setView("damage")} icon={<AlertTriangle className="h-4 w-4" />}>
            Damaged stock
          </TabButton>
        )}
      </div>

      <div className="mt-4">
        {view === "deliveries" && canViewDeliveries && (
          <DeliveriesView
            session={session}
            deliveries={deliveries}
            loading={deliveriesQuery.isLoading}
            canCreate={canCreateDelivery}
            canUpdate={canUpdateDelivery}
            onTransition={(d) => setDialog({ kind: "transition", delivery: d })}
            onDetail={(d) => setDialog({ kind: "delivery-detail", delivery: d })}
          />
        )}
        {view === "returns" && canReturn && (
          <ReturnsView
            session={session}
            returns={returns}
            creditNotes={creditNotes}
            loading={returnsQuery.isLoading}
            canUseCredit={canUseCredit}
            onVoid={(r) => setDialog({ kind: "void-return", ret: r })}
          />
        )}
        {view === "damage" && canDamage && (
          <DamageView records={damageRecords} loading={damageQuery.isLoading} onDecide={(d) => setDialog({ kind: "decide-damage", damage: d })} />
        )}
      </div>

      {dialog?.kind === "create-delivery" && (
        <CreateDeliveryDialog session={session} sales={sales} onClose={() => setDialog(null)} onDone={done("Delivery created")} onError={failed} />
      )}
      {dialog?.kind === "transition" && (
        <TransitionDialog session={session} delivery={dialog.delivery} onClose={() => setDialog(null)} onDone={done("Delivery updated")} onError={failed} />
      )}
      {dialog?.kind === "delivery-detail" && (
        <DeliveryDetailDialog session={session} deliveryId={dialog.delivery.id} onClose={() => setDialog(null)} />
      )}
      {dialog?.kind === "post-return" && (
        <PostReturnDialog session={session} sales={sales} accounts={accounts} onClose={() => setDialog(null)} onDone={done("Return posted")} onError={failed} />
      )}
      {dialog?.kind === "void-return" && (
        <VoidReturnDialog session={session} ret={dialog.ret} onClose={() => setDialog(null)} onDone={done("Return voided")} onError={failed} />
      )}
      {dialog?.kind === "record-damage" && (
        <DamageRecordDialog
          session={session}
          products={products}
          locations={locations}
          onClose={() => setDialog(null)}
          onDone={done("Damage recorded")}
          onError={failed}
        />
      )}
      {dialog?.kind === "decide-damage" && (
        <DecideDamageDialog session={session} damage={dialog.damage} onClose={() => setDialog(null)} onDone={done("Damage resolved")} onError={failed} />
      )}
    </div>
  );
}
