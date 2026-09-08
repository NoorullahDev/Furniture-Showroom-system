# Expenses, Profit, and Cash Management

## Scope

Version 1 provides operational cash and profit visibility, not a complete statutory accounting package. Reports must clearly label their basis and assumptions.

## Expense entry

Required: category, amount, date, payment account, description, and user. Optional: payee, reference, attachment, recurring tag, and product/delivery/supplier reference.

Suggested categories: rent, salaries, electricity, transport, loading/unloading, repairs, marketing, internet, office supplies, bank fees, food, and miscellaneous. Categories are user-managed.

## Expense workflow

Quick Add > Add Expense opens a focused form. On confirmation, one transaction creates the expense, cash/bank outflow, audit record, and attachment reference. Posted expenses are reversed rather than edited. Draft expenses may be edited.

## Cash accounts

Support Cash Counter, Bank, Mobile Wallet, and owner-defined accounts. Movements come from customer receipts, supplier payments, expenses, refunds, deposits/withdrawals, and controlled manual cash adjustments.

## Cash session option

The shop may enable opening/closing cash sessions:

- Opening cash.
- Expected inflows/outflows.
- Counted closing cash.
- Variance and reason.
- Manager approval for large variance.

## Profit definitions

- **Revenue:** net posted sales after discounts and returns, excluding customer payment timing.
- **COGS:** cost snapshots of net sold component quantities.
- **Gross profit:** revenue minus COGS.
- **Operational net profit:** gross profit plus delivery/service income minus recorded operating expenses and damage write-offs.
- Customer receipts are cash flow, not revenue again.
- Supplier payments are cash flow, not purchase cost again.

## Date attribution

Sales and COGS use invoice/return posting dates. Expenses use expense date. Cash flow uses actual payment dates. Reports disclose whether they use document or payment date.

## Owner withdrawals and capital

Record owner capital and withdrawals as separate cash-entry types excluded from operational profit. Do not disguise withdrawals as shop expenses.

## Controls

- Restrict delete/void, backdating, manual cash entry, and profit visibility.
- Require reason for reversals and cash adjustments.
- Warn on insufficient selected cash account balance if strict mode is enabled.
- Audit amount, account, date, category, attachment, and status changes.

## Reports

Expense detail and category summary, cash book, account balances, daily closing, gross profit, operational net profit, sales margin by item/category/set, and damage/write-off effect.

