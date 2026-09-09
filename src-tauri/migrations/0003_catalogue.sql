-- Furniture Shop Management System — Phase 3: product catalogue.
-- Categories, product types, units, products, product images, and attributes.
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.

CREATE TABLE IF NOT EXISTS categories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL COLLATE NOCASE,
    parent_id   INTEGER REFERENCES categories(id),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);

CREATE TABLE IF NOT EXISTS product_types (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL REFERENCES categories(id),
    name        TEXT NOT NULL COLLATE NOCASE,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (category_id, name)
);

CREATE INDEX IF NOT EXISTS idx_product_types_category ON product_types(category_id);

CREATE TABLE IF NOT EXISTS units (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    code        TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS products (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    article_number      TEXT NOT NULL,
    article_number_norm TEXT NOT NULL,
    name                TEXT NOT NULL,
    category_id         INTEGER NOT NULL REFERENCES categories(id),
    product_type_id     INTEGER REFERENCES product_types(id),
    unit_id             INTEGER REFERENCES units(id),
    description         TEXT,
    material            TEXT,
    color               TEXT,
    dimensions_text     TEXT,
    brand               TEXT,
    barcode             TEXT,
    warranty_months     INTEGER,
    notes               TEXT,
    cost_minor          INTEGER NOT NULL DEFAULT 0 CHECK (cost_minor >= 0),
    sale_price_minor    INTEGER NOT NULL DEFAULT 0 CHECK (sale_price_minor >= 0),
    minimum_stock       INTEGER NOT NULL DEFAULT 0 CHECK (minimum_stock >= 0),
    track_stock         INTEGER NOT NULL DEFAULT 1,
    is_active           INTEGER NOT NULL DEFAULT 1,
    archived_at         TEXT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Article numbers must be unique among live (non-archived) products,
-- case- and whitespace-insensitively. Archived products keep their numbers.
CREATE UNIQUE INDEX IF NOT EXISTS uq_products_article_live
    ON products(article_number_norm) WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_products_name ON products(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_products_category ON products(category_id);
CREATE INDEX IF NOT EXISTS idx_products_type ON products(product_type_id);
CREATE INDEX IF NOT EXISTS idx_products_archived ON products(archived_at);

CREATE TABLE IF NOT EXISTS product_images (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    relative_path   TEXT NOT NULL,
    thumbnail_path  TEXT NOT NULL,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    is_primary      INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
    sha256          TEXT NOT NULL,
    width           INTEGER,
    height          INTEGER,
    mime_type       TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_product_images_product ON product_images(product_id);

CREATE TABLE IF NOT EXISTS product_attributes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id      INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    attribute_name  TEXT NOT NULL,
    attribute_value TEXT NOT NULL,
    sort_order      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_product_attributes_product ON product_attributes(product_id);

-- Catalogue master-data seeds. Products, categories, and types are user data.
INSERT OR IGNORE INTO units (name, code) VALUES
  ('Pcs', 'pcs'),
  ('Pair', 'pr'),
  ('Set', 'set'),
  ('Dozen', 'dz'),
  ('Meter', 'm');