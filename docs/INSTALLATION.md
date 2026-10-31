# Installing CODE X MART

## Requirements

| | Minimum |
|---|---|
| Windows | Windows 10 64-bit (version 1809 or newer) or Windows 11 64-bit |
| Processor / memory | Any computer that runs Windows 10 comfortably (4 GB RAM recommended) |
| Disk | About 30 MB for the program, plus space for data and backups |
| WebView2 Runtime | Already included in Windows 11 and in updated Windows 10 |
| Internet | **Not needed** for normal use. Only needed once if WebView2 is missing. |

## Install

1. Copy `CODE X MART_1.0.0_x64-setup.exe` to the shop computer (for example from a USB drive).
2. Double-click it.
3. If Windows shows **"Windows protected your PC"**, click **More info → Run anyway**. This appears because the installer is not code-signed. It does not mean the file is harmful.
4. Follow the installer. CODE X MART is installed for the current Windows user and does not need administrator rights.
5. Open **CODE X MART** from the Start menu or the desktop shortcut.

### First start

1. Enter the **store name**, phone, address and currency symbol (default `Rs`).
2. Create the **administrator** account (full name, username, password).
3. A **recovery key** is shown **once**. Write it on paper and keep it with the shop documents. It is the only way to reset a forgotten administrator password.
4. Sign in. Then, as administrator:
   - add staff accounts in *Settings → Users* (Cashier or Manager);
   - check receipt settings in *Settings → Receipt & printer* and click **Print test**;
   - set a USB drive or second disk for backup copies in *Backup & restore → Backup settings*.

### Use one Windows user account

CODE X MART keeps its data in the Windows user's AppData folder. Use the **same Windows user account** for everyone who works on the till. Each person signs in to CODE X MART with their own username.

## Computers without internet: WebView2

The installer includes a small WebView2 bootstrapper that needs internet **only if** WebView2 is not already installed. To check, look for *Microsoft Edge WebView2 Runtime* under *Settings → Apps*.

On a computer that never goes online and lacks WebView2:

1. On another computer, download the **Evergreen Standalone Installer (x64)** from Microsoft's WebView2 download page.
2. Copy it to the shop computer, run it, then run the CODE X MART installer.

## Where things are installed

| What | Where |
|---|---|
| Program | `%LOCALAPPDATA%\CODE X MART\` (or the folder chosen during setup) |
| Shop data | `%APPDATA%\CODE X MART\data\codexmart.db` |
| Automatic backups | `%APPDATA%\CODE X MART\backups\` |
| Log files | `%LOCALAPPDATA%\com.codexmart.pos\logs\` |

Program files and shop data are kept separate on purpose.

## Updating to a new version

1. Make a manual backup to a USB drive (*Backup & restore → Back up to USB / folder…*).
2. Close CODE X MART.
3. Run the new setup file. It replaces the program only.
4. Start CODE X MART. If the new version changes the database, it first saves a `pre-upgrade` backup automatically, then upgrades.

Your data, settings and users are kept.

## Uninstalling

Use *Windows Settings → Apps → CODE X MART → Uninstall*. **Uninstalling removes only the program.**

The uninstaller shows a **"Delete the application data"** checkbox, which is off by default. Ticking it removes only log files and the app's browser cache (`%LOCALAPPDATA%\com.codexmart.pos` and `%APPDATA%\com.codexmart.pos`). The shop data and backups in `%APPDATA%\CODE X MART` are **never** deleted by the uninstaller. To remove them as well, delete that folder by hand, after making sure you have a backup you no longer need.

## Moving to a new computer

See [BACKUP_AND_RESTORE.md](BACKUP_AND_RESTORE.md#moving-to-a-new-computer-or-after-reinstalling-windows).
