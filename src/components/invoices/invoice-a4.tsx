import * as React from "react";
import type { SaleDto } from "@/lib/tauri/api";
/* eslint-disable @next/next/no-img-element -- print markup uses an offline logo data URL */
import type { InvoicePrintSettings } from "./invoice-settings";
import { formatPkr } from "@/lib/format";

export interface InvoiceA4Props {
  sale: SaleDto;
  shopName: string;
  shopAddress?: string | null;
  shopPhone?: string | null;
  logoDataUrl?: string | null;
  customerPhone?: string | null;
  customerAddress?: string | null;
  settings: InvoicePrintSettings;
  copies?: number;
}

function InvoiceBody({ sale, shopName, shopAddress, shopPhone, logoDataUrl, customerPhone, customerAddress, settings }: Omit<InvoiceA4Props, "copies">) {
  const invoiceNumber = sale.saleNumber ?? `#${sale.id}`;
  const balance = sale.dueMinor;
  const totalPaid = sale.paidMinor + sale.advanceUsedMinor;
  const fs = settings.fontSize === "small" ? "10pt" : settings.fontSize === "large" ? "12pt" : "11pt";
  return (
    <div className="invoice-body" style={{ fontFamily: "'Segoe UI', Arial, sans-serif", fontSize: fs, color: "#1a1a1a", lineHeight: 1.5, background: "#fff", width: "100%", boxSizing: "border-box" }}>
      <table style={{ width: "100%", borderCollapse: "collapse", marginBottom: "20px" }}>
        <tbody>
          <tr>
            <td style={{ verticalAlign: "top", width: "60%" }}>
              {settings.showLogo && (
                <div style={{ width: 56, height: 56, background: logoDataUrl ? "#fff" : "#1c4a2e", borderRadius: 8, display: "flex", alignItems: "center", justifyContent: "center", marginBottom: 8, overflow: "hidden" }}>
                  {logoDataUrl ? <img src={logoDataUrl} alt="" style={{ width: "100%", height: "100%", objectFit: "contain" }} /> : <span style={{ color: "#fff", fontWeight: 700, fontSize: "20px" }}>FS</span>}
                </div>
              )}
              <div style={{ fontWeight: 700, fontSize: "18px", color: "#1c4a2e", marginBottom: 2 }}>{shopName}</div>
              {settings.showAddress && shopAddress && <div style={{ fontSize: "9pt", color: "#555", marginTop: 2 }}>{shopAddress}</div>}
              {settings.showPhone && shopPhone && <div style={{ fontSize: "9pt", color: "#555" }}>Tel: {shopPhone}</div>}
            </td>
            <td style={{ verticalAlign: "top", textAlign: "right" }}>
              <div style={{ fontSize: "22px", fontWeight: 700, color: "#1c4a2e", marginBottom: 6 }}>INVOICE</div>
              <table style={{ borderCollapse: "collapse", marginLeft: "auto" }}>
                <tbody>
                  <tr>
                    <td style={{ fontSize: "9pt", color: "#777", paddingRight: 8 }}>Invoice No.</td>
                    <td style={{ fontSize: "9pt", fontWeight: 600, color: "#111" }}>{invoiceNumber}</td>
                  </tr>
                  <tr>
                    <td style={{ fontSize: "9pt", color: "#777", paddingRight: 8 }}>Date</td>
                    <td style={{ fontSize: "9pt", fontWeight: 600, color: "#111" }}>{sale.saleDate}</td>
                  </tr>
                  {sale.confirmedAt && (
                    <tr>
                      <td style={{ fontSize: "9pt", color: "#777", paddingRight: 8 }}>Confirmed</td>
                      <td style={{ fontSize: "9pt", color: "#111" }}>{new Date(sale.confirmedAt).toLocaleDateString("en-GB")}</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </td>
          </tr>
        </tbody>
      </table>
      <div style={{ borderTop: "2px solid #1c4a2e", marginBottom: 16 }} />
      {sale.customerName && (
        <div style={{ background: "#f5f9f6", border: "1px solid #d4e8d9", borderRadius: 6, padding: "12px 16px", marginBottom: 20 }}>
          <div style={{ fontSize: "8pt", fontWeight: 600, color: "#1c4a2e", textTransform: "uppercase" as const, letterSpacing: 0.5, marginBottom: 4 }}>Bill To</div>
          <div style={{ fontWeight: 600, fontSize: "11pt", color: "#111" }}>{sale.customerName}</div>
          {customerPhone && <div style={{ fontSize: "9pt", color: "#555", marginTop: 2 }}>Tel: {customerPhone}</div>}
          {customerAddress && <div style={{ fontSize: "9pt", color: "#555", marginTop: 2 }}>{customerAddress}</div>}
        </div>
      )}
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr style={{ background: "#1c4a2e", color: "#fff" }}>
            <th style={{ padding: "8px 10px", textAlign: "left" as const, fontSize: "9pt", fontWeight: 600, width: 28 }}>#</th>
            <th style={{ padding: "8px 10px", textAlign: "left" as const, fontSize: "9pt", fontWeight: 600 }}>Description</th>
            <th style={{ padding: "8px 10px", textAlign: "left" as const, fontSize: "9pt", fontWeight: 600 }}>Article</th>
            <th style={{ padding: "8px 10px", textAlign: "right" as const, fontSize: "9pt", fontWeight: 600, width: 44 }}>Qty</th>
            <th style={{ padding: "8px 10px", textAlign: "right" as const, fontSize: "9pt", fontWeight: 600, width: 110 }}>Unit Price</th>
            <th style={{ padding: "8px 10px", textAlign: "right" as const, fontSize: "9pt", fontWeight: 600, width: 110 }}>Amount</th>
          </tr>
        </thead>
        <tbody>
          {sale.items.map((item, idx) => (
            <tr key={item.id} style={{ borderBottom: "1px solid #e8efe9", pageBreakInside: "avoid" as const, background: idx % 2 === 0 ? "#fff" : "#f9fcfa" }}>
              <td style={{ padding: "7px 10px", fontSize: "9pt", color: "#777", verticalAlign: "top" }}>{idx + 1}</td>
              <td style={{ padding: "7px 10px", verticalAlign: "top" }}>
                <div style={{ fontWeight: 600, color: "#111" }}>{item.productName}</div>
                {item.bundleId && <div style={{ fontSize: "8pt", color: "#888", marginTop: 2 }}>Furniture Set</div>}
              </td>
              <td style={{ padding: "7px 10px", fontSize: "9pt", color: "#666", verticalAlign: "top" }}>{item.articleNumber}</td>
              <td style={{ padding: "7px 10px", textAlign: "right" as const, fontSize: "9pt", verticalAlign: "top" }}>{item.quantity}</td>
              <td style={{ padding: "7px 10px", textAlign: "right" as const, fontSize: "9pt", verticalAlign: "top" }}>{formatPkr(item.unitPriceMinor)}</td>
              <td style={{ padding: "7px 10px", textAlign: "right" as const, fontSize: "9pt", fontWeight: 600, verticalAlign: "top" }}>{formatPkr(item.lineTotalMinor)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <td colSpan={6} style={{ paddingTop: 16 }}>
              <table style={{ width: "50%", borderCollapse: "collapse", marginLeft: "auto" }}>
                <tbody>
                  <tr>
                    <td style={{ padding: "3px 10px", fontSize: "9pt", color: "#555" }}>Subtotal</td>
                    <td style={{ padding: "3px 10px", textAlign: "right" as const, fontSize: "9pt" }}>{formatPkr(sale.subtotalMinor)}</td>
                  </tr>
                  {sale.discountMinor > 0 && (
                    <tr>
                      <td style={{ padding: "3px 10px", fontSize: "9pt", color: "#555" }}>Discount</td>
                      <td style={{ padding: "3px 10px", textAlign: "right" as const, fontSize: "9pt", color: "#d32f2f" }}>- {formatPkr(sale.discountMinor)}</td>
                    </tr>
                  )}
                  {sale.deliveryChargeMinor > 0 && (
                    <tr>
                      <td style={{ padding: "3px 10px", fontSize: "9pt", color: "#555" }}>Delivery charge</td>
                      <td style={{ padding: "3px 10px", textAlign: "right" as const, fontSize: "9pt" }}>{formatPkr(sale.deliveryChargeMinor)}</td>
                    </tr>
                  )}
                  <tr style={{ borderTop: "2px solid #1c4a2e" }}>
                    <td style={{ padding: "6px 10px", fontSize: "11pt", fontWeight: 700, color: "#1c4a2e" }}>Total</td>
                    <td style={{ padding: "6px 10px", textAlign: "right" as const, fontSize: "11pt", fontWeight: 700, color: "#1c4a2e" }}>{formatPkr(sale.totalMinor)}</td>
                  </tr>
                  <tr>
                    <td style={{ padding: "3px 10px", fontSize: "9pt", color: "#555" }}>Amount paid</td>
                    <td style={{ padding: "3px 10px", textAlign: "right" as const, fontSize: "9pt", color: "#2e7d32" }}>{formatPkr(sale.paidMinor)}</td>
                  </tr>
                  {sale.advanceUsedMinor > 0 && (
                    <tr>
                      <td style={{ padding: "3px 10px", fontSize: "9pt", color: "#555" }}>Advance used</td>
                      <td style={{ padding: "3px 10px", textAlign: "right" as const, fontSize: "9pt", color: "#2e7d32" }}>{formatPkr(sale.advanceUsedMinor)}</td>
                    </tr>
                  )}
                  <tr style={{ background: balance > 0 ? "#fff8f0" : "#f0faf2", borderTop: "2px solid " + (balance > 0 ? "#e65100" : "#2e7d32") }}>
                    <td style={{ padding: "6px 10px", fontSize: "11pt", fontWeight: 700, color: balance > 0 ? "#e65100" : "#2e7d32" }}>{balance > 0 ? "Balance due" : "Fully paid"}</td>
                    <td style={{ padding: "6px 10px", textAlign: "right" as const, fontSize: "11pt", fontWeight: 700, color: balance > 0 ? "#e65100" : "#2e7d32" }}>{formatPkr(Math.abs(balance))}</td>
                  </tr>
                </tbody>
              </table>
            </td>
          </tr>
          {settings.showPaymentDetails && totalPaid > 0 && (
            <tr>
              <td colSpan={6} style={{ paddingTop: 16 }}>
                <div style={{ background: "#f5f9f6", border: "1px solid #d4e8d9", borderRadius: 6, padding: "10px 14px" }}>
                  <div style={{ fontSize: "8pt", fontWeight: 600, color: "#1c4a2e", textTransform: "uppercase" as const, letterSpacing: 0.5, marginBottom: 4 }}>Payment summary</div>
                  <div style={{ fontSize: "9pt" }}>
                    <span style={{ color: "#555" }}>Cash received: </span>
                    <span style={{ fontWeight: 600 }}>{formatPkr(sale.paidMinor)}</span>
                    {sale.advanceUsedMinor > 0 && (
                      <React.Fragment>
                        <span style={{ color: "#aaa", margin: "0 6px" }}>|</span>
                        <span style={{ color: "#555" }}>Advance applied: </span>
                        <span style={{ fontWeight: 600 }}>{formatPkr(sale.advanceUsedMinor)}</span>
                      </React.Fragment>
                    )}
                  </div>
                </div>
              </td>
            </tr>
          )}
          {sale.notes && (
            <tr>
              <td colSpan={6} style={{ paddingTop: 12, fontSize: "9pt", color: "#555" }}>
                <strong>Notes: </strong>{sale.notes}
              </td>
            </tr>
          )}
          {settings.footerText && (
            <tr>
              <td colSpan={6} style={{ paddingTop: 20, borderTop: "1px solid #d4e8d9" }}>
                <div style={{ fontSize: "9pt", color: "#777", textAlign: "center" as const }}>{settings.footerText}</div>
              </td>
            </tr>
          )}
          <tr>
            <td colSpan={6} style={{ paddingTop: 40 }}>
              <table style={{ width: "100%", borderCollapse: "collapse" }}>
                <tbody>
                  <tr>
                    <td style={{ width: "50%", verticalAlign: "bottom" }}>
                      <div style={{ borderTop: "1px solid #999", width: 200, margin: "0 auto 4px auto" }} />
                      <div style={{ fontSize: "8pt", color: "#777", textAlign: "center" as const }}>Authorized Signature</div>
                    </td>
                    <td style={{ width: "50%", verticalAlign: "bottom" }}>
                      <div style={{ borderTop: "1px solid #999", width: 200, margin: "0 auto 4px auto" }} />
                      <div style={{ fontSize: "8pt", color: "#777", textAlign: "center" as const }}>Customer Signature</div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

export function InvoiceA4({ sale, shopName, shopAddress, shopPhone, logoDataUrl, customerPhone, customerAddress, settings, copies = 1 }: InvoiceA4Props) {
  const safeCopies = Math.max(1, Math.min(copies, 10));
  const marginPx = Math.max(8, (settings.marginMm ?? 15) * 3.78);
  const bodyProps = { sale, shopName, shopAddress, shopPhone, logoDataUrl, customerPhone, customerAddress, settings };
  return (
    <div id="invoice-a4-root" style={{ background: "#fff", padding: marginPx, boxSizing: "border-box" as const }}>
      {Array.from({ length: safeCopies }).map((_, i) => (
        <React.Fragment key={i}>
          {i > 0 && <div style={{ pageBreakAfter: "always", breakAfter: "page", height: 0, margin: 0 }} aria-hidden />}
          <InvoiceBody {...bodyProps} />
        </React.Fragment>
      ))}
    </div>
  );
}
