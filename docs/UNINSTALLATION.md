# Uninstalling CODE X MART

Open *Settings → Apps → Installed apps*, find **CODE X MART**, then **Uninstall** (or run `uninstall.exe` in the program folder). The first page asks what should happen to your data:

| Choice | Meaning |
|---|---|
| **Keep my data and credentials** (selected by default) | "Your shop database, products, sales, purchases, customers, suppliers, login credentials, settings, backups and application data will remain on this computer. You can reinstall CODE X MART later and continue using your existing account and shop data." |
| **Delete all my data and credentials** | "This will permanently remove your CODE X MART shop data, login account, credentials, settings and locally stored backups from this computer. After reinstalling, you will need to create a new owner/admin account and set up the software again." |

Choosing *Delete* asks twice:

1. "Are you sure you want to permanently delete all CODE X MART data and credentials?"
2. A final confirmation.

Answering **No** to either, or **Cancel** on any page, changes nothing. If CODE X MART is open, the uninstaller offers to close it first; files are deleted only after it has closed.

## What is deleted and what is kept

| Data | Location | Keep | Delete |
|---|---|---|---|
| Program files, shortcuts, *Installed apps* entry | `%LOCALAPPDATA%\CODE X MART\` (or the chosen folder) | removed | removed |
| Shop database: products, stock, sales, invoices, returns, purchases, suppliers, customers, credit, expenses, cash closings, reports, audit log | `%APPDATA%\CODE X MART\data\` | kept | **deleted** |
| Login accounts, password hashes, recovery key, settings | inside the database | kept | **deleted** |
| Local backups and backup history | `%APPDATA%\CODE X MART\backups\`, `backup-history.jsonl` | kept | **deleted** |
| Temporary files | `%APPDATA%\CODE X MART\temp\` | kept | **deleted** |
| Log files, WebView2 cache and local storage | `%LOCALAPPDATA%\com.codexmart.pos\`, `%APPDATA%\com.codexmart.pos\` | kept | **deleted** |
| Backup copies on a USB drive or another folder you chose | wherever you saved them | kept | **kept** (never touched) |
| This computer's license record | `HKCU\Software\CODE X MART Licensing`, `…\CODE X MART Licensing\` folders, `X:\.codexmart-licensing\` | kept | **kept** |

### Why the license record stays

The license record is a minimal device record:

* installation and device ids;
* hashed hardware identifiers;
* the original trial start and the last time seen;
* activated licenses.

It contains no shop records, customer information, names, login credentials, password hashes or backups. It is kept so that uninstalling and reinstalling never starts a new 30-day free trial, and so that a paid license still works after reinstalling. Creating a new owner account after *Delete* does not start a new trial either.

Removing those records by hand would make a new free trial possible. The uninstaller deliberately does not offer that.

## After reinstalling

| Previous choice | What you see |
|---|---|
| Keep | The sign-in screen. Sign in with your existing username and password; all shop data, settings and backups are there. Trial or license status is unchanged. |
| Delete | The first-run setup. Create a new owner/admin account and a new recovery key, and the database starts empty. Old accounts no longer exist and old records are not restored automatically. The trial continues from its **original** start date (or the license stays active). |

## Files in use

If a file cannot be deleted because another program is using it (for example a backup or sync tool), the uninstaller:

* tries again for a few seconds;
* lists the folders it could not delete, and shows a message (exit code 2 for silent uninstalls);
* leaves a `DELETE-PENDING.txt` marker in `%APPDATA%\CODE X MART`. If CODE X MART is installed again, it finishes deleting that folder before opening anything, so old accounts are never reused. If the folder still cannot be deleted, the app shows a message instead of opening the old data.

Read-only files (for example a backup marked read-only) are deleted too.

## Silent uninstall (IT use)

| Command | Effect |
|---|---|
| `uninstall.exe /S` | Keep my data |
| `uninstall.exe /S /DELETEDATA` | Delete all my data and credentials, without questions |
| Updates (`/UPDATE`, run by the installer) | Never delete data |

## Tested

Tested with the production installer on Windows 10 Pro 22H2, driving the real uninstaller windows through UI Automation (`e2e/04-installer.mjs`).

**Cancelling never deletes:**

* Cancel on the data page;
* "No" at the first confirmation;
* "No" at the second confirmation.

**Keep my data:**

* the program is removed while the database, backups and license record remain;
* after reinstalling, the same login works, with the products, sale, settings and backup back;
* the trial start is unchanged.

**Delete all my data, with CODE X MART open:**

* the app is closed first;
* the data folder, logs and WebView2 profile are removed;
* the license record is kept with the original trial start;
* after reinstalling: the first-run setup appears, the old login is refused, the database is empty and there are no old backups;
* the new account does not restart the trial.

**Other cases:**

* silent `/S` keeps the data; silent `/S /DELETEDATA` deletes it;
* a license stays active after "Delete";
* a locked database gives exit code 2 and leaves the marker, and the next start finishes the deletion;
* a damaged database and a read-only backup are deleted;
* day 31 after "Delete all my data" and a new owner account still requires activation (clock simulated with the test build);
* seven installs and eight uninstalls in a row on the same computer, all 36 checks passing.

**Not tested:** Windows 11, uninstalling as a different Windows user, and a system restart (the marker covers it).
