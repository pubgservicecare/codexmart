# Backup and restore

All CODE X MART data — products, stock, sales, customers, suppliers, expenses, users and settings — is stored in **one database file on this computer**. If the disk fails, the computer is stolen or Windows has to be reinstalled, a backup is the only way to get the data back.

## What a backup is

A backup is a single file named like `CODEXMART_manual_20261101-183000.cxmbak`. It contains:

- a complete, consistent snapshot of the database (taken safely while the app is running);
- a manifest: app version, database version, date, store name, record counts, and a SHA-256 checksum of the data.

After every backup, CODE X MART reads the file back and verifies it. A backup that fails verification is deleted and reported as failed. A broken file is never kept as if it were good.

The kind of backup appears in the file name:

| Kind | Made when | Deleted automatically? |
|---|---|---|
| `auto` | On a schedule, and when the app is closed | Yes: only the latest N (default 30) are kept |
| `manual` | You click *Back up now* or *Back up to USB / folder* | Never |
| `pre-restore` | Just before a restore replaces the data | Never |
| `pre-upgrade` | Just before a new version upgrades the database | Never |

## Automatic backups

*Backup & restore → Backup settings* (administrator):

- **Automatic backups** — on by default, every 24 hours while CODE X MART is open (choose 1–48 hours). The first automatic backup runs about 30 seconds after start-up if one is due.
- **Back up when the app is closed** — on by default. It is skipped if a backup was made in the last 10 minutes.
- **Keep automatic backups** — how many recent automatic backups to keep (default 30).
- **Also copy automatic backups to** — a USB flash drive or a second disk (for example `D:\CODEXMART-Backups`). Every automatic backup is copied there whenever it is connected. Missed copies are shown in the history.

Default backup folder: `%APPDATA%\CODE X MART\backups`. This folder is on the **same disk** as the data, so it does not protect against disk failure. That is why the page shows a warning until a second drive is configured.

## Good practice

1. Configure the **second copy** folder on a USB drive or another disk.
2. At least once a week, plug in a USB drive and click **Back up to USB / folder…**, then take the USB drive home.
3. Check *Backup & restore* sometimes: "Last successful backup" should be recent.

## Restoring a backup

Restoring **replaces all current data** with the data in the backup. Everything recorded after the backup was made is replaced. Only an **administrator** can restore.

1. Go to *Backup & restore* and click **Restore from backup…**, or click **Restore…** next to a file in the list.
2. Choose the `.cxmbak` file.
3. CODE X MART checks the file: checksum, database integrity, that it belongs to CODE X MART, and that its version is supported. If any check fails, nothing is changed.
4. Read the summary (date made, store name, record counts) and type `RESTORE` to confirm.
5. CODE X MART:
   - saves a **pre-restore** backup of the current data;
   - replaces the database file safely (written to a temporary file first, then swapped in);
   - reopens and, if needed, upgrades the restored database;
   - signs everyone out.
6. Sign in with a username and password **from the backup**.

If anything goes wrong during the swap, the previous database file is still in place and is reopened.

## If the data cannot be opened

If the database file is damaged, CODE X MART shows **"The shop data could not be opened"** instead of the normal sign-in. The damaged file is **not** changed or deleted. From that screen:

1. Click **Choose a backup file (.cxmbak)** and pick the newest backup (from `%APPDATA%\CODE X MART\backups` or your USB drive).
2. Wait for the check to pass, type `RESTORE`, and click **Restore backup**.
3. A copy of the damaged file is kept in the backups folder as `damaged-database-<date>.db`, for professional recovery if needed.

## Moving to a new computer or after reinstalling Windows

**Before** reinstalling Windows or retiring the old computer:

1. In CODE X MART click **Back up to USB / folder…** and choose the USB drive.
2. Copy the whole `%APPDATA%\CODE X MART\backups` folder to the USB drive as well (extra safety).

**On the new or reinstalled computer:**

1. Install CODE X MART (see [INSTALLATION.md](INSTALLATION.md)).
2. The setup screen appears. Enter any store name and create a **temporary** administrator. This account is replaced by the restore.
3. Go to **Backup & restore → Restore from backup…** and choose the newest `.cxmbak` file from the USB drive.
4. Type `RESTORE` and confirm.
5. Sign in with your **usual** username and password from the backup.
6. Set up the backup settings again if the USB drive letter has changed.

## Forgotten administrator password

On the sign-in screen click **Forgot the administrator password?**, enter the recovery key from the first setup, the administrator username and a new password. A new recovery key can be generated in *Settings → Security & health* (the old one stops working).

## Technical notes

- Snapshot method: SQLite `VACUUM INTO`, which is consistent while the app is running.
- Archive: ZIP containing `manifest.json` and `codexmart.db` (deflate).
- Verification before restore: SHA-256 and size match the manifest, `PRAGMA integrity_check`, `PRAGMA foreign_key_check`, the `app_meta.app` marker, and a schema version no newer than the app.
- History: `%APPDATA%\CODE X MART\backup-history.jsonl`. It is stored outside the database so restoring an older backup keeps the full history.
