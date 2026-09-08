# Data Dictionary

This dictionary lists essential fields; migrations may add technical fields without weakening these rules.

## Catalogue

| Table | Important fields |
| --- | --- |
| `categories` | `id`, `name`, `parent_id`, `sort_order`, `is_active` |
| `product_types` | `id`, `category_id`, `name`, `is_active` |
| `products` | `id`, `article_number`, `name`, `category_id`, `product_type_id`, `description`, `material`, `color`, `dimensions_text`, `unit_id`, `cost_minor`, `sale_price_minor`, `minimum_stock`, `track_stock`, `is_active`, `archived_at` |
| `product_images` | `id`, `product_id`, `relative_path`, `thumbnail_path`, `sort_order`, `is_primary`, `sha256`, `width`, `height`, `mime_type` |
| `product_attributes` | `id`, `product_id`, `attribute_name`, `attribute_value`, `sort_order` |
| `locations` | `id`, `name`, `type`, `is_active` |

## Inventory and bundles

| Table | Important fields |
| --- | --- |
| `stock_movements` | `id`, `product_id`, `location_id`, `movement_type`, `quantity_delta`, `unit_cost_minor`, `reference_type`, `reference_id`, `reason`, `created_by`, `created_at`, `reversal_of_id` |
| `stock_reservations` | `id`, `sale_id`, `product_id`, `location_id`, `quantity`, `status`, `expires_at` |
| `bundles` | `id`, `code`, `name`, `default_price_minor`, `description`, `is_active` |
| `bundle_items` | `id`, `bundle_id`, `product_id`, `quantity`, `sort_order` |

## Sales and customers

| Table | Important fields |
| --- | --- |
| `customers` | `id`, `name`, `phone`, `alternate_phone`, `address`, `credit_limit_minor`, `notes`, `is_active` |
| `sales` | `id`, `invoice_number`, `customer_id`, `status`, `sale_date`, `subtotal_minor`, `discount_minor`, `tax_minor`, `delivery_charge_minor`, `total_minor`, `paid_minor`, `due_minor`, `notes`, `created_by`, `posted_at` |
| `sale_items` | `id`, `sale_id`, `product_id`, `bundle_id`, `article_snapshot`, `name_snapshot`, `quantity`, `unit_price_minor`, `discount_minor`, `total_minor`, `cost_snapshot_minor` |
| `sale_item_components` | `id`, `sale_item_id`, `product_id`, `article_snapshot`, `name_snapshot`, `quantity` |
| `customer_payments` | `id`, `receipt_number`, `customer_id`, `sale_id`, `amount_minor`, `method_id`, `account_id`, `received_at`, `reference`, `status` |
| `customer_ledger_entries` | `id`, `customer_id`, `entry_type`, `debit_minor`, `credit_minor`, `reference_type`, `reference_id`, `occurred_at`, `reversal_of_id` |

## Purchases and suppliers

| Table | Important fields |
| --- | --- |
| `suppliers` | `id`, `name`, `phone`, `address`, `opening_balance_minor`, `notes`, `is_active` |
| `purchases` | `id`, `purchase_number`, `supplier_invoice_number`, `supplier_id`, `status`, `purchase_date`, `subtotal_minor`, `discount_minor`, `tax_minor`, `freight_minor`, `total_minor`, `paid_minor`, `due_minor`, `posted_at` |
| `purchase_items` | `id`, `purchase_id`, `product_id`, `quantity`, `unit_cost_minor`, `total_minor`, `location_id` |
| `supplier_payments` | `id`, `voucher_number`, `supplier_id`, `purchase_id`, `amount_minor`, `method_id`, `account_id`, `paid_at`, `reference`, `status` |
| `supplier_ledger_entries` | `id`, `supplier_id`, `entry_type`, `debit_minor`, `credit_minor`, `reference_type`, `reference_id`, `occurred_at`, `reversal_of_id` |

## Operations and administration

| Table | Important fields |
| --- | --- |
| `deliveries` | `id`, `sale_id`, `scheduled_date`, `status`, `address`, `contact_name`, `contact_phone`, `charge_minor`, `driver_note`, `delivered_at` |
| `delivery_items` | `id`, `delivery_id`, `sale_item_id`, `quantity` |
| `expenses` | `id`, `expense_number`, `category_id`, `amount_minor`, `account_id`, `expense_date`, `payee`, `description`, `attachment_path`, `status` |
| `cash_entries` | `id`, `account_id`, `entry_type`, `amount_minor`, `reference_type`, `reference_id`, `occurred_at`, `reversal_of_id` |
| `audit_logs` | `id`, `user_id`, `action`, `entity_type`, `entity_id`, `reason`, `before_json`, `after_json`, `created_at` |
| `settings` | `key`, `value_json`, `updated_by`, `updated_at` |

## Derived values

`due_minor`, balances, and stock may be cached for fast screens, but every cached value must be updated within the same transaction and be independently rebuildable from line items, payments, ledgers, and movements.

