# CODE X MART — Roadmap

## Phase 1 — Offline Windows POS (current, v1.0.0)

Delivered: a complete offline desktop application covering billing, receipts, returns, customers and credit, products and variants, inventory with batches and expiry, purchasing, expenses, cash closing, reports, users and roles, audit log, and backup and restore. See [README](../README.md#main-features).

### Near-term improvements (still offline)

These are candidates for 1.x releases. None is implemented yet.

- Bulk product import and export (CSV / Excel) for faster first-time setup.
- Hold or park a bill and recall it later.
- Sales tax / GST on products and receipts.
- Optional product photos on product records, for catalogues and the future website (the `image_ref` field already exists). The billing screen stays image-free by design.
- Urdu translation and right-to-left layout (the `t()` localization layer exists for navigation).
- Matching customer payments to specific invoices.
- Customer statements printed on A4.
- Cash drawer shift handover (more than one closing per day).
- Verification on Windows 11 and with physical thermal printers and scanners.
- Code-signed installer.

## Phase 2 — Offline-first cloud ecosystem (planned, not started)

### Target architecture

```
 Shop desktop(s)                 Cloud                              Web
┌──────────────────┐   HTTPS    ┌──────────────────────────┐      ┌──────────────────┐
│ CODE X MART      │◀──────────▶│ Sync & business API      │◀────▶│ Admin website    │
│ SQLite (offline) │  retry-safe│ (shared Rust rules)      │      │ (full back office)│
│ sync outbox      │  batches   │ PostgreSQL (Neon)        │      └──────────────────┘
│ device identity  │            │ object storage (ImageKit)│      ┌──────────────────┐
└──────────────────┘            └──────────────────────────┘◀────▶│ Public website   │
                                                                  │ (approved data)  │
                                                                  └──────────────────┘
```

### Planned work

1. **Device authorization.** Register each till with the cloud using the existing `app_meta.device_id`. Issue per-device credentials and support revoking them.
2. **Sync outbox.** A migration adds a local `sync_outbox` (or `synced_at` columns). Every committed write enqueues the changed record IDs in the same transaction. Uploads run in batches and are retried until acknowledged.
3. **Initial upload.** A full snapshot of an existing shop. All IDs are already UUIDv7, so nothing needs renumbering.
4. **Server.** An authenticated API (Rust, reusing `codexmart-core::domain` for pricing, costing and validation) with PostgreSQL on Neon. Authorization per user and device. Duplicate prevention by `client_txn_id`. An audit history.
5. **Conflict handling.**
   - Financial documents and the stock ledger are append-only and never conflict.
   - Master data (products, prices, customers, suppliers) uses `updated_at` with field-level last-writer-wins, plus a review queue for conflicting price changes.
   - Stock is computed from the merged movement ledger, not overwritten.
   - Document numbers get a per-device prefix (for example `INV-A-000123`) to stay unique across tills.
6. **Download sync.** Changes made on the admin website (for example price updates) flow down to the tills.
7. **Admin website.** The same functions as the desktop, on cloud data. The UI's single API gateway (`src/lib/api`) makes the React screens reusable with an HTTP transport.
8. **Public website.** Only explicitly approved fields (product name, photo, public price, availability). No costs, customers, suppliers or internal records.
9. **Cloud files.** Product images in ImageKit, uploaded through an offline queue on the desktop.

### Principles that stay

- The desktop keeps working fully offline, at all times.
- The cloud is never required to sell, print or back up.
- Local backups remain, independent of the cloud.
