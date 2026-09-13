-- Track below-cost sales with reasons for audit and reporting.

ALTER TABLE sales ADD COLUMN below_cost_reason TEXT;
