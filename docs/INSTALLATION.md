# Installing CODE X MART

## Requirements

| | Minimum |
|---|---|
| Windows | Windows 10 64-bit (version 1809 or newer) or Windows 11 64-bit |
| Processor / memory | Any computer that runs Windows 10 comfortably (4 GB RAM recommended) |
| Disk | About 30 MB for the program, plus space for data and backups (about 600 MB more if WebView2 has to be installed) |
| WebView2 Runtime | Already included in Windows 11 and in updated Windows 10. If it is missing, the installer adds it (see below). |
| Internet | **Not needed**, either to install or to use CODE X MART. |

## Install

1. Copy `CODE X MART_1.1.0_x64-setup.exe` (about 212 MB) to the shop computer, for example from a USB drive. This one file is all that is needed. Do not copy the project folder.
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

## WebView2 and computers without internet

CODE X MART displays its screens with the **Microsoft Edge WebView2 Runtime**, the same component Windows uses for many built-in apps. The setup file contains Microsoft's official offline WebView2 installer (Evergreen Standalone Installer, x64, signed by Microsoft):

- If WebView2 is already installed (normal on Windows 11 and updated Windows 10), setup skips this step.
- If it is missing, setup installs it silently before installing CODE X MART. No internet is needed. This step can take a few minutes, and Windows may ask for permission.

To check whether a computer already has it, look for *Microsoft Edge WebView2 Runtime* under *Settings → Apps*.

If WebView2 cannot be installed (for example, a locked-down computer), setup stops with an error and installs nothing. Ask the computer's administrator to install the WebView2 Runtime, then run setup again.

## Where things are installed

| What | Where |
|---|---|
| Program | `%LOCALAPPDATA%\CODE X MART\` (or the folder chosen during setup) |
| Shop data | `%APPDATA%\CODE X MART\data\codexmart.db` |
| Automatic backups | `%APPDATA%\CODE X MART\backups\` |
| Log files | `%LOCALAPPDATA%\com.codexmart.pos\logs\` |
| License record of this computer | `HKCU\Software\CODE X MART Licensing` and `CODE X MART Licensing` folders (see [TRIAL_SYSTEM.md](TRIAL_SYSTEM.md)) |

Program files and shop data are kept separate on purpose.

## Updating to a new version

1. Make a manual backup to a USB drive (*Backup & restore → Back up to USB / folder…*).
2. Close CODE X MART.
3. Run the new setup file. It replaces the program only.
4. Start CODE X MART. If the new version changes the database, it first saves a `pre-upgrade` backup automatically, then upgrades.

Your data, settings and users are kept.

## Free trial and activation

CODE X MART is free for 30 days from the first setup. From day 25 a reminder appears once a day; from day 31 the business screens need an activation key from the software owner (Hamad Mansha, +92 348 6825408). Activation works without internet: see [OFFLINE_ACTIVATION.md](OFFLINE_ACTIVATION.md).

## Uninstalling

Use *Windows Settings → Apps → CODE X MART → Uninstall*. The uninstaller asks what to do with your data:

- **Keep my data and credentials** (default): only the program is removed. Reinstall later and sign in as before.
- **Delete all my data and credentials**: after two confirmations, it also removes the shop database, accounts, settings, local backups, logs and cache. Backups on USB drives or other folders are not touched.

In both cases the computer's license record is kept, so reinstalling does not start a new free trial. Full details: [UNINSTALLATION.md](UNINSTALLATION.md).

## Moving to a new computer

See [BACKUP_AND_RESTORE.md](BACKUP_AND_RESTORE.md#moving-to-a-new-computer-or-after-reinstalling-windows).
