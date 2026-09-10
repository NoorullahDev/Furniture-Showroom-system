-- Track which cash account a walk-in (no-customer) sale was paid from so that
-- cancelling the sale can refund the money. Customer sales already record the
-- account on their customer_payments row instead.
ALTER TABLE sales ADD COLUMN payment_cash_account_id INTEGER REFERENCES cash_accounts(id);