# POS Wireframe — New Sale

Status: **For confirmation** (see `31-phase-0-decision-record.md`)
Date: 2026-09-08

Two-panel desktop layout per `16-sales-pos-and-invoicing.md`: visual
catalogue/search on the left, current sale summary on the right. Must remain
clear at 1366×768 and scale up. Design system: `11-ui-ux-design-system.md`.

## Layout at 1366×768

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar:  ← Collapsed nav        Sales / New Sale      [🔍]  [+ Quick Add]  │
├───────────────────────────────────────────────────────────────┬──────────────┤
│ LEFT  ~ 55%                                                    │ RIGHT ~ 45%  │
│                                                                │              │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │ Search:  [Enter article / name / color / material .........]  CAT ▾     │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│  Filter chips:  [Sofas] [Beds] [Dining] [Wardrobes] [Sets]                  │
│                                                                             │
│  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐                             │
│  │  [img] │  │  [img] │  │  [img] │  │  [img] │                             │
│  │ Modern │  │ Queen  │  │ Dining │  │ 5ft    │   ▸ grid  ▸ table           │
│  │ Sofa   │  │ Bed    │  │ Table  │  │Wardrobe│                             │
│  │ ART001 │  │ ART002 │  │ ART003 │  │ ART004 │                             │
│  │PKR125k │  │PKR180k │  │ PKR75k │  │PKR98k  │                             │
│  └────────┘  └────────┘  └────────┘  └────────┘                             │
│  (click adds to sale; sets show a badge and expandable components)          │
│                                                                             │
│  1 2 3 4 5  …  page           [1] [2] [3]  ▸                                │
│                                                                             │
├──────────────────────────────────────────────────────────────────────────────┤
│ Success footer:   Checkout (Ctrl+Enter)      Fisher style subtotal preview  │
└──────────────────────────────────────────────────────────────────────────────┘
```

```text
RIGHT PANEL — CURRENT SALE                                   (after items added)
┌──────────────────────────────────────────────────────────────────────────────┐
│ Sale #240908-001   (draft)                        [Clear]  [Quotation]      │
│                                                                             │
│ Qty  Item                     Price      Disc     Total                      │
│  1   Modern Sofa  ART001    125,000      0     125,000     [x] [▾]          │
│  2   Dining Table ART003     75,500      0     151,000     [x] [▾]          │
│      └─ Modern Bedroom Set (expanded)  225,000       0    225,000 [x] [▾]   │
│                                                                             │
│  Customer:  [Ali Ahmed  ▾ ⏵ New]        phone: [0300-…]                     │
│  Location:  [Main Showroom ▾]            Delivery required [✓]              │
│                                                                             │
│  Subtotal                            501,000                                 │
│  Discount                    [-]     0                 [apply %]             │
│  Delivery charge                     0                                      │
│  Total                              PKR 501,000                              │
│                                                                             │
│  Payment:  ( ) Cash   ( ) Bank   ( ) Card   ( ) Wallet   (+) Meezan 3       │
│  Amount received:   [501,000  ]          due: 0                              │
│  [ ] Use customer advance (128,000)                                         │
│                                                                             │
│  [Checkout — Confirm Sale]        [ Reset ]                                 │
│                                                                             │
│  After confirm:  ✓ Invoice  ✓ Print  ✓ Save PDF  + Delivery note           │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Interaction notes

- **Add item:** click product card, press Enter on the search result, or scan a
  barcode into the same search field (full-screen search is the single entry).
- **Set sale:** picking a set adds one line; the expander reveals components
  read-only (from the sale-time snapshot). Availability derives from the
  limiting component.
- **Quantity/price changes:** opening row menu edits qty, or — with permission —
  custom price/discount. Backend re-validates everything on confirm.
- **Payment:** full / partial / credit / advance-funded shown as one control
  set; due preview updates live. Credit or delivery requires a named customer.
- **Confirm** is idempotent (duplicate-click safe). Success screen replaces the
  right panel with invoice actions.
- **Keyboard:** `Ctrl+Enter` checkout, `↑↓` navigate results, `Esc` cancel,
  `Tab` moves through sale fields. Targets WCAG AA.

## Image-first, not design-first

The left panel is a photo catalogue; the parts-of-trade are the product cards
and the cart totals. Colors for stock status are never the only indicator
(follow `11-ui-ux-design-system.md`).

## Files to produce in Phase 4

- `src/app/sales/new/page.tsx` — POS screen
- `src/features/sales/components/SaleCart.tsx`, `ProductPicker.tsx`,
  `PaymentPanel.tsx`
- `src/features/sales/queries.ts`, `schemas.ts` (Zod), `api.ts` (typed wrappers)
- Rust: application sale service + commands (`sales/draft_sale`,
  `sales/complete_sale`, `catalog/search_products`)