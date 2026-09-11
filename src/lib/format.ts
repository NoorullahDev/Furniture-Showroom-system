const PAISA_PER_RUPEE = 100;

/** Format integer minor units (paisa) as a PKR string, e.g. `PKR 12,345.67`. */
export function formatPkr(minor: number): string {
  return formatCurrency(minor, "PKR");
}

/** Format integer minor units using the shop's configured ISO currency code. */
export function formatCurrency(minor: number, currency = "PKR"): string {
  const negative = minor < 0;
  const abs = Math.abs(minor);
  const rupees = Math.floor(abs / PAISA_PER_RUPEE);
  const paisa = abs % PAISA_PER_RUPEE;
  return `${negative ? "-" : ""}${currency.toUpperCase()} ${rupees.toLocaleString("en-PK")}.${String(
    paisa,
  ).padStart(2, "0")}`;
}

/** Format minor units for editing inside an input (no currency prefix). */
export function formatPkrInput(minor: number): string {
  const negative = minor < 0;
  const abs = Math.abs(minor);
  const rupees = Math.floor(abs / PAISA_PER_RUPEE);
  const paisa = abs % PAISA_PER_RUPEE;
  return `${negative ? "-" : ""}${rupees.toLocaleString("en-PK")}.${String(
    paisa,
  ).padStart(2, "0")}`;
}

/** Parse user input back into minor units; returns 0 for unparseable text. */
export function parsePkrInput(text: string): number {
  const cleaned = text.replace(/[^0-9.-]/g, "");
  const negative = cleaned.startsWith("-");
  const digits = cleaned.replace(/-/g, "");
  const [whole = "0", fraction = "0"] = digits.split(".");
  const rupees = Number.parseInt(whole, 10) || 0;
  const paisa = Number.parseInt(fraction.padEnd(2, "0").slice(0, 2), 10) || 0;
  const minor = rupees * PAISA_PER_RUPEE + paisa;
  return negative ? -minor : minor;
}

/** RFC3339 timestamp from Rust (e.g. `2026-09-08T09:45:33.123Z`) → local, readable. */
export function formatDateTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(
    "en-GB",
    {
      day: "2-digit",
      month: "short",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    },
  );
}
