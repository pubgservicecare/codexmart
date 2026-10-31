-- CODE X MART schema v1
-- Conventions:
--   * Primary keys are UUIDv7 strings (stable, globally unique, time ordered).
--   * Money columns are INTEGER minor units (paisa). Quantities are INTEGER
--     thousandths of a unit (1 piece = 1000).
--   * Timestamps are UTC RFC 3339 strings; business_date is the local
--     calendar date (YYYY-MM-DD) the transaction belongs to.
--   * Financial documents are append-only. Corrections are made with new
--     documents (returns, voids, adjustments), never by editing history.

CREATE TABLE app_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE sequences (
  name       TEXT PRIMARY KEY,
  prefix     TEXT NOT NULL,
  next_value INTEGER NOT NULL CHECK (next_value >= 1),
  padding    INTEGER NOT NULL DEFAULT 6
);

-- ---------------------------------------------------------------- users
CREATE TABLE users (
  id              TEXT PRIMARY KEY,
  username        TEXT NOT NULL COLLATE NOCASE UNIQUE,
  full_name       TEXT NOT NULL,
  role            TEXT NOT NULL CHECK (role IN ('admin','manager','cashier')),
  password_hash   TEXT NOT NULL,
  is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  failed_attempts INTEGER NOT NULL DEFAULT 0,
  locked_until    TEXT,
  last_login_at   TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- -------------------------------------------------------------- catalog
CREATE TABLE categories (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL COLLATE NOCASE UNIQUE,
  is_active  INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE brands (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL COLLATE NOCASE UNIQUE,
  is_active  INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE units (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL COLLATE NOCASE UNIQUE,
  short_name    TEXT NOT NULL,
  allow_decimal INTEGER NOT NULL DEFAULT 0 CHECK (allow_decimal IN (0,1)),
  is_active     INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

CREATE TABLE products (
  id               TEXT PRIMARY KEY,
  name             TEXT NOT NULL,
  sku              TEXT NOT NULL COLLATE NOCASE UNIQUE,
  barcode          TEXT,
  category_id      TEXT REFERENCES categories(id),
  brand_id         TEXT REFERENCES brands(id),
  unit_id          TEXT NOT NULL REFERENCES units(id),
  variant_group_id TEXT,
  size             TEXT,
  color            TEXT,
  description      TEXT,
  sale_price       INTEGER NOT NULL CHECK (sale_price >= 0),
  wholesale_price  INTEGER CHECK (wholesale_price IS NULL OR wholesale_price >= 0),
  cost_price       INTEGER NOT NULL DEFAULT 0 CHECK (cost_price >= 0),
  stock_qty        INTEGER NOT NULL DEFAULT 0,
  stock_value      INTEGER NOT NULL DEFAULT 0 CHECK (stock_value >= 0),
  reorder_level    INTEGER NOT NULL DEFAULT 0 CHECK (reorder_level >= 0),
  track_batches    INTEGER NOT NULL DEFAULT 0 CHECK (track_batches IN (0,1)),
  image_ref        TEXT,
  is_active        INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX ux_products_barcode ON products(barcode) WHERE barcode IS NOT NULL;
CREATE INDEX ix_products_name ON products(name COLLATE NOCASE);
CREATE INDEX ix_products_category ON products(category_id);
CREATE INDEX ix_products_brand ON products(brand_id);
CREATE INDEX ix_products_variant_group ON products(variant_group_id);

-- ------------------------------------------------------------ inventory
CREATE TABLE stock_batches (
  id            TEXT PRIMARY KEY,
  product_id    TEXT NOT NULL REFERENCES products(id),
  batch_no      TEXT,
  expiry_date   TEXT,
  qty_received  INTEGER NOT NULL,
  qty_remaining INTEGER NOT NULL CHECK (qty_remaining >= 0),
  unit_cost     INTEGER NOT NULL,
  source_type   TEXT NOT NULL,
  source_id     TEXT,
  received_date TEXT NOT NULL,
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);
CREATE INDEX ix_batches_product ON stock_batches(product_id, expiry_date);
CREATE INDEX ix_batches_expiry ON stock_batches(expiry_date) WHERE qty_remaining > 0;

CREATE TABLE stock_movements (
  id            TEXT PRIMARY KEY,
  product_id    TEXT NOT NULL REFERENCES products(id),
  batch_id      TEXT REFERENCES stock_batches(id),
  movement_type TEXT NOT NULL CHECK (movement_type IN
                  ('opening','purchase','purchase_return','sale','sale_return',
                   'sale_void','adjustment_in','adjustment_out')),
  qty_change    INTEGER NOT NULL,
  value_change  INTEGER NOT NULL,
  qty_after     INTEGER NOT NULL,
  value_after   INTEGER NOT NULL,
  ref_type      TEXT,
  ref_id        TEXT,
  ref_no        TEXT,
  ref_line_id   TEXT,
  reason        TEXT,
  note          TEXT,
  user_id       TEXT REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_movements_product ON stock_movements(product_id, created_at);
CREATE INDEX ix_movements_date ON stock_movements(business_date);
CREATE INDEX ix_movements_ref ON stock_movements(ref_type, ref_id);
CREATE INDEX ix_movements_line ON stock_movements(ref_line_id);

CREATE TABLE stock_adjustments (
  id            TEXT PRIMARY KEY,
  adjustment_no TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  reason        TEXT NOT NULL CHECK (reason IN
                  ('opening_stock','stock_count','found','damaged','expired',
                   'lost','theft','internal_use','other')),
  note          TEXT,
  user_id       TEXT NOT NULL REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_adjustments_date ON stock_adjustments(business_date);

CREATE TABLE stock_adjustment_items (
  id            TEXT PRIMARY KEY,
  adjustment_id TEXT NOT NULL REFERENCES stock_adjustments(id),
  product_id    TEXT NOT NULL REFERENCES products(id),
  batch_id      TEXT REFERENCES stock_batches(id),
  qty_change    INTEGER NOT NULL CHECK (qty_change <> 0),
  unit_cost     INTEGER NOT NULL,
  value_change  INTEGER NOT NULL
);
CREATE INDEX ix_adjustment_items_adj ON stock_adjustment_items(adjustment_id);

-- ------------------------------------------------------------ customers
CREATE TABLE customers (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  phone           TEXT,
  email           TEXT,
  address         TEXT,
  opening_balance INTEGER NOT NULL DEFAULT 0,
  credit_limit    INTEGER CHECK (credit_limit IS NULL OR credit_limit >= 0),
  notes           TEXT,
  is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX ix_customers_name ON customers(name COLLATE NOCASE);
CREATE INDEX ix_customers_phone ON customers(phone);

-- ---------------------------------------------------------------- sales
CREATE TABLE sales (
  id                  TEXT PRIMARY KEY,
  invoice_no          TEXT NOT NULL UNIQUE,
  client_txn_id       TEXT NOT NULL UNIQUE,
  status              TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('completed','void')),
  customer_id         TEXT REFERENCES customers(id),
  price_tier          TEXT NOT NULL DEFAULT 'retail' CHECK (price_tier IN ('retail','wholesale')),
  item_count          INTEGER NOT NULL,
  total_qty           INTEGER NOT NULL,
  subtotal            INTEGER NOT NULL,
  line_discount_total INTEGER NOT NULL,
  bill_discount       INTEGER NOT NULL,
  total               INTEGER NOT NULL CHECK (total >= 0),
  paid_amount         INTEGER NOT NULL,
  tendered_cash       INTEGER NOT NULL DEFAULT 0,
  change_amount       INTEGER NOT NULL DEFAULT 0,
  credit_amount       INTEGER NOT NULL DEFAULT 0 CHECK (credit_amount >= 0),
  cogs_total          INTEGER NOT NULL,
  note                TEXT,
  cashier_id          TEXT NOT NULL REFERENCES users(id),
  business_date       TEXT NOT NULL,
  created_at          TEXT NOT NULL,
  voided_at           TEXT,
  voided_by           TEXT REFERENCES users(id),
  void_reason         TEXT
);
CREATE INDEX ix_sales_date ON sales(business_date, status);
CREATE INDEX ix_sales_customer ON sales(customer_id);
CREATE INDEX ix_sales_created ON sales(created_at);

CREATE TABLE sale_items (
  id                  TEXT PRIMARY KEY,
  sale_id             TEXT NOT NULL REFERENCES sales(id),
  line_no             INTEGER NOT NULL,
  product_id          TEXT NOT NULL REFERENCES products(id),
  product_name        TEXT NOT NULL,
  sku                 TEXT NOT NULL,
  barcode             TEXT,
  unit_name           TEXT NOT NULL,
  qty                 INTEGER NOT NULL CHECK (qty > 0),
  unit_price          INTEGER NOT NULL CHECK (unit_price >= 0),
  gross_amount        INTEGER NOT NULL,
  line_discount       INTEGER NOT NULL,
  bill_discount_share INTEGER NOT NULL,
  net_amount          INTEGER NOT NULL,
  cost_amount         INTEGER NOT NULL,
  UNIQUE (sale_id, line_no)
);
CREATE INDEX ix_sale_items_product ON sale_items(product_id);

CREATE TABLE sale_payments (
  id         TEXT PRIMARY KEY,
  sale_id    TEXT NOT NULL REFERENCES sales(id),
  method     TEXT NOT NULL CHECK (method IN ('cash','card','bank','wallet','cheque')),
  amount     INTEGER NOT NULL CHECK (amount > 0),
  reference  TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX ix_sale_payments_sale ON sale_payments(sale_id);

CREATE TABLE sale_returns (
  id            TEXT PRIMARY KEY,
  return_no     TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  sale_id       TEXT NOT NULL REFERENCES sales(id),
  customer_id   TEXT REFERENCES customers(id),
  refund_method TEXT NOT NULL CHECK (refund_method IN ('cash','card','bank','wallet','cheque','account')),
  total_refund  INTEGER NOT NULL CHECK (total_refund >= 0),
  cost_reversed INTEGER NOT NULL,
  reason        TEXT,
  user_id       TEXT NOT NULL REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_sale_returns_sale ON sale_returns(sale_id);
CREATE INDEX ix_sale_returns_date ON sale_returns(business_date);

CREATE TABLE sale_return_items (
  id            TEXT PRIMARY KEY,
  return_id     TEXT NOT NULL REFERENCES sale_returns(id),
  sale_item_id  TEXT NOT NULL REFERENCES sale_items(id),
  product_id    TEXT NOT NULL REFERENCES products(id),
  qty           INTEGER NOT NULL CHECK (qty > 0),
  refund_amount INTEGER NOT NULL,
  restock       INTEGER NOT NULL CHECK (restock IN (0,1)),
  cost_amount   INTEGER NOT NULL
);
CREATE INDEX ix_sale_return_items_return ON sale_return_items(return_id);
CREATE INDEX ix_sale_return_items_item ON sale_return_items(sale_item_id);

CREATE TABLE customer_payments (
  id            TEXT PRIMARY KEY,
  receipt_no    TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  customer_id   TEXT NOT NULL REFERENCES customers(id),
  amount        INTEGER NOT NULL CHECK (amount > 0),
  method        TEXT NOT NULL CHECK (method IN ('cash','card','bank','wallet','cheque')),
  note          TEXT,
  user_id       TEXT NOT NULL REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_customer_payments_customer ON customer_payments(customer_id);
CREATE INDEX ix_customer_payments_date ON customer_payments(business_date);

-- ----------------------------------------------------------- purchasing
CREATE TABLE suppliers (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  contact_person  TEXT,
  phone           TEXT,
  email           TEXT,
  address         TEXT,
  opening_balance INTEGER NOT NULL DEFAULT 0,
  notes           TEXT,
  is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX ix_suppliers_name ON suppliers(name COLLATE NOCASE);

CREATE TABLE purchase_orders (
  id             TEXT PRIMARY KEY,
  po_no          TEXT NOT NULL UNIQUE,
  supplier_id    TEXT NOT NULL REFERENCES suppliers(id),
  status         TEXT NOT NULL CHECK (status IN ('draft','ordered','partial','received','cancelled')),
  order_date     TEXT NOT NULL,
  expected_date  TEXT,
  note           TEXT,
  total_estimate INTEGER NOT NULL DEFAULT 0,
  created_by     TEXT NOT NULL REFERENCES users(id),
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
CREATE INDEX ix_po_supplier ON purchase_orders(supplier_id);

CREATE TABLE purchase_order_items (
  id           TEXT PRIMARY KEY,
  po_id        TEXT NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
  line_no      INTEGER NOT NULL,
  product_id   TEXT NOT NULL REFERENCES products(id),
  qty_ordered  INTEGER NOT NULL CHECK (qty_ordered > 0),
  qty_received INTEGER NOT NULL DEFAULT 0 CHECK (qty_received >= 0),
  unit_cost    INTEGER NOT NULL CHECK (unit_cost >= 0)
);
CREATE INDEX ix_po_items_po ON purchase_order_items(po_id);

CREATE TABLE purchases (
  id                  TEXT PRIMARY KEY,
  purchase_no         TEXT NOT NULL UNIQUE,
  client_txn_id       TEXT NOT NULL UNIQUE,
  supplier_id         TEXT NOT NULL REFERENCES suppliers(id),
  supplier_invoice_no TEXT,
  po_id               TEXT REFERENCES purchase_orders(id),
  subtotal            INTEGER NOT NULL,
  discount            INTEGER NOT NULL DEFAULT 0 CHECK (discount >= 0),
  total               INTEGER NOT NULL CHECK (total >= 0),
  note                TEXT,
  user_id             TEXT NOT NULL REFERENCES users(id),
  business_date       TEXT NOT NULL,
  created_at          TEXT NOT NULL
);
CREATE UNIQUE INDEX ux_purchases_supplier_invoice
  ON purchases(supplier_id, supplier_invoice_no COLLATE NOCASE)
  WHERE supplier_invoice_no IS NOT NULL;
CREATE INDEX ix_purchases_date ON purchases(business_date);
CREATE INDEX ix_purchases_supplier ON purchases(supplier_id);

CREATE TABLE purchase_items (
  id             TEXT PRIMARY KEY,
  purchase_id    TEXT NOT NULL REFERENCES purchases(id),
  line_no        INTEGER NOT NULL,
  product_id     TEXT NOT NULL REFERENCES products(id),
  qty            INTEGER NOT NULL CHECK (qty > 0),
  unit_cost      INTEGER NOT NULL CHECK (unit_cost >= 0),
  line_total     INTEGER NOT NULL,
  discount_share INTEGER NOT NULL DEFAULT 0,
  net_cost       INTEGER NOT NULL,
  batch_id       TEXT REFERENCES stock_batches(id),
  batch_no       TEXT,
  expiry_date    TEXT,
  po_item_id     TEXT REFERENCES purchase_order_items(id)
);
CREATE INDEX ix_purchase_items_purchase ON purchase_items(purchase_id);
CREATE INDEX ix_purchase_items_product ON purchase_items(product_id);

CREATE TABLE supplier_payments (
  id            TEXT PRIMARY KEY,
  payment_no    TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  supplier_id   TEXT NOT NULL REFERENCES suppliers(id),
  purchase_id   TEXT REFERENCES purchases(id),
  amount        INTEGER NOT NULL CHECK (amount > 0),
  method        TEXT NOT NULL CHECK (method IN ('cash','card','bank','wallet','cheque')),
  reference     TEXT,
  note          TEXT,
  user_id       TEXT NOT NULL REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_supplier_payments_supplier ON supplier_payments(supplier_id);
CREATE INDEX ix_supplier_payments_date ON supplier_payments(business_date);

CREATE TABLE purchase_returns (
  id            TEXT PRIMARY KEY,
  return_no     TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  supplier_id   TEXT NOT NULL REFERENCES suppliers(id),
  purchase_id   TEXT NOT NULL REFERENCES purchases(id),
  total         INTEGER NOT NULL CHECK (total >= 0),
  reason        TEXT,
  user_id       TEXT NOT NULL REFERENCES users(id),
  business_date TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_purchase_returns_supplier ON purchase_returns(supplier_id);
CREATE INDEX ix_purchase_returns_date ON purchase_returns(business_date);

CREATE TABLE purchase_return_items (
  id               TEXT PRIMARY KEY,
  return_id        TEXT NOT NULL REFERENCES purchase_returns(id),
  purchase_item_id TEXT NOT NULL REFERENCES purchase_items(id),
  product_id       TEXT NOT NULL REFERENCES products(id),
  batch_id         TEXT REFERENCES stock_batches(id),
  qty              INTEGER NOT NULL CHECK (qty > 0),
  line_total       INTEGER NOT NULL
);
CREATE INDEX ix_purchase_return_items_return ON purchase_return_items(return_id);
CREATE INDEX ix_purchase_return_items_item ON purchase_return_items(purchase_item_id);

-- ------------------------------------------------------------- expenses
CREATE TABLE expense_categories (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL COLLATE NOCASE UNIQUE,
  is_active  INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE expenses (
  id            TEXT PRIMARY KEY,
  expense_no    TEXT NOT NULL UNIQUE,
  client_txn_id TEXT NOT NULL UNIQUE,
  category_id   TEXT NOT NULL REFERENCES expense_categories(id),
  amount        INTEGER NOT NULL CHECK (amount > 0),
  method        TEXT NOT NULL CHECK (method IN ('cash','card','bank','wallet','cheque')),
  payee         TEXT,
  note          TEXT,
  business_date TEXT NOT NULL,
  status        TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','void')),
  void_reason   TEXT,
  voided_at     TEXT,
  voided_by     TEXT REFERENCES users(id),
  user_id       TEXT NOT NULL REFERENCES users(id),
  created_at    TEXT NOT NULL
);
CREATE INDEX ix_expenses_date ON expenses(business_date, status);
CREATE INDEX ix_expenses_category ON expenses(category_id);

-- ---------------------------------------------------------- cash closing
CREATE TABLE cash_closings (
  id                     TEXT PRIMARY KEY,
  closing_no             TEXT NOT NULL UNIQUE,
  business_date          TEXT NOT NULL UNIQUE,
  opening_float          INTEGER NOT NULL,
  cash_sales             INTEGER NOT NULL,
  cash_customer_receipts INTEGER NOT NULL,
  cash_refunds           INTEGER NOT NULL,
  cash_expenses          INTEGER NOT NULL,
  cash_supplier_payments INTEGER NOT NULL,
  expected_cash          INTEGER NOT NULL,
  counted_cash           INTEGER NOT NULL,
  difference             INTEGER NOT NULL,
  note                   TEXT,
  user_id                TEXT NOT NULL REFERENCES users(id),
  created_at             TEXT NOT NULL
);

-- ----------------------------------------------------------------- audit
CREATE TABLE audit_log (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  at          TEXT NOT NULL,
  user_id     TEXT,
  username    TEXT,
  action      TEXT NOT NULL,
  entity_type TEXT,
  entity_id   TEXT,
  summary     TEXT NOT NULL,
  details     TEXT
);
CREATE INDEX ix_audit_at ON audit_log(at);
CREATE INDEX ix_audit_entity ON audit_log(entity_type, entity_id);

-- ------------------------------------------------- record protection
-- Ledgers and posted documents can never be edited or deleted.
CREATE TRIGGER trg_movements_no_update BEFORE UPDATE ON stock_movements
BEGIN SELECT RAISE(ABORT, 'Stock movements are permanent records and cannot be changed.'); END;
CREATE TRIGGER trg_movements_no_delete BEFORE DELETE ON stock_movements
BEGIN SELECT RAISE(ABORT, 'Stock movements are permanent records and cannot be deleted.'); END;

CREATE TRIGGER trg_audit_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'The audit log cannot be changed.'); END;
CREATE TRIGGER trg_audit_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'The audit log cannot be deleted.'); END;

CREATE TRIGGER trg_sales_protect BEFORE UPDATE ON sales
WHEN OLD.status = 'void'
  OR NEW.invoice_no IS NOT OLD.invoice_no OR NEW.client_txn_id IS NOT OLD.client_txn_id
  OR NEW.customer_id IS NOT OLD.customer_id OR NEW.subtotal IS NOT OLD.subtotal
  OR NEW.line_discount_total IS NOT OLD.line_discount_total OR NEW.bill_discount IS NOT OLD.bill_discount
  OR NEW.total IS NOT OLD.total OR NEW.paid_amount IS NOT OLD.paid_amount
  OR NEW.tendered_cash IS NOT OLD.tendered_cash OR NEW.change_amount IS NOT OLD.change_amount
  OR NEW.credit_amount IS NOT OLD.credit_amount OR NEW.cogs_total IS NOT OLD.cogs_total
  OR NEW.business_date IS NOT OLD.business_date OR NEW.created_at IS NOT OLD.created_at
  OR NEW.cashier_id IS NOT OLD.cashier_id
BEGIN SELECT RAISE(ABORT, 'Completed sales cannot be modified. Use a return or void instead.'); END;
CREATE TRIGGER trg_sales_no_delete BEFORE DELETE ON sales
BEGIN SELECT RAISE(ABORT, 'Sales cannot be deleted.'); END;

CREATE TRIGGER trg_sale_items_no_update BEFORE UPDATE ON sale_items
BEGIN SELECT RAISE(ABORT, 'Sale lines cannot be modified.'); END;
CREATE TRIGGER trg_sale_items_no_delete BEFORE DELETE ON sale_items
BEGIN SELECT RAISE(ABORT, 'Sale lines cannot be deleted.'); END;
CREATE TRIGGER trg_sale_payments_no_update BEFORE UPDATE ON sale_payments
BEGIN SELECT RAISE(ABORT, 'Sale payments cannot be modified.'); END;
CREATE TRIGGER trg_sale_payments_no_delete BEFORE DELETE ON sale_payments
BEGIN SELECT RAISE(ABORT, 'Sale payments cannot be deleted.'); END;
CREATE TRIGGER trg_sale_returns_no_update BEFORE UPDATE ON sale_returns
BEGIN SELECT RAISE(ABORT, 'Returns cannot be modified.'); END;
CREATE TRIGGER trg_sale_returns_no_delete BEFORE DELETE ON sale_returns
BEGIN SELECT RAISE(ABORT, 'Returns cannot be deleted.'); END;
CREATE TRIGGER trg_sale_return_items_no_update BEFORE UPDATE ON sale_return_items
BEGIN SELECT RAISE(ABORT, 'Return lines cannot be modified.'); END;
CREATE TRIGGER trg_sale_return_items_no_delete BEFORE DELETE ON sale_return_items
BEGIN SELECT RAISE(ABORT, 'Return lines cannot be deleted.'); END;
CREATE TRIGGER trg_customer_payments_no_update BEFORE UPDATE ON customer_payments
BEGIN SELECT RAISE(ABORT, 'Customer payments cannot be modified.'); END;
CREATE TRIGGER trg_customer_payments_no_delete BEFORE DELETE ON customer_payments
BEGIN SELECT RAISE(ABORT, 'Customer payments cannot be deleted.'); END;

CREATE TRIGGER trg_purchases_no_update BEFORE UPDATE ON purchases
BEGIN SELECT RAISE(ABORT, 'Posted purchases cannot be modified. Use a purchase return instead.'); END;
CREATE TRIGGER trg_purchases_no_delete BEFORE DELETE ON purchases
BEGIN SELECT RAISE(ABORT, 'Posted purchases cannot be deleted.'); END;
CREATE TRIGGER trg_purchase_items_no_update BEFORE UPDATE ON purchase_items
BEGIN SELECT RAISE(ABORT, 'Purchase lines cannot be modified.'); END;
CREATE TRIGGER trg_purchase_items_no_delete BEFORE DELETE ON purchase_items
BEGIN SELECT RAISE(ABORT, 'Purchase lines cannot be deleted.'); END;
CREATE TRIGGER trg_supplier_payments_no_update BEFORE UPDATE ON supplier_payments
BEGIN SELECT RAISE(ABORT, 'Supplier payments cannot be modified.'); END;
CREATE TRIGGER trg_supplier_payments_no_delete BEFORE DELETE ON supplier_payments
BEGIN SELECT RAISE(ABORT, 'Supplier payments cannot be deleted.'); END;
CREATE TRIGGER trg_purchase_returns_no_update BEFORE UPDATE ON purchase_returns
BEGIN SELECT RAISE(ABORT, 'Purchase returns cannot be modified.'); END;
CREATE TRIGGER trg_purchase_returns_no_delete BEFORE DELETE ON purchase_returns
BEGIN SELECT RAISE(ABORT, 'Purchase returns cannot be deleted.'); END;
CREATE TRIGGER trg_purchase_return_items_no_update BEFORE UPDATE ON purchase_return_items
BEGIN SELECT RAISE(ABORT, 'Purchase return lines cannot be modified.'); END;
CREATE TRIGGER trg_purchase_return_items_no_delete BEFORE DELETE ON purchase_return_items
BEGIN SELECT RAISE(ABORT, 'Purchase return lines cannot be deleted.'); END;

CREATE TRIGGER trg_adjustments_no_update BEFORE UPDATE ON stock_adjustments
BEGIN SELECT RAISE(ABORT, 'Stock adjustments cannot be modified.'); END;
CREATE TRIGGER trg_adjustments_no_delete BEFORE DELETE ON stock_adjustments
BEGIN SELECT RAISE(ABORT, 'Stock adjustments cannot be deleted.'); END;
CREATE TRIGGER trg_adjustment_items_no_update BEFORE UPDATE ON stock_adjustment_items
BEGIN SELECT RAISE(ABORT, 'Stock adjustment lines cannot be modified.'); END;
CREATE TRIGGER trg_adjustment_items_no_delete BEFORE DELETE ON stock_adjustment_items
BEGIN SELECT RAISE(ABORT, 'Stock adjustment lines cannot be deleted.'); END;

CREATE TRIGGER trg_cash_closings_no_update BEFORE UPDATE ON cash_closings
BEGIN SELECT RAISE(ABORT, 'Cash closings cannot be modified.'); END;
CREATE TRIGGER trg_cash_closings_no_delete BEFORE DELETE ON cash_closings
BEGIN SELECT RAISE(ABORT, 'Cash closings cannot be deleted.'); END;

CREATE TRIGGER trg_expenses_protect BEFORE UPDATE ON expenses
WHEN OLD.status = 'void'
  OR NEW.amount IS NOT OLD.amount OR NEW.category_id IS NOT OLD.category_id
  OR NEW.method IS NOT OLD.method OR NEW.business_date IS NOT OLD.business_date
  OR NEW.expense_no IS NOT OLD.expense_no OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'Expenses cannot be edited. Void the expense and record a new one.'); END;
CREATE TRIGGER trg_expenses_no_delete BEFORE DELETE ON expenses
BEGIN SELECT RAISE(ABORT, 'Expenses cannot be deleted. Void the expense instead.'); END;
