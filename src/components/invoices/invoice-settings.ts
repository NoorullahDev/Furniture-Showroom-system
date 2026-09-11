/**
 * invoice-settings.ts
 *
 * Shared type for Invoice & Print Settings.
 * Stored in localStorage so they persist across app restarts.
 */

export type PrinterDestination = "print-window" | "system-default" | `printer:${string}`;
export type PaperSize = "a4";
export type PageOrientation = "portrait" | "landscape";
export type FontSize = "small" | "normal" | "large";

export interface InvoicePrintSettings {
  printerDestination: PrinterDestination;
  paperSize: PaperSize;
  orientation: PageOrientation;
  marginMm: number;
  fontSize: FontSize;
  showLogo: boolean;
  showAddress: boolean;
  showPhone: boolean;
  showPaymentDetails: boolean;
  footerText: string;
  copies: number;
}

export const DEFAULT_PRINT_SETTINGS: InvoicePrintSettings = {
  printerDestination: "print-window",
  paperSize: "a4",
  orientation: "portrait",
  marginMm: 15,
  fontSize: "normal",
  showLogo: true,
  showAddress: true,
  showPhone: true,
  showPaymentDetails: true,
  footerText: "Thank you for your business!",
  copies: 1,
};

const STORAGE_KEY = "invoice_print_settings";
export const PRINT_SETTINGS_CHANGED_EVENT = "furniture-print-settings-changed";

function isDestination(value: unknown): value is PrinterDestination {
  return value === "print-window" || value === "system-default" ||
    (typeof value === "string" && value.startsWith("printer:") && value.length > 8);
}

function sanitize(settings: Partial<InvoicePrintSettings>): InvoicePrintSettings {
  return {
    printerDestination: isDestination(settings.printerDestination)
      ? settings.printerDestination
      : DEFAULT_PRINT_SETTINGS.printerDestination,
    paperSize: "a4",
    orientation: settings.orientation === "landscape" ? "landscape" : "portrait",
    marginMm: Math.max(5, Math.min(40, Number(settings.marginMm) || 15)),
    fontSize: ["small", "normal", "large"].includes(settings.fontSize ?? "")
      ? settings.fontSize as FontSize
      : DEFAULT_PRINT_SETTINGS.fontSize,
    showLogo: settings.showLogo ?? true,
    showAddress: settings.showAddress ?? true,
    showPhone: settings.showPhone ?? true,
    showPaymentDetails: settings.showPaymentDetails ?? true,
    footerText: String(settings.footerText ?? DEFAULT_PRINT_SETTINGS.footerText).slice(0, 300),
    copies: Math.max(1, Math.min(10, Number(settings.copies) || 1)),
  };
}

export function loadPrintSettings(): InvoicePrintSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_PRINT_SETTINGS };
    const parsed = JSON.parse(raw) as Partial<InvoicePrintSettings>;
    return sanitize({ ...DEFAULT_PRINT_SETTINGS, ...parsed });
  } catch {
    return { ...DEFAULT_PRINT_SETTINGS };
  }
}

export function savePrintSettings(settings: InvoicePrintSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(sanitize(settings)));
    window.dispatchEvent(new CustomEvent(PRINT_SETTINGS_CHANGED_EVENT));
  } catch {
    // localStorage not available – silently ignore
  }
}
