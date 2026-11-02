# Changelog

All notable changes to CODE X MART are documented here.

## [1.1.0] — 2026-11-02

Free trial, offline license activation and the uninstall data choice.

### Added

- **30-day free trial** from shop setup.
  - From day 25 a reminder shows the days left once a day after sign-in, plus a header chip.
  - From day 31, staff can sign in but business screens wait for activation. Backups, restore, sign-out and the request code keep working, and no data is deleted or hidden.
  - Enforced in the backend: business commands return `LICENSE_REQUIRED`.
- **Offline activation** (*Settings → License*, or the activation screen):
  - a device-bound request code (`CXMR-`, 8 groups) and an Ed25519-signed activation key (`CXMA-`, 34 groups);
  - per-group Luhn mod 32 check symbols, with live typing feedback;
  - clear messages for wrong device, forged, expired and mistyped keys;
  - the software owner's contact details and a WhatsApp button.
- **License types:** 1, 3 and 6 months, 1 year, lifetime, 30-day trial extension and custom end date. Each license has a serial number in the format `CXMT-XXXX-XXXX-XXXX-XXXX`.
- **Trial protection:**
  - authenticated state copies in the registry, AppData, LocalAppData, ProgramData, other local disks and the database, merged conservatively;
  - a trusted clock that never goes backwards;
  - clock set-backs detected, and the trial ends after three;
  - no reset on update, reinstall, data deletion, a new account or a backup restore.
- **Device binding** from hashed SMBIOS identifiers, with tolerance for small hardware changes and owner-controlled transfers.
- **License Generator** (`tools/license-generator`), a separate owner-only program, never in the customer installer:
  - signing key protected by passphrase (Argon2id + XChaCha20-Poly1305) and Windows DPAPI;
  - mandatory key backup, restore on a new PC and passphrase change;
  - request check, review before issuing and a ready-to-send WhatsApp message;
  - license history with search, reissue, transfer and CSV export;
  - idle lock, and a pause after wrong passphrases.
- **Uninstaller data choice:**
  - **Keep my data and credentials** (default) or **Delete all my data and credentials**, with two confirmations;
  - deletion is verified, and files in use are reported, with a marker so CODE X MART finishes the deletion at its next start;
  - `/S /DELETEDATA` for silent use;
  - the device license record is kept in both cases.
- Documentation: licensing overview, trial system, offline activation, License Generator, owner key management, license security, device binding, troubleshooting, uninstallation.
- Tests:
  - 62 new Rust tests (144 in total);
  - end-to-end suite `03-licensing.mjs`, 44 checks on the app and the generator;
  - `04-installer.mjs`, 36 checks on the production installer and uninstaller, driven through UI Automation.

### Changed

- Version 1.1.0. The production signing key (fingerprint `RPF7-LC9Y-XCWJ-GJBM`) is built in. The installer uses a custom NSIS template (`src-tauri/windows/`) derived from Tauri's.
- `npm run test:e2e` builds the app with the `e2e` test feature and also builds the License Generator.

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
- Windows installer (NSIS, per-user). It is fully offline: it contains Microsoft's offline WebView2 Runtime installer and runs it only when WebView2 is missing.
- Automated end-to-end tests that drive the real desktop app (`npm run test:e2e`).

### Known limitations

See [README → Current limitations](README.md#current-limitations).
