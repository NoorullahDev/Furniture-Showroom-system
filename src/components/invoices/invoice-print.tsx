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
  payments?: { date: string; amount: number; method: string }[],
): Promise<void> {
  return new Promise((resolve, reject) => {
    const paper = "A4";
    const orientation =
      settings.orientation === "landscape" ? "landscape" : "portrait";
    const marginMm = Math.max(2, settings.marginMm ?? 15);
    // Bottom margin must be large enough to contain the fixed developer footer
    const bottomMarginMm = Math.max(14, marginMm);
    const margin = `${marginMm}mm ${marginMm}mm ${bottomMarginMm}mm ${marginMm}mm`;
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
      ${payments && payments.length > 1 ? `<tr><td colspan="6" style="padding-top:10px"><div style="background:#fafafa;border:1px solid #e0e0e0;border-radius:6px;padding:10px 14px"><div style="font-size:8pt;font-weight:600;color:#555;text-transform:uppercase;margin-bottom:6px">Payment history</div><table style="width:100%;border-collapse:collapse;font-size:9pt"><thead><tr style="border-bottom:1px solid #e0e0e0"><th style="padding:3px 0;text-align:left;color:#777">Date</th><th style="padding:3px 0;text-align:left;color:#777">Method</th><th style="padding:3px 0;text-align:right;color:#777">Amount</th></tr></thead><tbody>${payments.map((p) => `<tr style="border-bottom:1px solid #f0f0f0"><td style="padding:3px 0">${escHtml(p.date)}</td><td style="padding:3px 0;color:#555">${escHtml(p.method)}</td><td style="padding:3px 0;text-align:right;font-weight:600;color:#2e7d32">${fmt(p.amount)}</td></tr>`).join("")}</tbody></table></div></td></tr>` : ""}
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
  @media print {
    @page { size: ${paper} ${orientation}; margin: 0mm !important; }
    body { margin: 0 !important; padding: 0 !important; }
  }
  @page { size: ${paper} ${orientation}; margin: 0mm; }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: 'Segoe UI', Arial, sans-serif; font-size: ${fs}; color: #1a1a1a; background: #fff; -webkit-print-color-adjust: exact; print-color-adjust: exact; }
  table { border-collapse: collapse; }
  thead { display: table-header-group; }
  tfoot { display: table-row-group; }
  tr { page-break-inside: avoid; }
  .main-table { width: 100%; }
  .main-thead { display: table-header-group; height: ${marginMm}mm; }
  .main-tfoot { display: table-footer-group; height: ${bottomMarginMm}mm; }
  .developer-footer { position: fixed; right: 0; bottom: ${Math.max(2, marginMm - 5)}mm; left: 0; text-align: center; color: #777; font-size: 8pt; line-height: 1.2; }
</style>
</head>
<body>
  <table class="main-table">
    <thead class="main-thead"><tr><td></td></tr></thead>
    <tbody>
      <tr><td style="padding: 0 ${marginMm}mm">
        ${allCopies}
      </td></tr>
    </tbody>
    <tfoot class="main-tfoot"><tr><td></td></tr></tfoot>
  </table>
  <div class="developer-footer">Software developed by <strong>EagleNest Creations</strong> (0346-4451505)</div>
</body>
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
