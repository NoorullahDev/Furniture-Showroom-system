-- Operate as one showroom while preserving every inventory and business row.
-- The canonical location is the one already matching shop.name when possible,
-- otherwise the first active showroom. Stock caches are aggregated before all
-- location-linked rows are reassigned, so quantities and FIFO layers survive.

INSERT INTO locations (name, type, is_active)
SELECT 'Main Showroom', 'showroom', 1
WHERE NOT EXISTS (SELECT 1 FROM locations);

CREATE TEMP TABLE _single_showroom (
    id           INTEGER PRIMARY KEY,
    desired_name TEXT NOT NULL
);

INSERT INTO _single_showroom (id, desired_name)
WITH desired AS (
    SELECT COALESCE(
        NULLIF(TRIM(json_extract(
            (SELECT value_json FROM settings WHERE key = 'shop.name'), '$'
        )), ''),
        (SELECT name FROM locations WHERE type = 'showroom' ORDER BY is_active DESC, id LIMIT 1),
        'Furniture Showroom'
    ) AS name
)
SELECT
    COALESCE(
        (SELECT id FROM locations, desired WHERE locations.name = desired.name ORDER BY is_active DESC, id LIMIT 1),
        (SELECT id FROM locations WHERE is_active = 1 AND type = 'showroom' ORDER BY id LIMIT 1),
        (SELECT id FROM locations ORDER BY id LIMIT 1)
    ),
    desired.name
FROM desired;

CREATE TEMP TABLE _single_showroom_balances AS
SELECT
    product_id,
    (SELECT id FROM _single_showroom) AS location_id,
    SUM(on_hand) AS on_hand,
    SUM(reserved) AS reserved,
    SUM(damaged) AS damaged
FROM stock_balances
GROUP BY product_id;

DELETE FROM stock_balances;
INSERT INTO stock_balances (product_id, location_id, on_hand, reserved, damaged)
SELECT product_id, location_id, on_hand, reserved, damaged
FROM _single_showroom_balances;

UPDATE stock_movements
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE stock_reservations
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE inventory_count_sessions
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE purchases
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE supplier_returns
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE sales
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE deliveries
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE sales_returns
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

UPDATE damage_records
SET location_id = (SELECT id FROM _single_showroom)
WHERE location_id <> (SELECT id FROM _single_showroom);

-- Free the desired name if a retired duplicate owns it, then rename the one
-- canonical row. IDs remain stable and old location rows remain available for
-- referential/audit purposes, but cannot be selected for new work.
UPDATE locations
SET name = '__retired_location_' || id,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id <> (SELECT id FROM _single_showroom)
  AND name = (SELECT desired_name FROM _single_showroom);

UPDATE locations
SET name = (SELECT desired_name FROM _single_showroom),
    type = 'showroom',
    is_active = 1,
    move_num_seq = (SELECT MAX(move_num_seq) FROM locations),
    session_num_seq = (SELECT MAX(session_num_seq) FROM locations),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = (SELECT id FROM _single_showroom);

UPDATE locations
SET is_active = 0,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id <> (SELECT id FROM _single_showroom);

DROP TABLE _single_showroom_balances;
DROP TABLE _single_showroom;
