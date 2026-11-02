# CODE X MART — Architecture

This document describes how CODE X MART (Phase 1) is built and why. It is written for developers who will maintain the application or build Phase 2 (cloud synchronization).

## 1. Overview

CODE X MART is a Tauri 2 desktop application. It has three parts:

| Part | Path | Responsibility |
|---|---|---|
| Business core | `crates/codexmart-core` | Every business rule, all SQL, reports, backup/restore, receipt rendering for thermal printers. A plain Rust library with no dependency on Tauri or the UI. |
| Desktop shell | `src-tauri` | Starts the app, resolves the data folder, opens and upgrades the database, holds the signed-in session, schedules backups, talks to Windows printers, and exposes core services as Tauri commands. |
| User interface | `src` | React screens. It never contains business rules: it shows what the backend returns and sends the user's intent back. |

```
UI (React)  ──invoke──▶  commands (src-tauri)  ──▶  services (core)  ──▶  repo (SQL)  ──▶  SQLite
                                                       │
                                                       └──▶ domain (pure rules)
```

## 2. Layers of the business core

```
services / reports / backup   business workflows, permissions, transactions, audit
repo                          SQL for one aggregate per module (products, sales, …)
domain                        pure calculations: pricing, costing, payments, validation
db                            connection settings and versioned migrations
```

Each layer only calls the layers below it.

- **`domain`** has no I/O at all. `pricing::price_cart` computes line amounts, line discounts, the bill discount and its exact allocation to lines. `costing` implements weighted-average costing. `payments::settle` turns tenders into applied payments, change and credit. `validation` normalizes text, SKUs, barcodes (GS1 check digits), dates and quantities.
- **`repo`** is the only place that contains SQL. Functions take `&Connection`, so they work on a plain connection or inside a transaction. Every value is a bound parameter. Dynamic `WHERE` clauses are built from fixed fragments plus parameters (`repo::Where`), never by inserting user text.
- **`services`** implement one function per business operation (`sales::checkout`, `purchasing::create_purchase`, `inventory::adjust`, …). Each one:
  1. checks the user's permission (`Ctx::require`);
  2. validates and normalizes the input;
  3. opens an `IMMEDIATE` transaction (the write lock is taken up front);
  4. returns the existing document if the client transaction ID was already used (idempotency);
  5. performs the writes and appends the audit record in the same transaction;
  6. commits.
- **`services::stock`** is the *stock engine* and the only code that changes stock. Every change updates the product's quantity and value **and** appends a `stock_movements` row in the same transaction. `inventory::verify_ledger` proves the two always agree.
- **`reports`** contains the accounting rules for all reports (see §5).

## 3. Desktop shell

- `state.rs` resolves the data folder (`%APPDATA%\CODE X MART`, or `CODEXMART_DATA_DIR` for tests) and opens the database. It runs a quick integrity check, takes a safety backup before upgrading an existing database, applies migrations, and holds the single connection behind a mutex. The app runs on one till, so serializing access is simple and safe.
- Commands are `async` (run off the UI thread) and are thin: build a `Ctx` from the session, call one service, return the result or a structured `{ code, message }` error.
- `backup_jobs.rs` runs a background thread that checks every 10 minutes whether an automatic backup is due. It also runs the backup when the window closes, copies automatic backups to the secondary folder, and orchestrates restore.
- `printing.rs` uses the Windows spooler API (`OpenPrinterW`, `WritePrinter`, datatype `RAW`) for direct ESC/POS printing.
- `tauri-plugin-single-instance` guarantees that only one copy of the app writes to the database.
- `licensing/` enforces the free trial and licenses. `AppState::run` (used by every business command) asks `Licensing::require_access` first; `run_always` is used by the few commands that must keep working when activation is required (settings, own password, backup status). The rules and formats live in the separate `codexmart-license` crate, shared with the owner's License Generator (`tools/license-generator`), which alone enables its signing feature. See [LICENSING.md](LICENSING.md).
- The Content Security Policy only allows the app's own resources. The window has the `core:default` and dialog permissions only: there is no shell or general filesystem access from the UI. The two file-writing commands (`export_text_file` for `.csv`/`.txt`, and backups) take paths the user chose in a native dialog.

## 4. User interface

- `src/lib/api` is the **only** module that calls the backend (`invoke`). Screens use typed functions such as `api.sales.checkout(...)`. In Phase 2, a web admin can reuse the screens by replacing this transport with an HTTP client.
- TanStack Query caches server data. Mutations invalidate the related query keys.
- The billing screen keeps the in-progress bill in `localStorage` (`usePosCart`), including the checkout's idempotency key. After a crash the bill is restored, and pressing *Pay* again either completes the sale or returns the sale that was already saved. A customer is never charged twice.
- Printing renders React components into `#print-root`. Print CSS and an `@page` rule set the paper size (58 mm, 80 mm, A4 or label sizes).
- `lib/scanner.ts` recognizes keyboard-wedge barcode scanners by their typing speed (under 35 ms per key, ending with Enter) anywhere on the billing screen.
- `lib/i18n.ts` provides `t()` and a direction switch. Navigation and shared chrome use it; full string extraction for Urdu is future work.

## 5. Money, quantities and accounting rules

| Concept | Representation |
|---|---|
| Money | `i64` minor units (paisa). Rs 1,250.50 = `125050`. |
| Quantity | `i64` thousandths of a unit. 2 pieces = `2000`; 0.75 kg = `750`. Units marked "whole numbers" reject fractions. |
| Percentages | basis points (`1250` = 12.5 %). |
| Rounding | half away from zero, in one function (`money::div_round`). |
| Allocation | largest-remainder method (`money::allocate`): parts always sum exactly to the total. |

**Costing.** Perpetual weighted average (AVCO). For each product, the core stores the on-hand quantity and the total cost value of that stock. Purchases add their actual net cost: the line cost minus its share of any invoice discount. Sales remove a proportional share of the value, and the removed value is written on the sale line as `cost_amount`. Returns put goods back at the cost recorded on the original line. Returns to suppliers remove stock at the purchase cost.

**Report formulas** (`reports.rs`):

- Net sales = invoice totals after discounts − refunds on returns.
- Cost of goods sold = Σ `cost_amount` of sold lines − cost of returned goods that were put back on the shelf. Goods returned as damaged stay in cost of goods.
- Gross profit = net sales − cost of goods sold.
- Net operating result = gross profit − operating expenses + stock adjustment gains − stock write-offs. Opening stock is not a gain.
- Purchases and supplier payments are **not** expenses: purchases become stock and reach profit only through cost of goods sold. This prevents double counting.
- Voided sales and voided expenses are excluded everywhere.

## 6. Reliability measures

| Risk | Measure |
|---|---|
| Partial writes | Every operation is one transaction. A failed sale leaves no trace and uses no invoice number. |
| Double-clicks and retries | Client transaction IDs with `UNIQUE` constraints; repeating a request returns the original document. |
| Editing history | Database triggers reject `UPDATE`/`DELETE` on posted sales, sale lines, payments, returns, purchases, supplier payments, customer payments, stock movements, adjustments, cash closings and the audit log. Corrections are new documents (void, return, adjustment). |
| Stock drift | The stock engine updates the product and the ledger together; *Verify stock records* recomputes from history. |
| Power loss | SQLite WAL with `synchronous=FULL`. |
| Corrupt database at startup | `PRAGMA quick_check`. On failure, a recovery screen is shown and the file is never reset. |
| App upgrade | Checksummed migrations; automatic `pre-upgrade` backup; a newer database version is refused. |
| Crash during billing | The in-progress bill and its idempotency key are persisted locally. |
| Two copies running | Single-instance plugin. |
| Unclean exit | On exit the write-ahead log is checkpointed and the database closed, so the data file is complete on its own. If the app is killed instead, SQLite replays the log on the next start. |
| File briefly locked at start-up | The recovery screen's *Try opening again* retries without restarting the app. |
| Panics | Logged through `tauri-plugin-log`; a poisoned mutex is recovered (the transaction was rolled back during unwinding). |

## 7. Security

- Passwords are hashed with **Argon2id** (random salt, PHC string format). Five failed attempts lock an account for five minutes. Unknown usernames take comparable time to reject.
- Roles: **Administrator** (everything), **Manager** (everything except users, settings and restore), and **Cashier** (billing, sales history, customers, customer payments). The matrix lives in `context.rs` (`Role::allows`) and is enforced in every service. The UI only hides what the backend would refuse. Cost prices and profit are removed from responses for roles without `viewCosts`.
- The recovery key is 20 random characters, shown once and stored only as an Argon2 hash in `app_meta`.
- All SQL uses bound parameters.
- Settings contain no secrets.
- Licenses are Ed25519-signed and device-bound; the app contains only public keys ([LICENSE_SECURITY.md](LICENSE_SECURITY.md)).

## 8. Preparing for Phase 2 (cloud synchronization)

Phase 1 must work with no cloud at all. The following choices make adding synchronization later a matter of *adding* code, not rewriting:

| Requirement | Phase 1 implementation |
|---|---|
| Stable unique identifiers | UUIDv7 primary keys everywhere. They are time-ordered and globally unique, so records from different devices never collide. Seeded reference data (units, categories, expense categories) uses fixed IDs that are identical on every device. |
| Device identity | `app_meta.device_id` and `app_meta.db_id` are created once per database. |
| Consistent timestamps | UTC RFC 3339 with milliseconds (`created_at`/`updated_at`) plus a local `business_date` for accounting. |
| Idempotent operations | Every document carries a `client_txn_id` with a `UNIQUE` constraint: the natural key for retry-safe upload. |
| Conflict-friendly data | Financial documents and the stock ledger are append-only. Two devices can only *add* records, never edit the same one. Master data (products, customers, suppliers) carries `updated_at` for last-writer-wins or field-level merge. |
| Shared business rules | `codexmart-core` has no Tauri dependency. A Rust API server can use the same `domain` rules (pricing, costing, validation). The SQLite `repo` layer would get a PostgreSQL counterpart. |
| Audit history | `audit_log` records who did what and when. |
| Versioned schema | Checksummed, ordered migrations. |
| File storage independence | `products.image_ref` is an opaque reference. Nothing in business logic reads files. |
| UI transport | `src/lib/api` is the single backend gateway. |

### Sync outbox decision

Phase 1 does **not** add a `sync_outbox` table. An outbox has no value until there is something to send to, and an unconsumed queue would grow forever on a shop computer. Instead, Phase 1 guarantees the properties a sync engine needs: append-only documents with unique IDs and client transaction IDs, `updated_at` on master data, an append-only stock ledger, and an audit log. When Phase 2 starts, a migration will add `sync_outbox` (or per-table `synced_at` columns), and the initial upload will be a full snapshot. Both are possible because every record is already globally identifiable.

## 9. Testing strategy

- **Domain unit tests** cover rounding, allocation, cart pricing, costing (including negative stock), payment settlement, GS1 check digits, SKU, barcode, quantity and date rules.
- **Integration tests** (`crates/codexmart-core/tests`) set up a full shop on an in-memory or temporary file database and run real workflows: catalog, sales, returns, voids, purchasing, purchase orders, batches/FEFO, adjustments, reports with hand-calculated expected figures, cash closing, backup/corruption/restore, migrations and upgrade paths, authentication, lockout, recovery and record immutability.
- **UI unit tests** (Vitest) cover money and quantity parsing and formatting.
- **End-to-end tests** (`e2e/`, `npm run test:e2e`) drive the real desktop app through WebView2's debugging port. They walk through setup, billing, returns, purchasing, expenses, cash closing, roles, backup and restore, recovery-key reset, clean shutdown and damaged-database recovery on a throwaway data folder. They also verify that no network request leaves the computer.
