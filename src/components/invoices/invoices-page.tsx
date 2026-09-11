"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Printer, Search, FileText, Eye, RefreshCw,
  ReceiptText, X, AlertTriangle, Loader2,
} from "lucide-react";
import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogDescription,
  DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";
import {
  Table, TableBody, TableCell,
  TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { useToast } from "@/components/ui/toast";
import { useSession, isSessionError } from "@/components/session/session-provider";
import { printerList, saleList, saleGet, settingsGet, shopLogoGet, customerGet } from "@/lib/tauri/api";
import type { SaleDto } from "@/lib/tauri/api";
import { formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { InvoiceA4 } from "./invoice-a4";
import {
  loadPrintSettings,
  type FontSize,
  type InvoicePrintSettings,
  type PageOrientation,
  type PaperSize,
  type PrinterDestination,
} from "./invoice-settings";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parseSettingString(raw: string | null | undefined): string | null {
  if (raw == null) return null;
  try {
    const p = JSON.parse(raw) as unknown;
    return typeof p === "string" ? p : raw;
  } catch {
    return raw;
  }
}

function escHtml(s: string): string {
  return s
    .replace(/[&]/g, "&amp;")
    .replace(/[<]/g, "&lt;")
    .replace(/[>]/g, "&gt;")
    .replace(/["]/g, "&quot;")
    .replace(/[']/g, "&#39;");
}

function InvoiceStatusBadge({ status }: { status: string }) {
  if (status === "confirmed") return <Badge variant="success">Confirmed</Badge>;
  if (status === "cancelled") return <Badge variant="danger">Cancelled</Badge>;
  return <Badge variant="neutral">{status}</Badge>;
}

function SummaryCard({
  label, value, money, accent,
}: {
  label: string;
  value: number;
  money?: boolean;
  accent?: "green" | "rose";
}) {
  const formatted = money ? formatPkr(value) : String(value);
  const color = accent === "green"
    ? "text-emerald-700"
    : accent === "rose"
      ? "text-rose-600"
      : "text-neutral-900";
  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-4 shadow-sm">
      <p className="text-xs text-neutral-500">{label}</p>
      <p className={`mt-1 text-xl font-semibold tabular-nums ${color}`}>{formatted}</p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

export function InvoicesPage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const session = profile?.sessionId ?? "";
  const canPrint = hasPermission("invoice.print");

  const [q, setQ] = React.useState("");
  const [viewingSale, setViewingSale] = React.useState<SaleDto | null>(null);
  const [printingSale, setPrintingSale] = React.useState<SaleDto | null>(null);

  const salesQuery = useQuery({
    queryKey: ["invoices", "sales"],
    queryFn: () => saleList(session),
    enabled: !!session && canPrint,
  });

  const shopNameQuery = useQuery({
    queryKey: ["invoices", "shop-name"],
    queryFn: () => settingsGet(session, "shop.name"),
    enabled: !!session,
  });
  const shopAddressQuery = useQuery({
    queryKey: ["invoices", "shop-address"],
    queryFn: () => settingsGet(session, "shop.address"),
    enabled: !!session,
  });
  const shopPhoneQuery = useQuery({
    queryKey: ["invoices", "shop-phone"],
    queryFn: () => settingsGet(session, "shop.phone"),
    enabled: !!session,
  });
  const logoQuery = useQuery({
    queryKey: ["branding", "logo"],
    queryFn: () => shopLogoGet(session),
    enabled: !!session,
  });

  const shopName = parseSettingString(shopNameQuery.data) ?? "Furniture Shop";
  const shopAddress = parseSettingString(shopAddressQuery.data);
  const shopPhone = parseSettingString(shopPhoneQuery.data);

  const allSales = salesQuery.data ?? [];
  const confirmed = allSales.filter((s) => s.status === "confirmed");
  const term = q.trim().toLowerCase();
  const filtered = term
    ? confirmed.filter(
        (s) =>
          (s.saleNumber ?? `#${s.id}`).toLowerCase().includes(term) ||
          (s.customerName ?? "").toLowerCase().includes(term),
      )
    : confirmed;

  const handleError = (e: unknown) => {
    if (isSessionError(e)) { refresh(); return; }
    toast({ variant: "error", title: "Error", description: commandErrorMessage(e) });
  };

  if (!canPrint) {
    return (
      <div className="flex flex-col items-center justify-center gap-4 rounded-lg border border-dashed border-neutral-300 p-16 text-center">
        <AlertTriangle className="h-10 w-10 text-amber-400" />
        <div>
          <p className="text-sm font-medium text-neutral-800">Permission required</p>
          <p className="mt-1 text-sm text-neutral-500">
            You need the Invoice Print permission to access this page.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title="Invoices"
        subtitle="View, print, and reprint invoices from confirmed sales history."
      />

      {/* Summary */}
      <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <SummaryCard label="Total invoices" value={confirmed.length} />
        <SummaryCard
          label="Total value"
          value={confirmed.reduce((a, s) => a + s.totalMinor, 0)}
          money
        />
        <SummaryCard
          label="Collected"
          value={confirmed.reduce((a, s) => a + s.paidMinor + s.advanceUsedMinor, 0)}
          money
          accent="green"
        />
        <SummaryCard
          label="Outstanding"
          value={confirmed.reduce((a, s) => a + s.dueMinor, 0)}
          money
          accent="rose"
        />
      </div>

      {/* Search + refresh */}
      <div className="mt-5 flex items-center gap-3">
        <div className="relative max-w-sm flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search invoice # or customer..."
            className="pl-8"
          />
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => salesQuery.refetch()}
          disabled={salesQuery.isFetching}
        >
          {salesQuery.isFetching
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <RefreshCw className="h-4 w-4" />}
          Refresh
        </Button>
      </div>

      {/* Invoice table */}
      <div className="mt-4 overflow-hidden rounded-lg border border-neutral-200 bg-white">
        {salesQuery.isLoading ? (
          <div className="flex items-center justify-center gap-2 p-10 text-sm text-neutral-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading invoices...
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex flex-col items-center gap-3 p-12 text-center">
            <ReceiptText className="h-10 w-10 text-neutral-300" />
            <p className="text-sm font-medium text-neutral-600">
              {q ? "No invoices match your search." : "No confirmed sales yet."}
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Invoice #</TableHead>
                  <TableHead>Customer</TableHead>
                  <TableHead>Date</TableHead>
                  <TableHead>Items</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead className="text-right">Paid</TableHead>
                  <TableHead className="text-right">Balance</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((s) => (
                  <TableRow key={s.id}>
                    <TableCell className="font-medium text-neutral-900">
                      <span className="flex items-center gap-1.5">
                        <FileText className="h-3.5 w-3.5 text-neutral-400" />
                        {s.saleNumber ?? `#${s.id}`}
                      </span>
                    </TableCell>
                    <TableCell>
                      {s.customerName ?? (
                        <span className="text-neutral-400">Walk-in</span>
                      )}
                    </TableCell>
                    <TableCell className="whitespace-nowrap">{s.saleDate}</TableCell>
                    <TableCell>{s.items.length}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatPkr(s.totalMinor)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums text-emerald-700">
                      {formatPkr(s.paidMinor + s.advanceUsedMinor)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      <span className={s.dueMinor > 0 ? "font-medium text-rose-600" : "text-emerald-700"}>
                        {formatPkr(s.dueMinor)}
                      </span>
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => setViewingSale(s)}
                        >
                          <Eye className="mr-1 h-3.5 w-3.5" />
                          View
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => setPrintingSale(s)}
                        >
                          <Printer className="mr-1 h-3.5 w-3.5" />
                          Print
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      {/* Dialogs */}
      {viewingSale && (
        <ViewInvoiceDialog
          session={session}
          sale={viewingSale}
          shopName={shopName}
          shopAddress={shopAddress}
          shopPhone={shopPhone}
          logoDataUrl={logoQuery.data}
          onClose={() => setViewingSale(null)}
          onPrint={(s) => {
            setViewingSale(null);
            setPrintingSale(s);
          }}
        />
      )}
      {printingSale && (
        <PrintPreviewDialog
          session={session}
          sale={printingSale}
          shopName={shopName}
          shopAddress={shopAddress}
          shopPhone={shopPhone}
          logoDataUrl={logoQuery.data}
          onClose={() => setPrintingSale(null)}
          onError={handleError}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// View Invoice Dialog
// ---------------------------------------------------------------------------

function ViewInvoiceDialog({
  session,
  sale,
  shopName,
  shopAddress,
  shopPhone,
  logoDataUrl,
  onClose,
  onPrint,
}: {
  session: string;
  sale: SaleDto;
  shopName: string;
  shopAddress?: string | null;
  shopPhone?: string | null;
  logoDataUrl?: string | null;
  onClose: () => void;
  onPrint: (s: SaleDto) => void;
}) {
  const detailQuery = useQuery({
    queryKey: ["invoices", "sale-detail", sale.id],
    queryFn: () => saleGet(session, sale.id),
    enabled: !!session,
  });
  const current = detailQuery.data ?? sale;
  const settings = loadPrintSettings();

  const customerQuery = useQuery({
    queryKey: ["invoices", "customer", current.customerId],
    queryFn: () => customerGet(session, current.customerId!),
    enabled: !!session && !!current.customerId,
  });
  const cust = customerQuery.data;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            Invoice {current.saleNumber ?? `#${current.id}`}
          </DialogTitle>
          <DialogDescription>
            {current.customerName ?? "Walk-in customer"} · {current.saleDate} ·{" "}
            <InvoiceStatusBadge status={current.status} />
          </DialogDescription>
        </DialogHeader>

        <div className="overflow-auto rounded-lg border border-neutral-200 bg-neutral-50 p-4">
          <div
            className="mx-auto bg-white shadow-sm"
            style={{ width: "794px", minHeight: "1123px" }}
          >
            {detailQuery.isLoading ? (
              <div className="flex items-center justify-center p-20">
                <Loader2 className="h-8 w-8 animate-spin text-neutral-400" />
              </div>
            ) : (
              <InvoiceA4
                sale={current}
                shopName={shopName}
                shopAddress={shopAddress}
                shopPhone={shopPhone}
                logoDataUrl={logoDataUrl}
                customerPhone={cust?.phone}
                customerAddress={cust?.address}
                settings={settings}
                copies={1}
              />
            )}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
          <Button onClick={() => onPrint(current)}>
            <Printer className="mr-2 h-4 w-4" />
            Print Invoice
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Print Preview Dialog
// ---------------------------------------------------------------------------

function PrintPreviewDialog({
  session,
  sale,
  shopName,
  shopAddress,
  shopPhone,
  logoDataUrl,
  onClose,
  onError,
}: {
  session: string;
  sale: SaleDto;
  shopName: string;
  shopAddress?: string | null;
  shopPhone?: string | null;
  logoDataUrl?: string | null;
  onClose: () => void;
  onError: (e: unknown) => void;
}) {
  const { toast } = useToast();

  const detailQuery = useQuery({
    queryKey: ["invoices", "sale-detail", sale.id],
    queryFn: () => saleGet(session, sale.id),
    enabled: !!session,
  });
  const current = detailQuery.data ?? sale;

  const customerQuery = useQuery({
    queryKey: ["invoices", "customer", current.customerId],
    queryFn: () => customerGet(session, current.customerId!),
    enabled: !!session && !!current.customerId,
  });
  const cust = customerQuery.data;

  const saved = loadPrintSettings();
  const [jobSettings, setJobSettings] = React.useState<InvoicePrintSettings>(saved);
  const [printing, setPrinting] = React.useState(false);
  const printersQuery = useQuery({
    queryKey: ["settings", "printers"],
    queryFn: () => printerList(session),
    enabled: !!session,
  });
  const updateJob = <K extends keyof InvoicePrintSettings>(key: K, value: InvoicePrintSettings[K]) =>
    setJobSettings((currentSettings) => ({ ...currentSettings, [key]: value }));

  const handlePrint = async () => {
    setPrinting(true);
    try {
      await printInvoiceA4(
        current,
        jobSettings,
        shopName,
        shopAddress,
        shopPhone,
        logoDataUrl,
        cust?.phone,
        cust?.address,
      );
      toast({ variant: "success", title: "Print window opened" });
      onClose();
    } catch (e) {
      toast({
        variant: "error",
        title: "Print failed",
        description: commandErrorMessage(e),
      });
      onError(e);
    } finally {
      setPrinting(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-5xl max-h-[95vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Printer className="h-5 w-5" />
            Print Invoice &ndash; {current.saleNumber ?? `#${current.id}`}
          </DialogTitle>
          <DialogDescription>
            Preview your invoice. Adjust options then click Print.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-[240px_1fr]">
          {/* Sidebar controls */}
          <div className="space-y-5 rounded-lg border border-neutral-200 bg-neutral-50 p-4">
            <div>
              <p className="mb-3 text-xs font-semibold uppercase tracking-wider text-neutral-500">
                Print Options
              </p>
              <div className="space-y-3">
                <div className="grid gap-1.5">
                  <Label htmlFor="pp-printer">Printer destination</Label>
                  <Select
                    value={jobSettings.printerDestination}
                    onValueChange={(v) => updateJob("printerDestination", v as PrinterDestination)}
                  >
                    <SelectTrigger id="pp-printer">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="print-window">
                        Open in Print Window
                      </SelectItem>
                      <SelectItem value="system-default">
                        Default Printer (System Default)
                      </SelectItem>
                      {(printersQuery.data ?? []).map((installed) => (
                        <SelectItem key={installed.name} value={`printer:${installed.name}`}>
                          {installed.name}{installed.isDefault ? " (default)" : ""}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-[11px] text-neutral-500">
                    {jobSettings.printerDestination === "print-window"
                      ? "The browser print dialog will open. Choose your printer and paper settings there."
                      : "The print window opens with this destination preference; confirm the final printer in the system dialog."}
                  </p>
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="pp-paper">Paper size</Label>
                  <Select value={jobSettings.paperSize} onValueChange={(v) => updateJob("paperSize", v as PaperSize)}>
                    <SelectTrigger id="pp-paper"><SelectValue /></SelectTrigger>
                    <SelectContent><SelectItem value="a4">A4 — 210 × 297 mm</SelectItem></SelectContent>
                  </Select>
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="pp-orientation">Orientation</Label>
                  <Select value={jobSettings.orientation} onValueChange={(v) => updateJob("orientation", v as PageOrientation)}>
                    <SelectTrigger id="pp-orientation"><SelectValue /></SelectTrigger>
                    <SelectContent><SelectItem value="portrait">Portrait</SelectItem><SelectItem value="landscape">Landscape</SelectItem></SelectContent>
                  </Select>
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="pp-font">Font size</Label>
                  <Select value={jobSettings.fontSize} onValueChange={(v) => updateJob("fontSize", v as FontSize)}>
                    <SelectTrigger id="pp-font"><SelectValue /></SelectTrigger>
                    <SelectContent><SelectItem value="small">Small</SelectItem><SelectItem value="normal">Normal</SelectItem><SelectItem value="large">Large</SelectItem></SelectContent>
                  </Select>
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="pp-margin">Margins (mm)</Label>
                  <Input id="pp-margin" type="number" min={5} max={40} value={jobSettings.marginMm} onChange={(e) => updateJob("marginMm", Math.max(5, Math.min(40, Number(e.target.value) || 15)))} />
                </div>

                <div className="grid gap-1.5">
                  <Label htmlFor="pp-copies">Copies</Label>
                  <Input
                    id="pp-copies"
                    type="number"
                    min={1}
                    max={10}
                    value={jobSettings.copies}
                    onChange={(e) =>
                      updateJob("copies",
                        Math.max(1, Math.min(10, Number(e.target.value) || 1)),
                      )
                    }
                  />
                  {jobSettings.copies > 1 && (
                    <p className="text-[11px] text-neutral-500">
                      {jobSettings.copies} copies will print. Keep the OS dialog copy count at 1 to avoid multiplication.
                    </p>
                  )}
                </div>
              </div>
            </div>

            {/* Invoice summary */}
            <div className="space-y-1 rounded-md border border-neutral-200 bg-white p-3 text-xs text-neutral-600">
              <p className="font-medium text-neutral-800">Invoice details</p>
              <p>Number: {current.saleNumber ?? `#${current.id}`}</p>
              <p>Date: {current.saleDate}</p>
              <p>Items: {current.items.length}</p>
              <p>Total: {formatPkr(current.totalMinor)}</p>
              <p>
                Balance:{" "}
                <span
                  className={
                    current.dueMinor > 0
                      ? "font-semibold text-rose-600"
                      : "text-emerald-700"
                  }
                >
                  {formatPkr(current.dueMinor)}
                </span>
              </p>
            </div>

            <div className="rounded-md border border-amber-200 bg-amber-50 p-3 text-xs text-amber-800">
              <p className="font-medium">Print safety</p>
              <p className="mt-1">
                Printing never creates new sales or changes stock, payments, or customer balances.
              </p>
            </div>
          </div>

          {/* A4 preview */}
          <div className="overflow-auto rounded-lg border border-neutral-200 bg-neutral-100 p-4">
            <div
              className="mx-auto bg-white shadow-lg"
              style={{ width: "794px", minHeight: "1123px" }}
            >
              {detailQuery.isLoading ? (
                <div className="flex items-center justify-center p-20">
                  <Loader2 className="h-8 w-8 animate-spin text-neutral-400" />
                </div>
              ) : (
                <InvoiceA4
                  sale={current}
                  shopName={shopName}
                  shopAddress={shopAddress}
                  shopPhone={shopPhone}
                  logoDataUrl={logoDataUrl}
                  settings={jobSettings}
                  copies={jobSettings.copies}
                />
              )}
            </div>
          </div>
        </div>

        <DialogFooter className="flex-row justify-between">
          <Button variant="outline" onClick={onClose} disabled={printing}>
            <X className="mr-2 h-4 w-4" />
            Cancel
          </Button>
          <Button
            onClick={() => void handlePrint()}
            disabled={printing || detailQuery.isLoading}
          >
            {printing ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Printing...
              </>
            ) : (
              <>
                <Printer className="mr-2 h-4 w-4" />
                Print Invoice
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Core print engine
// ---------------------------------------------------------------------------

/**
 * triggerPrint
 *
 * Builds a complete A4 HTML document and injects it into a hidden iframe,
 * then calls iframe.contentWindow.print(). This is purely read-only;
 * it NEVER calls any Tauri command that creates or modifies data.
 *
 * Copies > 1 are embedded as separate invoice blocks separated by
 * CSS page breaks so the OS print dialog copy count should stay at 1.
 */
export async function printInvoiceA4(
  sale: SaleDto,
  settings: InvoicePrintSettings,
  shopName: string,
  shopAddress?: string | null,
  shopPhone?: string | null,
  logoDataUrl?: string | null,
  customerPhone?: string | null,
  customerAddress?: string | null,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const paper = "A4";
    const orientation =
      settings.orientation === "landscape" ? "landscape" : "portrait";
    const margin = `${settings.marginMm ?? 15}mm`;
    const fs =
      settings.fontSize === "small"
        ? "10pt"
        : settings.fontSize === "large"
          ? "12pt"
          : "11pt";
    const copies = Math.max(1, Math.min(settings.copies ?? 1, 10));
    const invoiceNumber = sale.saleNumber ?? `#${sale.id}`;

    const fmt = (minor: number) => {
      const neg = minor < 0;
      const abs = Math.abs(minor);
      const r = Math.floor(abs / 100);
      const p = abs % 100;
      return `${neg ? "-" : ""}PKR ${r.toLocaleString("en-PK")}.${String(p).padStart(2, "0")}`;
    };

    const rows = sale.items
      .map(
        (item, idx) =>
          `<tr style="border-bottom:1px solid #e8efe9;background:${idx % 2 === 0 ? "#fff" : "#f9fcfa"};page-break-inside:avoid">
            <td style="padding:6px 10px;font-size:9pt;color:#777">${idx + 1}</td>
            <td style="padding:6px 10px">
              <div style="font-weight:600;color:#111">${escHtml(item.productName)}</div>
              ${item.bundleId ? '<div style="font-size:8pt;color:#888;margin-top:2px">Furniture Set</div>' : ""}
            </td>
            <td style="padding:6px 10px;font-size:9pt;color:#666">${escHtml(item.articleNumber)}</td>
            <td style="padding:6px 10px;text-align:right;font-size:9pt">${item.quantity}</td>
            <td style="padding:6px 10px;text-align:right;font-size:9pt">${fmt(item.unitPriceMinor)}</td>
            <td style="padding:6px 10px;text-align:right;font-size:9pt;font-weight:600">${fmt(item.lineTotalMinor)}</td>
          </tr>`,
      )
      .join("");

    const bal = sale.dueMinor;
    const totPaid = sale.paidMinor + sale.advanceUsedMinor;

    const invoiceHtml = `
<div class="inv">
  <table style="width:100%;border-collapse:collapse;margin-bottom:20px"><tbody><tr>
    <td style="vertical-align:top;width:60%">
      ${settings.showLogo ? logoDataUrl ? `<div style="width:52px;height:52px;margin-bottom:8px"><img src="${escHtml(logoDataUrl)}" alt="" style="width:100%;height:100%;object-fit:contain"></div>` : '<div style="width:52px;height:52px;background:#1c4a2e;border-radius:8px;display:inline-flex;align-items:center;justify-content:center;margin-bottom:8px"><span style="color:#fff;font-weight:700;font-size:18px">FS</span></div><br>' : ""}
      <span style="font-weight:700;font-size:17px;color:#1c4a2e">${escHtml(shopName)}</span>
      ${settings.showAddress && shopAddress ? `<div style="font-size:9pt;color:#555;margin-top:2px">${escHtml(shopAddress)}</div>` : ""}
      ${settings.showPhone && shopPhone ? `<div style="font-size:9pt;color:#555">Tel: ${escHtml(shopPhone)}</div>` : ""}
    </td>
    <td style="vertical-align:top;text-align:right">
      <div style="font-size:22px;font-weight:700;color:#1c4a2e;margin-bottom:6px">INVOICE</div>
      <table style="border-collapse:collapse;margin-left:auto"><tbody>
        <tr><td style="font-size:9pt;color:#777;padding-right:8px">Invoice No.</td><td style="font-size:9pt;font-weight:600">${escHtml(invoiceNumber)}</td></tr>
        <tr><td style="font-size:9pt;color:#777;padding-right:8px">Date</td><td style="font-size:9pt;font-weight:600">${sale.saleDate}</td></tr>
        ${sale.confirmedAt ? `<tr><td style="font-size:9pt;color:#777;padding-right:8px">Confirmed</td><td style="font-size:9pt">${new Date(sale.confirmedAt).toLocaleDateString("en-GB")}</td></tr>` : ""}
      </tbody></table>
    </td>
  </tr></tbody></table>
  <div style="border-top:2px solid #1c4a2e;margin-bottom:16px"></div>
  ${sale.customerName ? `<div style="background:#f5f9f6;border:1px solid #d4e8d9;border-radius:6px;padding:12px 16px;margin-bottom:20px"><div style="font-size:8pt;font-weight:600;color:#1c4a2e;text-transform:uppercase;margin-bottom:4px">Bill To</div><div style="font-weight:600;font-size:11pt">${escHtml(sale.customerName)}</div>${customerPhone ? `<div style="font-size:9pt;color:#555;margin-top:2px">Tel: ${escHtml(customerPhone)}</div>` : ""}${customerAddress ? `<div style="font-size:9pt;color:#555;margin-top:2px">${escHtml(customerAddress)}</div>` : ""}</div>` : ""}
  <table style="width:100%;border-collapse:collapse">
    <thead>
      <tr style="background:#1c4a2e;color:#fff">
        <th style="padding:8px 10px;text-align:left;font-size:9pt;width:28px">#</th>
        <th style="padding:8px 10px;text-align:left;font-size:9pt">Description</th>
        <th style="padding:8px 10px;text-align:left;font-size:9pt">Article</th>
        <th style="padding:8px 10px;text-align:right;font-size:9pt;width:44px">Qty</th>
        <th style="padding:8px 10px;text-align:right;font-size:9pt;width:110px">Unit Price</th>
        <th style="padding:8px 10px;text-align:right;font-size:9pt;width:110px">Amount</th>
      </tr>
    </thead>
    <tbody>${rows}</tbody>
    <tfoot>
      <tr><td colspan="6" style="padding-top:16px">
        <table style="width:50%;border-collapse:collapse;margin-left:auto"><tbody>
          <tr><td style="padding:3px 10px;font-size:9pt;color:#555">Subtotal</td><td style="padding:3px 10px;text-align:right;font-size:9pt">${fmt(sale.subtotalMinor)}</td></tr>
          ${sale.discountMinor > 0 ? `<tr><td style="padding:3px 10px;font-size:9pt;color:#555">Discount</td><td style="padding:3px 10px;text-align:right;font-size:9pt;color:#d32f2f">- ${fmt(sale.discountMinor)}</td></tr>` : ""}
          ${sale.deliveryChargeMinor > 0 ? `<tr><td style="padding:3px 10px;font-size:9pt;color:#555">Delivery</td><td style="padding:3px 10px;text-align:right;font-size:9pt">${fmt(sale.deliveryChargeMinor)}</td></tr>` : ""}
          <tr style="border-top:2px solid #1c4a2e"><td style="padding:6px 10px;font-size:11pt;font-weight:700;color:#1c4a2e">Total</td><td style="padding:6px 10px;text-align:right;font-size:11pt;font-weight:700;color:#1c4a2e">${fmt(sale.totalMinor)}</td></tr>
          <tr><td style="padding:3px 10px;font-size:9pt;color:#555">Amount paid</td><td style="padding:3px 10px;text-align:right;font-size:9pt;color:#2e7d32">${fmt(sale.paidMinor)}</td></tr>
          ${sale.advanceUsedMinor > 0 ? `<tr><td style="padding:3px 10px;font-size:9pt;color:#555">Advance used</td><td style="padding:3px 10px;text-align:right;font-size:9pt;color:#2e7d32">${fmt(sale.advanceUsedMinor)}</td></tr>` : ""}
          <tr style="background:${bal > 0 ? "#fff8f0" : "#f0faf2"};border-top:2px solid ${bal > 0 ? "#e65100" : "#2e7d32"}">
            <td style="padding:6px 10px;font-size:11pt;font-weight:700;color:${bal > 0 ? "#e65100" : "#2e7d32"}">${bal > 0 ? "Balance due" : "Fully paid"}</td>
            <td style="padding:6px 10px;text-align:right;font-size:11pt;font-weight:700;color:${bal > 0 ? "#e65100" : "#2e7d32"}">${fmt(Math.abs(bal))}</td>
          </tr>
        </tbody></table>
      </td></tr>
      ${settings.showPaymentDetails && totPaid > 0 ? `<tr><td colspan="6" style="padding-top:14px"><div style="background:#f5f9f6;border:1px solid #d4e8d9;border-radius:6px;padding:10px 14px"><div style="font-size:8pt;font-weight:600;color:#1c4a2e;text-transform:uppercase;margin-bottom:4px">Payment summary</div><div style="font-size:9pt"><span style="color:#555">Cash received: </span><span style="font-weight:600">${fmt(sale.paidMinor)}</span>${sale.advanceUsedMinor > 0 ? ` &nbsp;|&nbsp; <span style="color:#555">Advance applied: </span><span style="font-weight:600">${fmt(sale.advanceUsedMinor)}</span>` : ""}</div></div></td></tr>` : ""}
      ${sale.notes ? `<tr><td colspan="6" style="padding-top:10px;font-size:9pt;color:#555"><strong>Notes: </strong>${escHtml(sale.notes)}</td></tr>` : ""}
      ${settings.footerText ? `<tr><td colspan="6" style="padding-top:24px;border-top:1px solid #d4e8d9"><div style="font-size:9pt;color:#777;text-align:center">${escHtml(settings.footerText)}</div></td></tr>` : ""}
      <tr><td colspan="6" style="padding-top:40px"><table style="width:100%;border-collapse:collapse"><tbody><tr>
        <td style="width:50%;vertical-align:bottom;text-align:center"><div style="border-top:1px solid #999;width:200px;margin:0 auto 4px auto"></div><div style="font-size:8pt;color:#777">Authorized Signature</div></td>
        <td style="width:50%;vertical-align:bottom;text-align:center"><div style="border-top:1px solid #999;width:200px;margin:0 auto 4px auto"></div><div style="font-size:8pt;color:#777">Customer Signature</div></td>
      </tr></tbody></table></td></tr>
    </tfoot>
  </table>
</div>`;

    const allCopies = Array.from({ length: copies })
      .map((_, i) =>
        i > 0
          ? `<div style="page-break-before:always;break-before:page;height:0;margin:0"></div>${invoiceHtml}`
          : invoiceHtml,
      )
      .join("");

    const doc = `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Invoice ${escHtml(invoiceNumber)}</title>
<style>
  @page { size: ${paper} ${orientation}; margin: ${margin}; }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: 'Segoe UI', Arial, sans-serif; font-size: ${fs}; color: #1a1a1a; background: #fff; -webkit-print-color-adjust: exact; print-color-adjust: exact; }
  table { border-collapse: collapse; }
  thead { display: table-header-group; }
  tfoot { display: table-footer-group; }
  tr { page-break-inside: avoid; }
</style>
</head>
<body>${allCopies}</body>
</html>`;

    const iframe = document.createElement("iframe");
    iframe.style.cssText = "position:fixed;right:0;bottom:0;width:0;height:0;border:none;visibility:hidden";
    document.body.appendChild(iframe);

    const cleanup = () => {
      try { document.body.removeChild(iframe); } catch { /* already removed */ }
    };

    iframe.onload = () => {
      try {
        const win = iframe.contentWindow;
        if (!win) { cleanup(); reject(new Error("Could not access print window.")); return; }
        win.focus();
        win.print();
        setTimeout(() => { cleanup(); resolve(); }, 1500);
      } catch (e) { cleanup(); reject(e as Error); }
    };

    iframe.onerror = () => { cleanup(); reject(new Error("Failed to load print frame.")); };

    const iframeDoc =
      iframe.contentDocument ?? iframe.contentWindow?.document;
    if (!iframeDoc) { cleanup(); reject(new Error("Could not access iframe document.")); return; }
    iframeDoc.open();
    iframeDoc.write(doc);
    iframeDoc.close();
  });
}
