ALTER TABLE expenses ADD COLUMN payment_method_id INTEGER REFERENCES payment_methods(id);

CREATE INDEX IF NOT EXISTS idx_expenses_payment_method
    ON expenses(payment_method_id);
