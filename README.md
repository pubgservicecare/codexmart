# CODE X MART

**CODE X MART** is an offline retail point-of-sale (POS) and shop management application for Windows. It is built for marts and general stores selling skincare and creams, makeup and cosmetics, bags and general mart products.

It handles the full daily cycle of a shop: billing and receipts, returns, customers and credit (udhaar), products and stock, suppliers and purchasing, expenses, cash closing, profit reporting, and automatic backups. Everything runs on one computer, with no internet connection required.

> **Status:** Version 1.1.0, Phase 1 (offline desktop application), with a 30-day free trial and offline license activation. The cloud features described under [Roadmap](#future-cloud-architecture-roadmap) are **not** implemented yet.

---

## Contents

- [Main features](#main-features)
- [Supported Windows versions](#supported-windows-versions)
- [Technology stack](#technology-stack)
- [Architecture](#architecture)
- [Folder structure](#folder-structure)
- [Installing CODE X MART (shop computer)](#installing-code-x-mart-shop-computer)
- [Development setup](#development-setup)
- [Development commands](#development-commands)
- [Production build](#production-build)
- [Database](#database)
- [Backup and restore](#backup-and-restore)
- [Printer setup](#printer-setup)
- [Barcode scanners and labels](#barcode-scanners-and-labels)
- [Testing](#testing)
- [Release procedure](#release-procedure)
- [Troubleshooting](#troubleshooting)
- [Future cloud architecture roadmap](#future-cloud-architecture-roadmap)
- [Current limitations](#current-limitations)
- [Documentation](#documentation)

---

## Main features

**Billing (POS)**
- Fast billing screen with search by name, SKU, barcode, brand, size or colour; product tiles by category for shops without a scanner.
- USB barcode scanner support (keyboard-wedge scanners), including a `3*` prefix to add several units at once.
- Line discounts and whole-bill discounts (amount or %), an optional cashier discount limit, and retail or wholesale prices.
- Cash, card, bank transfer, mobile wallet and cheque payments, with split payments and change calculation.
- Credit sales (udhaar) to named customers, with optional credit limits.
- Receipt preview and printing (58 mm, 80 mm or A4), reprints, and an invoice barcode on every receipt.
- Gap-free invoice numbers (`INV-000001`, configurable prefix) and duplicate-sale protection, so a crash or double-click never charges twice.
- Voiding a sale on the same day, plus partial or full returns with refund by cash or to the customer's account. Damaged returned goods can be kept out of stock.

**Inventory**
- Products with categories, brands, units (whole or decimal quantities), SKU, barcode, size and colour variants, cost, sale and wholesale prices, and reorder levels.
- Opening stock; stock adjustments (count, damage, expiry, loss, theft, own use, found); a complete permanent stock history.
- Optional batch and expiry tracking per product, with first-expiry-first-out selling and an expiry report.
- Weighted-average costing; inventory valuation at cost and at selling price; a stock record verification tool.
- In-store EAN-13 barcode generation and barcode/price label printing.

**Purchasing**
- Suppliers with opening balances, payable balances and account statements.
- Purchase orders (draft → ordered → partly received → received or cancelled).
- Receiving goods (supplier invoices), with invoice discounts spread into item cost, batch and expiry entry, optional selling-price updates, and payment at receipt.
- Supplier payments and returns to supplier.

**Money and reports**
- Expenses with built-in and custom categories (rent, electricity, salaries, transport, utilities, maintenance, miscellaneous). Expenses are voided, never deleted.
- End-of-day cash closing: expected versus counted cash, with a printed slip.
- Dashboard: today's and this month's net sales, gross profit, expenses, net operating result, stock value, low stock, receivables and payables, plus a 14-day trend.
- Reports: sales summary (daily, weekly, monthly), profit and loss, sales by product, sales by category, payment methods, returns, purchases, expenses, customer and supplier balances, inventory valuation, low stock and expiry. Every report exports to CSV and prints on A4.

**Safety**
- Users with Administrator, Manager and Cashier roles. Permissions are enforced in the backend, not only hidden in the UI.
- Passwords hashed with Argon2id; account lock after 5 wrong passwords; a one-time recovery key for administrator password resets.
- Permanent audit log of important actions.
- Posted sales, payments, purchases, returns, stock movements and the audit log cannot be edited or deleted (enforced by database triggers).
- Automatic and manual backups with verification, a second copy on a USB drive or other disk, and a guarded restore.

**Free trial and licensing (offline)**
- **30-day free trial** from shop setup. A once-a-day reminder runs from day 25; from day 31, staff can still sign in, but the business screens wait for activation. Shop data is never touched, and backups stay available.
- **Offline activation:** the shop sends a short request code (`CXMR-…`) to the software owner, Hamad Mansha (+92 348 6825408, WhatsApp button included). The owner sends back a device-bound activation key (`CXMA-…`), which is checked on the computer with no internet.
- Keys are signed with **Ed25519**. CODE X MART contains only the public key, and every group of a code has a Luhn mod 32 check symbol that catches typing mistakes.
- License types: 1, 3 and 6 months, 1 year, lifetime, 30-day trial extension and custom dates. Each license has a serial number like `CXMT-7K92-4P6D-8W3H-5R2X`.
- Trial protection: authenticated copies in several places, clock set-back detection, and no reset on reinstall, data deletion, new accounts or backup restore.
- A separate owner-only **License Generator** (`tools/license-generator`): encrypted signing key with backup and recovery, issuing, license history, reissue, transfer to a new computer, and CSV export. It is never part of the customer installer.
- The **uninstaller asks** whether to keep or delete all shop data and credentials (two confirmations to delete). In both cases the device license record is kept, so reinstalling does not start a new trial.
- Details: [docs/LICENSING.md](docs/LICENSING.md).

## Supported Windows versions

| Windows | Status |
|---|---|
| Windows 10 64-bit (version 1809 or later) | Supported. Developed and tested on Windows 10 Pro 22H2 (build 19045). |
| Windows 11 64-bit | Expected to work (same WebView2 runtime). **Not yet tested on a Windows 11 machine.** |

CODE X MART needs the **Microsoft Edge WebView2 Runtime**. It is preinstalled on Windows 11 and on updated Windows 10 computers. If it is missing, the installer installs it from Microsoft's offline installer, which is built into the setup file, so no internet is needed. See [docs/INSTALLATION.md](docs/INSTALLATION.md#webview2-and-computers-without-internet).

## Technology stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://tauri.app) (Rust), WebView2 |
| Business core | Rust library `codexmart-core` |
| Database | SQLite (bundled through `rusqlite`), WAL journal, `synchronous=FULL` |
| User interface | React 18, TypeScript, Vite, Tailwind CSS 3, TanStack Query, React Router |
| Security | Argon2id password hashing, SHA-256 backup checksums |
| Printing | Windows print dialog (any printer) or direct ESC/POS (RAW) printing to thermal printers |
| Barcodes | JsBarcode (Code 128 / EAN-13 rendering), GS1 check-digit validation |
| Tests | `cargo test` (unit and integration tests on real SQLite databases), Vitest |

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ React UI (src/)                                                  │
│   screens → lib/api (the only place that calls the backend)      │
└───────────────────────────────┬──────────────────────────────────┘
                                │ Tauri IPC (typed commands)
┌───────────────────────────────▼──────────────────────────────────┐
│ Desktop shell (src-tauri/)                                       │
│   commands (thin adapters) · session · printing · backup jobs    │
└───────────────────────────────┬──────────────────────────────────┘
                                │ plain Rust calls
┌───────────────────────────────▼──────────────────────────────────┐
│ Business core (crates/codexmart-core/)                           │
│   services   workflows, permissions, transactions, audit         │
│   reports    accounting rules for every report                   │
│   backup     snapshot, verify, restore                           │
│   repo       all SQL, one module per aggregate                   │
│   domain     pure rules: pricing, costing, payments, validation  │
│   db         connection setup, versioned migrations              │
└───────────────────────────────┬──────────────────────────────────┘
                                ▼
                     SQLite database (AppData)
```

Key principles:

- **All business rules live in Rust**, in `codexmart-core`, which has no dependency on Tauri or the UI. The billing screen does not calculate totals itself: it asks the backend for a *quote* whenever the cart changes, and checkout prices the cart again with the same code.
- **Money is stored as integer paisa and quantities as integer thousandths of a unit.** No floating point touches business values.
- **Every write is one database transaction**, together with its audit record. Document-creating operations are idempotent: each carries a client transaction ID.
- **Cost of goods sold** is the weighted-average cost recorded on each sale line at the moment of sale.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design, including the preparation for cloud synchronization.

## Folder structure

```
CODE X MART/
├── crates/codexmart-core/      Business core (Rust library)
│   ├── src/domain/             Pricing, costing, payments, validation (pure functions)
│   ├── src/db/                 Connection setup and versioned SQL migrations
│   ├── src/repo/               Data access (all SQL)
│   ├── src/services/           Business operations
│   ├── src/reports.rs          Reports and dashboard
│   ├── src/backup.rs           Backup and restore engine
│   ├── src/escpos.rs           Thermal printer receipt rendering
│   └── tests/                  Integration tests
├── crates/codexmart-license/   Licensing: codes, signatures, device id, trial records
├── src-tauri/                  Desktop shell (Tauri 2)
│   ├── src/commands/           Tauri commands (one file per area)
│   ├── src/state.rs            Data folder, database connection, session
│   ├── src/licensing/          Trial/license enforcement, device fingerprint, state copies
│   ├── src/backup_jobs.rs      Scheduled backups, USB copy, restore
│   ├── src/printing.rs         Windows printer access (RAW / ESC-POS)
│   ├── windows/                Installer template and uninstall data choice (NSIS)
│   ├── icons/                  Application icons
│   └── tauri.conf.json         App and installer configuration
├── tools/license-generator/    Owner-only License Generator (separate Tauri app)
├── src/                        User interface (React + TypeScript)
│   ├── lib/                    API client, formatting, session, print, scanner, CSV
│   ├── components/             Design system, receipt, barcode, product search
│   └── features/               One folder per screen (pos, sales, products, …)
├── public/                     Static files (logo)
├── docs/                       Documentation
├── Cargo.toml                  Rust workspace
└── package.json                Node project and scripts
```

## Installing CODE X MART (shop computer)

1. Run `CODE X MART_1.1.0_x64-setup.exe`. This single file is the complete installer. It does not need the project folder, developer tools or internet.
2. Windows SmartScreen may warn that the publisher is unknown, because the installer is **not code-signed**. Choose **More info → Run anyway**.
3. Start CODE X MART from the Start menu or desktop shortcut.
4. On first start, enter the store details, create the administrator account, and **write down the recovery key** that is shown. The 30-day free trial starts now.
5. To activate (any time, or when the trial ends): send the request code from *Settings → License* to the software owner and enter the activation key you receive. See [docs/OFFLINE_ACTIVATION.md](docs/OFFLINE_ACTIVATION.md).

Uninstalling asks whether to keep or delete your shop data: see [docs/UNINSTALLATION.md](docs/UNINSTALLATION.md).

Full guide: [docs/INSTALLATION.md](docs/INSTALLATION.md). Daily use: [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Development setup

Prerequisites (Windows 10/11 64-bit):

| Tool | Version used | Notes |
|---|---|---|
| Rust | stable 1.85+ (developed with 1.99), MSVC toolchain | `rustup default stable-x86_64-pc-windows-msvc`; add `clippy` and `rustfmt` |
| Visual Studio Build Tools | 2022, "Desktop development with C++" | Required by Rust on Windows |
| Node.js | 20.19+ or 22.x (developed with 22.23) | npm 10 |
| WebView2 Runtime | any current | Preinstalled on Windows 11 |

```powershell
git clone https://github.com/pubgservicecare/codexmart.git
cd codexmart
npm install
```

## Development commands

| Command | What it does |
|---|---|
| `npm run app:dev` | Runs the desktop app with hot reload (Vite on port 1420 + Tauri). |
| `npm run dev` | Runs the UI only in a browser (backend calls will fail outside Tauri). |
| `npm run typecheck` | TypeScript type check. |
| `npm run lint` | ESLint. |
| `npm test` | Frontend unit tests (Vitest). |
| `cargo test --workspace` | Rust unit and integration tests. |
| `cargo clippy --workspace --all-targets` | Rust lints. |
| `npm run check` | All of the above. |
| `npm run test:e2e` | End-to-end tests on the real desktop app and License Generator, using throwaway folders. |
| `npm run licensegen:dev` | Runs the License Generator UI in Vite (port 1430); for the desktop window run `npx tauri dev` in `tools/license-generator`. |
| `npm run licensegen:build` | Builds `target\release\CODE X MART License Generator.exe` (owner only, not part of the installer). |

During development you can keep test data away from your real data:

```powershell
$env:CODEXMART_DATA_DIR = "C:\temp\codexmart-dev"; npm run app:dev
```

## Production build

```powershell
npm run app:build
```

This builds the optimized application and the Windows installer:

- Application: `target\release\CODE X MART.exe`
- Installer: `target\release\bundle\nsis\CODE X MART_1.1.0_x64-setup.exe`

The installer contains only these items:

- `CODE X MART.exe`, with the public license key built in;
- the uninstaller;
- the WebView2 offline installer.

The License Generator, the private signing key and the owner's license history are never part of it. The uninstaller uses the template in `src-tauri/windows/`: the Tauri template plus the keep/delete data page.

The owner's License Generator is built separately with `npm run licensegen:build`. The output is `target\release\CODE X MART License Generator.exe`, a single file of about 11 MB.

The application is a single executable (about 15 MB). The UI, fonts and icons are embedded in it, SQLite is compiled in, and the C runtime is linked statically, so it needs no other files. The installer is about 212 MB because it also contains Microsoft's offline WebView2 installer (`webviewInstallMode: offlineInstaller` in `src-tauri/tauri.conf.json`). That installer runs only on computers that do not already have WebView2.

To build a small installer (about 6 MB) for computers that are known to have WebView2 or internet, set `webviewInstallMode` to `{ "type": "embedBootstrapper" }`. On a computer without WebView2, that version must download it during installation.

The first build downloads the NSIS installer toolkit and the WebView2 offline installer. Both are cached for later builds.

## Database

- **Engine:** SQLite, a single file, opened in WAL mode with `synchronous=FULL` and foreign keys enabled.
- **Location:** `%APPDATA%\CODE X MART\data\codexmart.db` (for example `C:\Users\<name>\AppData\Roaming\CODE X MART\data\codexmart.db`). This is outside the installation folder, so updates and reinstalls never touch it.
- **Migrations:** versioned SQL files embedded in the app (`crates/codexmart-core/src/db/migrations/`), applied in order inside transactions, with checksums. A safety backup is taken automatically before an existing database is upgraded. A database from a *newer* app version is refused rather than modified.
- **Integrity:** a quick check runs at every start. If the database cannot be opened, the app shows a recovery screen and never resets or recreates the data.

Details: [docs/DATABASE.md](docs/DATABASE.md).

## Backup and restore

- **Automatic backups** run every 24 hours by default (configurable from 1 to 48 hours) while the app is open, and when the app is closed. The latest 30 automatic backups are kept.
- **Second copy:** set a USB drive or second disk in *Backup & restore → Backup settings*. Every automatic backup is copied there whenever it is connected.
- **Manual backup:** *Back up now*, or *Back up to USB / folder…*.
- Each backup is a single `.cxmbak` file containing a consistent database snapshot and a manifest with a SHA-256 checksum. It is read back and verified immediately after it is written.
- **Restore** (administrator only) validates the file first (checksum, SQLite integrity, application marker, version). It then takes a safety backup of the current data, replaces the database atomically, and signs everyone out.

Step-by-step guide, including **recovery after a Windows reinstall**: [docs/BACKUP_AND_RESTORE.md](docs/BACKUP_AND_RESTORE.md).

## Printer setup

*Settings → Receipt & printer*:

- **Windows print dialog** (default) works with any installed printer: thermal receipt printers with a Windows driver, laser and inkjet printers. Choose 80 mm, 58 mm or A4 paper.
- **Direct to thermal printer (ESC/POS)** prints silently, without a dialog, to a selected printer. It suits 58 mm and 80 mm ESC/POS printers (Epson TM, Xprinter, Rongta, Bixolon and similar) and can open a cash drawer connected to the printer.
- Use **Print test** to check alignment.

> **Hardware testing status:** no physical printer was connected during development.
> - The end-to-end tests verify that receipts and closing slips are rendered into the print area (up to `window.print()`), but not the Windows print dialog or printed output.
> - Direct ESC/POS printing has only its byte output unit-tested.
>
> Run *Print test* on the shop's printer before going live.

## Barcode scanners and labels

- Any **USB barcode scanner in keyboard mode** (the default for most scanners) works without setup. Scan on the billing screen, the product search boxes or the sales history search.
- Product barcodes are validated: EAN-8, UPC-A, EAN-13 and GTIN-14 check digits are verified.
- Products without a manufacturer barcode can be given an **in-store EAN-13** (prefix `20`) with *Generate* in the product form.
- *Barcode labels* prints product name, price and barcode on 38×25 mm or 50×25 mm roll labels, or A4 label sheets (65 per sheet).
- Manual billing works fully without a scanner.

## Testing

```powershell
cargo test --workspace     # Rust: unit + integration tests
npm test                   # UI unit tests
npm run typecheck; npm run lint
```

The Rust integration tests (`crates/codexmart-core/tests/`) run every business workflow against real SQLite databases. They cover product rules, SKU and barcode validation, sales calculations, discounts, stock deduction, invoice numbering, duplicate-sale prevention, transaction rollback, returns, voids, purchases and average cost, supplier balances, expenses, the profit and loss figures, cash closing, backup creation, corruption detection, restore, migrations and upgrades, authentication, and record immutability.

### End-to-end tests

```powershell
npm run test:e2e            # builds test copies of the app and the License Generator, then runs suites 1-3
```

`e2e/` drives the **real desktop application** through WebView2's debugging port (Playwright).

- `CODEXMART_DATA_DIR`, `CODEXMART_LICENSE_DIR` and `CXM_LICENSEGEN_DIR` point to temporary folders, so real shop data, license state and signing keys are never touched.
- The test copy is built with the `e2e` cargo feature: a shifted clock, a simulated computer and an extra trusted test key. The installer never includes it.
- Close any running CODE X MART first. Screenshots are saved for review.

- **`01-first-day.mjs`** — first-run setup and recovery key, products (generated EAN-13, batch tracking), duplicate-barcode rejection, cash sale with bill discount and change, receipt printing, split payment, credit sale, oversell refusal, sales history, partial return, profit and loss figures, stock ledger verification, customer balance, the scheduled automatic backup, and a check that no network requests leave the computer.
- **`02-restart-backup-restore.mjs`** — restart and persistence, wrong password, receiving goods with partial payment, supplier balance, expenses, cash closing slip, cashier account and role restrictions, backup and full restore (with safety backup), administrator reset with the recovery key, clean shutdown, and the damaged-database recovery screen (file left untouched, retry without restart).
- **`03-licensing.mjs`** covers the License Generator and the trial on the real app:
  - **Generator key setup:** short passphrase refused, key backup, lock and unlock.
  - **Trial days:** days 1 and 24 (no reminder), 25-30 (reminder once a day) and 31 (activation screen, backend refusal, backup still allowed).
  - **Activation:** request code, issuing in the generator, a typing mistake located by group, a forged key with valid checksums refused, an expired key refused, then activation with shop data intact.
  - **Afterwards:** restart, restore, clock set-back warning, motherboard replacement and transfer, lifetime license years later, history, CSV export, and key restore on another computer.
- **`04-installer.mjs`** tests the **production installer** and its uninstaller through UI Automation. Run it by hand: `node e2e/04-installer.mjs <setup.exe> <e2e app> <generator> <work> <safekeeping>`. It moves the computer's real CODE X MART data and registry entries aside first and restores them at the end.
  - **Production build:** test hooks ignored, license records in the right locations, a foreign signing key refused.
  - **Uninstall choices:** cancel and both "No" answers change nothing. "Keep my data" then reinstall gives the same login and data. "Delete all my data" with the app open, then reinstall, gives a new setup, refuses the old login and keeps the original trial.
  - **Activation with the real owner key.**
  - **Edge cases:** silent `/S` and `/S /DELETEDATA`, a locked database, a damaged database, a read-only backup, and day 31 after deletion.

## Release procedure

1. Update the version in `package.json`, `Cargo.toml` (`[workspace.package]`) and `src-tauri/tauri.conf.json`.
2. Add a section to `CHANGELOG.md`.
3. If the database changes, add a **new** migration file. Never edit a released migration: checksums protect them.
4. Run `npm run check`.
5. Run `npm run app:build` and install the produced setup file on a test computer over the previous version. Confirm that data is kept and a `pre-upgrade` backup was made.
6. Commit, tag (`git tag v1.0.1`) and push, then attach the installer to a GitHub release.

## Troubleshooting

| Problem | Solution |
|---|---|
| "Windows protected your PC" when installing | The installer is not code-signed. Click **More info → Run anyway**. |
| App opens to a "shop data could not be opened" screen | The database file is damaged or unreadable. It has not been changed. Use **Choose a backup file** to restore the latest `.cxmbak`. |
| Forgot the administrator password | On the sign-in screen click **Forgot the administrator password?** and enter the recovery key. |
| Account locked | Wait 5 minutes after 5 wrong passwords, or ask an administrator to reset the password. |
| "Not enough stock" when billing | Record the stock first (purchase or stock adjustment). Alternatively, an administrator can allow overselling in *Settings → Features & rules* (not recommended). |
| Scanner types into the wrong place | Most scanners need *keyboard (HID) mode* and an *Enter* suffix. Both are usually the default. |
| Receipt is cut off or too small | Pick the correct paper size and, in the Windows print dialog, set margins to *None* and scale to 100%. |
| Where are the log files? | *Settings → About → Open logs*. |
| "Your free trial has expired" | Send the request code to Hamad Mansha (+92 348 6825408) and enter the activation key: [docs/OFFLINE_ACTIVATION.md](docs/OFFLINE_ACTIVATION.md). Shop data is safe and backups still work. |
| An activation key is refused | See [docs/LICENSE_TROUBLESHOOTING.md](docs/LICENSE_TROUBLESHOOTING.md). |

## Future cloud architecture roadmap

Phase 2 will add an offline-first cloud ecosystem: a central PostgreSQL database (Neon), a secure sync API, an admin website, a public website, cloud image storage (ImageKit), and multiple authorized devices. Phase 1 is prepared for this:

- UUIDv7 identifiers for every record (globally unique across devices) and fixed IDs for seeded reference data.
- A per-database **device ID** and **database ID** in `app_meta`.
- UTC timestamps on every record, plus a separate local business date.
- Append-only financial documents and an append-only stock ledger. These are naturally conflict-free to synchronize.
- Idempotent operations keyed by client transaction IDs (the basis of retry-safe sync).
- Business rules in a UI-independent Rust crate that a server can reuse.
- A single typed API client in the UI (`src/lib/api`), so a web admin can reuse the screens with a different transport.

The decision on the synchronization outbox is documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#sync-outbox-decision). The full plan is in [docs/ROADMAP.md](docs/ROADMAP.md).

**CODE X MART works fully without any cloud service, and nothing in Phase 1 depends on one.**

## Current limitations

- One computer and one Windows user account. Data lives in that user's AppData folder.
- No sales tax or GST calculation on receipts.
- No hold or park bill feature, and no bulk CSV import of products.
- Billing is intentionally **image-free**: product tiles and search results show text only, for speed. The database has an optional `image_ref` field reserved for future use (for example the Phase 2 website), but there is no image upload screen.
- The interface is English only. Localization hooks exist (navigation text), but Urdu is not translated.
- Customer payments reduce the overall balance; they are not matched to specific invoices.
- A return to a supplier must be made against a recorded purchase invoice.
- One cash closing per business day.
- If overselling is enabled, stock cost differences on goods sold before they were received are absorbed rather than reported.
- Not tested on Windows 11 or with physical thermal printers or barcode scanners (see above).
- The installer is not code-signed.
- Offline licensing has limits:
  - Someone who studies the public source code and edits every copy of the trial records can cheat the trial.
  - Licenses cannot be revoked remotely.
  - Licenses themselves cannot be forged without the owner's private key. See [docs/LICENSE_SECURITY.md](docs/LICENSE_SECURITY.md).

## Documentation

| Document | For |
|---|---|
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Installing, updating, uninstalling |
| [docs/USER_GUIDE.md](docs/USER_GUIDE.md) | Shop owner and staff: daily use |
| [docs/BACKUP_AND_RESTORE.md](docs/BACKUP_AND_RESTORE.md) | Backups and disaster recovery |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Developers: design and principles |
| [docs/DATABASE.md](docs/DATABASE.md) | Developers: schema, migrations, integrity |
| [docs/LICENSING.md](docs/LICENSING.md) | Overview of the trial and licensing system |
| [docs/OFFLINE_ACTIVATION.md](docs/OFFLINE_ACTIVATION.md) | Shop owner: activating without internet |
| [docs/TRIAL_SYSTEM.md](docs/TRIAL_SYSTEM.md) | How the trial works and is protected |
| [docs/LICENSE_GENERATOR.md](docs/LICENSE_GENERATOR.md) | Software owner: issuing licenses |
| [docs/OWNER_KEY_MANAGEMENT.md](docs/OWNER_KEY_MANAGEMENT.md) | Software owner: signing key, backup, recovery, rotation |
| [docs/LICENSE_SECURITY.md](docs/LICENSE_SECURITY.md) | Cryptography, threat model and offline limits |
| [docs/DEVICE_BINDING.md](docs/DEVICE_BINDING.md) | Device id and license transfers |
| [docs/LICENSE_TROUBLESHOOTING.md](docs/LICENSE_TROUBLESHOOTING.md) | License problems and their fixes |
| [docs/UNINSTALLATION.md](docs/UNINSTALLATION.md) | Keep or delete data when uninstalling |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Future phases |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

© 2026 CODE X MART. All rights reserved.
