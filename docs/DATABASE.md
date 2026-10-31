# CODE X MART — Database

## Location and settings

| Item | Value |
|---|---|
| Engine | SQLite 3 (bundled with the app through `rusqlite`) |
| File | `%APPDATA%\CODE X MART\data\codexmart.db` |
| Journal | WAL (`codexmart.db-wal`, `codexmart.db-shm` appear while the app runs) |
| Durability | `synchronous = FULL` |
| Foreign keys | enforced (`PRAGMA foreign_keys = ON`) |
| Busy timeout | 5 seconds |

The data folder also contains `backups\` (automatic backups), `temp\` (working files during backup and restore) and `backup-history.jsonl` (backup log, kept outside the database so a restore does not erase it).

`CODEXMART_DATA_DIR` (environment variable) overrides the data folder. It is used for testing only.

## Conventions

- **Primary keys:** UUIDv7 text. They are unique across devices and sort by creation time.
- **Money:** `INTEGER` paisa.
- **Quantities:** `INTEGER` thousandths of a unit.
- **Timestamps:** `created_at` and `updated_at` are UTC RFC 3339 strings with milliseconds. `business_date` is the local calendar date `YYYY-MM-DD` that the transaction belongs to.
- **Document numbers** come from `sequences`, inside the same transaction as the document, so numbers are gap-free (a failed sale does not consume an invoice number).

## Migrations

Migrations live in `crates/codexmart-core/src/db/migrations/` and are compiled into the app.

| Version | File | Content |
|---|---|---|
| 1 | `0001_initial_schema.sql` | All tables, indexes and protection triggers |
| 2 | `0002_reference_data.sql` | Number sequences, units, default product categories, expense categories (with fixed IDs) |

Rules:

1. Each migration runs in its own transaction and is recorded in `schema_migrations` with its SHA-256 checksum. Line endings are normalized, so CRLF and LF checkouts produce the same checksum.
2. **Never edit a released migration.** Add a new file. An edited migration is detected at startup, and the database is left untouched.
3. If the database version is newer than the app supports, the app refuses to open it and asks for an update.
4. Before migrating an existing database, the app writes a `pre-upgrade` backup. If that backup fails, the upgrade does not run.

## Tables

### System

| Table | Purpose |
|---|---|
| `app_meta` | `app` marker (`CODE X MART`), `db_id`, `device_id`, `created_at`, `recovery_key_hash` |
| `schema_migrations` | Applied migrations with checksums |
| `settings` | Key/value settings (JSON values), merged over defaults |
| `sequences` | Document number prefixes and counters |
| `users` | Accounts, roles, Argon2id password hashes, lockout state |
| `audit_log` | Permanent record of important actions |

### Catalog and stock

| Table | Purpose |
|---|---|
| `categories`, `brands`, `units` | Reference lists (deactivated, never deleted) |
| `products` | Sellable items (each size or colour variant is its own product, linked by `variant_group_id`), prices, current `stock_qty`, `stock_value` and average `cost_price` |
| `stock_movements` | Append-only stock ledger: every change with quantity and value, balance after, reference, user and date |
| `stock_batches` | Batch number, expiry, received and remaining quantity, for batch-tracked products |
| `stock_adjustments`, `stock_adjustment_items` | Adjustment documents with reason and value change |

Invariant: for every product, `stock_qty = Σ stock_movements.qty_change` and `stock_value = Σ stock_movements.value_change`. *Inventory → Verify stock records* and *Settings → Security & health* check this.

### Sales

| Table | Purpose |
|---|---|
| `customers` | Customer records, opening balance, credit limit |
| `sales` | Invoice header: totals, discounts, paid/tendered/change/credit, cost of goods, status (`completed`/`void`) |
| `sale_items` | Lines with a product snapshot (name, SKU, unit), price, discounts, net amount and **recorded cost** |
| `sale_payments` | Amounts applied per payment method (cash is stored net of change) |
| `sale_returns`, `sale_return_items` | Returns with refund method, refund amount, restock flag and returned cost |
| `customer_payments` | Payments received against customer balances |
| `cash_closings` | One closing per business day |

Customer balance = opening balance + Σ credit on completed sales − Σ customer payments − Σ refunds to account.

### Purchasing and expenses

| Table | Purpose |
|---|---|
| `suppliers` | Supplier records and opening balance |
| `purchase_orders`, `purchase_order_items` | Orders and received quantities |
| `purchases`, `purchase_items` | Supplier invoices (goods received), with discount allocated into `net_cost` |
| `supplier_payments` | Payments to suppliers, optionally linked to a purchase |
| `purchase_returns`, `purchase_return_items` | Goods returned to suppliers |
| `expense_categories`, `expenses` | Operating expenses (`active`/`void`) |

Supplier balance = opening balance + Σ purchases − Σ supplier payments − Σ purchase returns.

`UNIQUE(supplier_id, supplier_invoice_no)` prevents entering the same supplier invoice twice.

## Record protection

Triggers abort any `UPDATE` or `DELETE` on posted records:

- `sales`: only the status change to `void` (with `voided_at`, `voided_by`, `void_reason`) is allowed. Totals, customer, dates and numbers can never change, and a voided sale cannot change again.
- `sale_items`, `sale_payments`, `sale_returns`, `sale_return_items`, `customer_payments`, `purchases`, `purchase_items`, `supplier_payments`, `purchase_returns`, `purchase_return_items`, `stock_movements`, `stock_adjustments`, `stock_adjustment_items`, `cash_closings`, `audit_log`: no updates or deletes at all.
- `expenses`: only voiding is allowed; amount, category, method and date are locked.

## Integrity checks

- At startup: `PRAGMA quick_check`. On failure, the recovery screen is shown and nothing is reset.
- Backups: the snapshot passes `PRAGMA integrity_check` and `PRAGMA foreign_key_check` before it is accepted, and again before any restore.
- *Settings → Security & health → Run health check*: full integrity check plus the stock ledger verification.

## Opening the database manually (advanced)

Close CODE X MART first. Make a backup, then open a **copy** of the file with any SQLite browser. Never edit records directly: the protection triggers will refuse it, and bypassing them would break the stock ledger and audit trail.
