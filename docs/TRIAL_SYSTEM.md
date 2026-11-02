# Free trial

## Rules

| Day | What the customer sees |
|---|---|
| Before setup | Nothing: the trial has not started. |
| 1-24 | Full access, no reminders. *Settings → License* shows the trial status for anyone who looks. |
| 25-30 | Full access. After signing in, a reminder opens **once a day** ("Your free trial expires in 5 days (on Sun, 30 Nov 2026). Contact Hamad Mansha to activate CODE X MART.") and an amber chip in the header shows the days left. Billing is never interrupted. |
| 31 and later | The app opens and staff can sign in. Then the **activation screen** replaces the business screens until a valid key is entered. Backups, sign-out and the request code still work. Nothing is deleted. |

The trial is **30 calendar days in the shop's local time**, starting on the day the shop is set up (the first administrator account is created), not when the installer is downloaded or first opened. If setup happens on 2 Nov, the last trial day is 1 Dec and activation is needed from 2 Dec. "Days left" counts the days after today (day 25 shows 5, day 30 shows "today").

The same reminder rules apply to time-limited licenses: during the last 5 days before a license expires.

## Enforcement

The decision is made in the Rust backend (`src-tauri/src/licensing/mod.rs`), not in the screen:

* every business command goes through `AppState::run`, which refuses with `LICENSE_REQUIRED` when access is not allowed;
* allowed at all times: sign-in and sign-out, first-run setup, the license commands, own password change, store settings (read), backup status, backups, backup check and restore, and opening the data folder.

When access ends, no data is deleted, hidden or encrypted. Activating a license immediately reopens everything.

## How the trial is remembered

The state is a small binary record:

* installation id and device id;
* hashed hardware identifiers;
* trial start, last time seen and number of clock set-backs;
* activated licenses.

Each copy carries an HMAC-SHA256 tag. Copies with a wrong tag are ignored. The record is kept in several places:

| Copy | Purpose |
|---|---|
| `HKCU\Software\CODE X MART Licensing` (value `State`) | survives uninstall and deleted folders |
| `%APPDATA%\CODE X MART Licensing\state.dat` | survives uninstall |
| `%LOCALAPPDATA%\CODE X MART Licensing\state.dat` | survives uninstall |
| `%ProgramData%\CODE X MART Licensing\state.dat` | survives uninstall; readable by other Windows users of the PC |
| `X:\.codexmart-licensing\state.dat` on up to four other fixed local disks | can survive a Windows reinstall that only formats the system drive |
| the shop database (`app_meta.license_state`) | travels with backups |

Every folder holds a `README.txt` that explains what it is. None of these copies contains shop data, names, passwords or anything personal. The uninstaller never removes them, even with "Delete all my data and credentials" ([Uninstalling](UNINSTALLATION.md)).

On every start the app reads all copies and merges them conservatively. Then it writes the merged record back to every copy, which also repairs copies that were deleted. Copies are refreshed every 5 minutes and at exit:

* **trial start:** the earliest start found anywhere, and never later than the shop's setup time stored in the database;
* **last time seen and set-back count:** the latest value found;
* **licenses:** every signed license found, each verified again.

So the trial does **not** restart when:

| Event | Why |
|---|---|
| the app is updated, uninstalled and reinstalled | the copies outside the program folder remain |
| the shop data is deleted (including "Delete all my data") | the copies outside the data folder remain |
| a new administrator account is created | the earliest trial start wins |
| an old backup is restored | its older copy merges with the newer ones, and the earliest start wins |
| another Windows user on the same PC opens it | the ProgramData copy is shared |
| the shop data is moved to a new computer | the database copy and the setup time come along |
| Windows is reinstalled on the system drive only | a copy on another local disk (for example D:) remains, and the device id is unchanged ([Device binding](DEVICE_BINDING.md)) |

## Clock tampering

The app uses a trusted time: the latest of the Windows clock, the last time seen, the time since start-up (a monotonic clock) and the release date. Setting the clock back therefore never returns trial days. When the clock is more than one hour behind the last time seen, at start-up or while running:

* the set-back is counted and a warning asks the user to correct the date and time;
* after **three set-backs the free trial ends early** (licenses are not affected).

A clock set far ahead by mistake would make the trial or a time-limited license look expired. A new license from the owner fixes this. The owner's signed issue date confirms today's date, and a newer, never-seen license resets the stored time. The set-back count is cleared at the same time.

## Limits (offline)

These are inherent to software that never goes online:

* The HMAC key that tags the copies is inside the program, and the source code is public. Someone who studies it can forge copies; the tags only stop casual editing.
* Someone who deletes every copy (registry, all folders, all disks) **and** the shop data, or reinstalls Windows on a single-disk PC without restoring a backup, gets a new trial. They also lose their shop data, which is a strong deterrent for a real shop.
* A computer whose clock is kept permanently wrong gains at most the hours the app actually runs, until the third detected set-back.
* Licenses are protected by signatures and cannot be forged this way. See [License security](LICENSE_SECURITY.md).

## Tests

* Unit tests in `crates/codexmart-license/src/state.rs` and `src-tauri/src/licensing/mod.rs`. They cover: days 1-24 without reminders, days 25-30 with the correct days left, expiry on day 31, local calendar days, edited copies ignored, copies deleted, new database, restore, clock set-back, repeated set-backs, activation, restart, wrong device, expired and lifetime licenses.
* End-to-end (`e2e/03-licensing.mjs`) on the real app with the clock shifted. It checks days 1, 24, 25, 26, 29, 30 and 31, and that the reminder does not repeat on the same day.
* Production installer (`e2e/04-installer.mjs`). It checks that uninstall (keep or delete) and reinstall keep the original trial start, and that day 31 after "Delete all my data" plus a new account still requires activation.
