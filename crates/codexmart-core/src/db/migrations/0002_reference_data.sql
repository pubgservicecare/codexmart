-- CODE X MART schema v2: reference data every installation starts with.
-- Seeded rows use fixed identifiers so that every device (and a future
-- central cloud database) agrees on them.

INSERT INTO sequences (name, prefix, next_value, padding) VALUES
  ('sale',             'INV-',  1, 6),
  ('sale_return',      'SR-',   1, 6),
  ('purchase',         'GRN-',  1, 6),
  ('purchase_order',   'PO-',   1, 6),
  ('purchase_return',  'PR-',   1, 6),
  ('supplier_payment', 'SP-',   1, 6),
  ('customer_payment', 'RCPT-', 1, 6),
  ('expense',          'EXP-',  1, 6),
  ('adjustment',       'ADJ-',  1, 6),
  ('cash_closing',     'CC-',   1, 6),
  ('sku',              'CXM-',  1, 5),
  ('internal_barcode', '',      1, 10);

INSERT INTO units (id, name, short_name, allow_decimal, is_active, created_at, updated_at) VALUES
  ('00000000-0000-7000-8000-000000000101', 'Piece',      'pcs',  0, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000102', 'Pack',       'pack', 0, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000103', 'Box',        'box',  0, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000104', 'Dozen',      'dz',   0, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000105', 'Bottle',     'btl',  0, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000106', 'Kilogram',   'kg',   1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000107', 'Gram',       'g',    1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000108', 'Litre',      'L',    1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000109', 'Millilitre', 'ml',   1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000110', 'Metre',      'm',    1, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'));

INSERT INTO categories (id, name, is_active, created_at, updated_at) VALUES
  ('00000000-0000-7000-8000-000000000201', 'Skincare & Creams',   1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000202', 'Makeup & Cosmetics',  1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000203', 'Bags',                1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000204', 'General Mart',        1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000205', 'Other',               1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'));

INSERT INTO expense_categories (id, name, is_active, created_at, updated_at) VALUES
  ('00000000-0000-7000-8000-000000000301', 'Rent',          1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000302', 'Electricity',   1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000303', 'Salaries',      1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000304', 'Transport',     1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000305', 'Utilities',     1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000306', 'Maintenance',   1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  ('00000000-0000-7000-8000-000000000307', 'Miscellaneous', 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'));
