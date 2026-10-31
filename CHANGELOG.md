# Changelog

All notable changes to CODE X MART are documented here.

## [1.0.0] — 2026-11-01

First release: the complete offline Windows POS (Phase 1).

### Added

- **Billing:** fast search, barcode scanning, `3*` quantity prefix, product tiles by category, line and bill discounts, retail and wholesale prices, cash, card, bank, wallet and cheque payments, split payments, change calculation, credit sales to customers with credit limits, receipt preview and printing (58 mm, 80 mm, A4), duplicate-charge protection and recovery of an unfinished bill after a crash.
- **Sales:** history with search (including scanning a receipt's barcode), reprints, same-day void, partial and full returns with cash or account refunds and a damaged/restock choice.
- **Customers:** records, balances, account statements, payments received.
- **Products:** categories, brands, units with whole or decimal quantities, SKU auto-numbering, barcode validation and in-store EAN-13 generation, size and colour variants, cost, sale and wholesale prices, reorder levels, opening stock, activation and deactivation.
- **Inventory:** weighted-average costing, stock adjustments with reasons, full stock history, batch and expiry tracking with first-expiry-first-out, stock record verification, barcode label printing.
- **Purchasing:** suppliers and balances, purchase orders, goods receiving with invoice discount allocation, batch and expiry entry and price updates, supplier payments, returns to supplier, duplicate supplier-invoice protection.
- **Expenses:** categories, recording and voiding.
- **Cash closing:** expected versus counted cash, printed slip.
- **Reports:** dashboard, sales summary, profit and loss, product and category sales, payment methods, returns, purchases, expenses, customer and supplier balances, inventory valuation, low stock, expiry, with CSV export and A4 printing.
- **Security:** Administrator, Manager and Cashier roles enforced in the backend; Argon2id passwords; account lockout; recovery key; permanent audit log; database triggers protecting posted records.
- **Backup:** automatic (scheduled and on exit) and manual backups, a second copy to a USB drive or other disk, verification after writing, validated restore with an automatic safety backup, backup history, a recovery screen for damaged databases, an automatic backup before upgrades.
- Windows installer (NSIS, per-user).
- Automated end-to-end tests that drive the real desktop app (`npm run test:e2e`).

### Known limitations

See [README → Current limitations](README.md#current-limitations).
