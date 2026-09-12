"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { Loader2, Printer, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  loadPrintSettings,
  type PageOrientation,
  type PrinterDestination,
} from "@/components/invoices/invoice-settings";
import { printerList } from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";

export type ReportPrintColumn = {
  header: string;
  key: string;
  alignRight?: boolean;
};

export type ReportPrintSummary = {
  label: string;
  value: string;
};

export type ReportPrintBranding = {
  name: string;
  address?: string | null;
  phone?: string | null;
  logo?: string | null;
};

type Props = {
  session: string;
  title: string;
  period: string;
  columns: ReportPrintColumn[];
  rows: Record<string, string>[];
  summary: ReportPrintSummary[];
  branding: ReportPrintBranding;
  onClose: () => void;
};

function paginate<T>(rows: T[], orientation: PageOrientation, hasSummary: boolean): T[][] {
  const capacity = orientation === "landscape" ? 18 : 30;
  const pages: T[][] = [];
  for (let index = 0; index < rows.length; index += capacity) {
    pages.push(rows.slice(index, index + capacity));
  }
  if (pages.length === 0) pages.push([]);
  if (hasSummary) {
    const last = pages[pages.length - 1];
    const summaryCapacity = capacity - 3;
    if (last.length > summaryCapacity) {
      pages.push(last.splice(summaryCapacity));
    }
  }
  return pages;
}

function ReportSheet({
  title,
  period,
  columns,
  rows,
  summary,
  branding,
  page,
  pageCount,
  orientation,
  marginMm,
}: Omit<Props, "session" | "onClose"> & {
  page: number;
  pageCount: number;
  orientation: PageOrientation;
  marginMm: number;
}) {
  const landscape = orientation === "landscape";
  const generated = new Date().toLocaleString("en-PK");
  const footerInsetMm = Math.max(4, marginMm / 2);
  return (
    <section
      className="report-sheet"
      style={{
        width: landscape ? "297mm" : "210mm",
        height: landscape ? "210mm" : "297mm",
        padding: `${marginMm}mm`,
        paddingBottom: `${marginMm + 12}mm`,
        boxSizing: "border-box",
        background: "white",
        color: "#171a18",
        fontFamily: "'Segoe UI', Arial, sans-serif",
        display: "flex",
        flexDirection: "column",
        position: "relative",
        overflow: "hidden",
      }}
    >
      <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
        <tbody>
          <tr>
            <td style={{ width: "52%", verticalAlign: "top" }}>
              <div style={{ display: "flex", alignItems: "flex-start", gap: 10 }}>
                {branding.logo ? (
                  // eslint-disable-next-line @next/next/no-img-element -- offline saved data URL
                  <img src={branding.logo} alt="" style={{ width: 54, height: 54, objectFit: "contain" }} />
                ) : (
                  <div style={{ width: 50, height: 50, borderRadius: 8, background: "#1c4a2e", color: "white", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: 700 }}>FS</div>
                )}
                <div>
                  <div style={{ color: "#1c4a2e", fontSize: 17, fontWeight: 700 }}>{branding.name}</div>
                  {branding.address && <div style={{ color: "#59615c", fontSize: 10, marginTop: 3 }}>{branding.address}</div>}
                  {branding.phone && <div style={{ color: "#59615c", fontSize: 10, marginTop: 2 }}>Tel: {branding.phone}</div>}
                </div>
              </div>
            </td>
            <td style={{ verticalAlign: "top", textAlign: "right" }}>
              <div style={{ color: "#1c4a2e", fontSize: 20, fontWeight: 700, textTransform: "uppercase" }}>{title}</div>
              <div style={{ color: "#59615c", fontSize: 10, marginTop: 5 }}>Period: {period}</div>
              <div style={{ color: "#737b76", fontSize: 9, marginTop: 3 }}>Generated {generated}</div>
            </td>
          </tr>
        </tbody>
      </table>

      <div style={{ borderTop: "2px solid #1c4a2e", margin: "12px 0 10px" }} />

      <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed", fontSize: 10 }}>
        <thead>
          <tr style={{ height: 34, background: "#1c4a2e", color: "white" }}>
            {columns.map((column) => (
              <th key={column.key} style={{ padding: "0 8px", textAlign: column.alignRight ? "right" : "left", fontWeight: 600 }}>{column.header}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex} style={{ height: 26, background: rowIndex % 2 ? "#f7faf8" : "white", borderBottom: "1px solid #dce6df" }}>
              {columns.map((column) => (
                <td
                  key={column.key}
                  style={{ padding: "0 8px", textAlign: column.alignRight ? "right" : "left", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", fontVariantNumeric: "tabular-nums" }}
                >
                  {row[column.key] ?? ""}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>

      {page === pageCount && summary.length > 0 && (
        <div style={{ display: "grid", gridTemplateColumns: `repeat(${Math.min(summary.length, 4)}, 1fr)`, gap: 1, marginTop: 12, background: "#dbe8df", border: "1px solid #dbe8df" }}>
          {summary.slice(0, 4).map((item) => (
            <div key={item.label} style={{ padding: "8px 10px", background: "#f4f8f5" }}>
              <div style={{ color: "#66706a", fontSize: 8, fontWeight: 600, textTransform: "uppercase", letterSpacing: ".04em" }}>{item.label}</div>
              <div style={{ color: "#1c4a2e", fontSize: 12, fontWeight: 700, marginTop: 3, fontVariantNumeric: "tabular-nums" }}>{item.value}</div>
            </div>
          ))}
        </div>
      )}

      <div
        style={{
          position: "absolute",
          right: `${marginMm}mm`,
          bottom: `${footerInsetMm}mm`,
          left: `${marginMm}mm`,
          paddingTop: 5,
          borderTop: "1px solid #ccd8cf",
          color: "#69716c",
          fontSize: 9,
          lineHeight: 1.2,
        }}
      >
        <div style={{ textAlign: "center" }}>
          Software developed by <strong>EagleNest Creations</strong> (0346-4451505)
        </div>
        <div style={{ marginTop: 3, textAlign: "right" }}>Page {page} of {pageCount}</div>
      </div>
    </section>
  );
}

async function printPreview(root: HTMLDivElement, orientation: PageOrientation, copies: number) {
  const sheets = root.innerHTML;
  const allCopies = Array.from({ length: Math.max(1, Math.min(copies, 10)) }, () => sheets).join("");
  const iframe = document.createElement("iframe");
  iframe.style.cssText = "position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden";
  document.body.appendChild(iframe);

  return new Promise<void>((resolve, reject) => {
    const cleanup = () => { try { iframe.remove(); } catch { /* already removed */ } };
    iframe.onload = () => {
      try {
        const win = iframe.contentWindow;
        if (!win) throw new Error("Could not access the report print window.");
        win.focus();
        win.print();
        window.setTimeout(() => { cleanup(); resolve(); }, 1500);
      } catch (error) { cleanup(); reject(error); }
    };
    iframe.onerror = () => { cleanup(); reject(new Error("Could not prepare the report preview.")); };
    const doc = iframe.contentDocument;
    if (!doc) { cleanup(); reject(new Error("Could not access the report preview.")); return; }
    doc.open();
    doc.write(`<!doctype html><html><head><meta charset="utf-8"><title>Report</title><style>@page{size:A4 ${orientation};margin:0}*{box-sizing:border-box}html,body{margin:0;padding:0;background:#fff;-webkit-print-color-adjust:exact;print-color-adjust:exact}body>.report-page-wrapper{break-after:page;page-break-after:always}body>.report-page-wrapper:last-child{break-after:auto;page-break-after:auto}table{border-collapse:collapse}tr{break-inside:avoid;page-break-inside:avoid}</style></head><body>${allCopies}</body></html>`);
    doc.close();
  });
}

export function ReportPrintPreviewDialog(props: Props) {
  const { toast } = useToast();
  const initial = loadPrintSettings();
  const [orientation, setOrientation] = React.useState<PageOrientation>(props.columns.length > 5 ? "landscape" : "portrait");
  const [destination, setDestination] = React.useState<PrinterDestination>(initial.printerDestination);
  const [printing, setPrinting] = React.useState(false);
  const previewRef = React.useRef<HTMLDivElement>(null);
  const printers = useQuery({ queryKey: ["reports", "printers"], queryFn: () => printerList(props.session), enabled: !!props.session });
  const pages = paginate(props.rows, orientation, props.summary.length > 0);

  const handlePrint = async () => {
    if (!previewRef.current) return;
    setPrinting(true);
    try {
      await printPreview(previewRef.current, orientation, initial.copies);
      toast({ variant: "success", title: "Print window opened", description: "Confirm the selected printer in the system dialog." });
      props.onClose();
    } catch (error) {
      toast({ variant: "error", title: "Print failed", description: commandErrorMessage(error) });
    } finally {
      setPrinting(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => !open && props.onClose()}>
      <DialogContent className="max-h-[95vh] max-w-6xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><Printer className="h-5 w-5" />Print Report</DialogTitle>
          <DialogDescription>Review the populated A4 pages and choose a printer before confirming.</DialogDescription>
        </DialogHeader>
        <div className="grid gap-5 lg:grid-cols-[250px_1fr]">
          <div className="space-y-4 rounded-lg border border-neutral-200 bg-neutral-50 p-4">
            <div className="grid gap-1.5">
              <Label htmlFor="report-printer">Printer destination</Label>
              <Select value={destination} onValueChange={(value) => setDestination(value as PrinterDestination)}>
                <SelectTrigger id="report-printer"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="print-window">Choose in Print Window</SelectItem>
                  <SelectItem value="system-default">Default Printer</SelectItem>
                  {(printers.data ?? []).map((printer) => <SelectItem key={printer.name} value={`printer:${printer.name}`}>{printer.name}{printer.isDefault ? " (default)" : ""}</SelectItem>)}
                </SelectContent>
              </Select>
              <p className="text-[11px] text-neutral-500">The system print dialog provides the final printer confirmation.</p>
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="report-orientation">Orientation</Label>
              <Select value={orientation} onValueChange={(value) => setOrientation(value as PageOrientation)}>
                <SelectTrigger id="report-orientation"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="portrait">Portrait</SelectItem><SelectItem value="landscape">Landscape</SelectItem></SelectContent>
              </Select>
            </div>
            <div className="rounded-md border border-neutral-200 bg-white p-3 text-xs text-neutral-600">
              <p className="font-medium text-neutral-900">{props.title}</p>
              <p className="mt-1">{props.period}</p>
              <p>{props.rows.length.toLocaleString()} records</p>
              <p>{pages.length} A4 page{pages.length === 1 ? "" : "s"}</p>
            </div>
          </div>
          <div className="max-h-[70vh] overflow-auto rounded-lg border border-neutral-200 bg-neutral-200 p-4">
            <div ref={previewRef} className="space-y-4">
              {pages.map((rows, index) => (
                <div key={index} className="report-page-wrapper mx-auto w-fit shadow-lg">
                  <ReportSheet {...props} rows={rows} page={index + 1} pageCount={pages.length} orientation={orientation} marginMm={initial.marginMm} />
                </div>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter className="flex-row justify-between">
          <Button variant="outline" onClick={props.onClose} disabled={printing}><X className="mr-2 h-4 w-4" />Cancel</Button>
          <Button onClick={() => void handlePrint()} disabled={printing}>
            {printing ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Printer className="mr-2 h-4 w-4" />}
            {printing ? "Opening..." : "Print Report"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
