# UI and UX Design System

## Design direction

Use a light, calm, professional retail interface. It should look intentionally designed—not like a collection of generic dashboard cards. Product photography supplies visual richness; the surrounding interface stays restrained.

## Visual foundation

- Background: warm off-white such as `#F7F7F5`.
- Surfaces: white with subtle neutral borders.
- Primary: deep forest/teal, used for main actions and selected navigation.
- Accent: warm amber used sparingly for attention.
- Danger: muted red; success: accessible green; warning: amber.
- Use one modern sans-serif family and tabular numerals for money.
- Border radius: moderate and consistent; avoid excessive pills.
- Shadows: subtle, mainly for overlays—not every card.

## Layout

- Fixed collapsible left navigation on desktop.
- Compact top bar with page context, search, Quick Add, notifications, and user menu.
- Content width uses available desktop space with 24–32 px gutters.
- Dense tables for operational data; spacious forms for entry.
- Preserve primary action placement at the upper right.

## Typography

- Page title: 24–28 px semibold.
- Section heading: 18–20 px semibold.
- Body: 14–16 px.
- Table: 13–14 px.
- Use sentence case, not all caps, except short invoice labels.
- Format PKR consistently, for example `PKR 125,000`.

## Interaction standards

- Minimum interactive target: 40 × 40 px.
- Clear keyboard focus ring on every control.
- `Ctrl+K` focuses global search; `Ctrl+N` opens context-aware create; `Ctrl+Shift+A` opens Quick Add.
- Enter submits only when safe; destructive actions require explicit confirmation.
- Toasts confirm short actions; important results remain visible in the page.

## Data tables

- Sticky header, optional sticky key column, visible row hover.
- Server/backend pagination for large datasets.
- Filters summarized in a removable filter bar.
- Column chooser saved per user.
- Amount columns right-aligned; statuses and actions predictable.
- Row menu contains secondary actions; primary row action may remain visible.

## Product presentation

- Support grid and table modes.
- Image-first cards show primary photo, name, article number, price, and stock status.
- Never rely on color alone for stock or status.
- Missing photos use a neutral furniture placeholder and text.

## Accessibility

- Target WCAG AA contrast.
- All dialogs trap focus and restore it on close.
- Inputs have real labels, descriptions, and associated errors.
- Icons include tooltips or accessible labels.
- Provide reduced-motion behavior.

## Empty and error states

Every screen includes loading, empty, filtered-empty, error, and permission-denied designs. Empty states explain the benefit and offer exactly one sensible next action.

