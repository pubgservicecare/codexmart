# License Generator (software owner only)

The License Generator turns a customer's request code into a signed activation key. It is a **separate program** for the owner's computer. It is not part of the customer installer and cannot be reached from CODE X MART.

* Program: `CODE X MART License Generator.exe`, built with `npm run licensegen:build` into `target\release\`. It is a single file: copy it anywhere, for example onto the owner's USB drive. It needs the WebView2 runtime, which is already on Windows 10/11.
* Data folder: `%APPDATA%\CODE X MART License Generator\`, which holds:
  * `signing-key.dat`: the private key, encrypted;
  * `license-history.db`: the history of issued licenses.
* Works offline.

## First start

| What you see | What to do |
|---|---|
| **Choose your owner passphrase** | The signing key built into CODE X MART 1.1.0 was created on the development PC and is protected by your Windows account only. Choose a passphrase of at least 10 characters (a short sentence works well). |
| **Create a new signing key / Restore from a key backup** | No key on this computer yet. Restore your backup, or create a key; a new key also needs a new CODE X MART build, see [Owner key management](OWNER_KEY_MANAGEMENT.md). |
| **Unlock** | Enter your passphrase. The generator locks itself after 15 minutes without use, or when you click **Lock**. After 5 wrong passphrases it waits 30 seconds. |

Then open **Signing key → Save key backup…** and save the `.cxkey` file on a USB drive. The generator refuses to issue licenses until a backup has been saved.

## Issuing a license

1. **Issue license.** Paste the customer's request code (`CXMR-...`) and click **Check request**. The generator shows:
   * the computer's Device ID (compare it with what the customer sees, if in doubt);
   * the CODE X MART version, trial status and trial start;
   * any licenses already issued for that computer.
2. Fill in:
   * **Customer name or reference** (required) and phone;
   * **License type**: 1, 3 or 6 months, 1 year, Lifetime, 30-day trial extension, or Custom end date;
   * **Issue date**: today by default;
   * **Kind**: *New license*, *Renew or reissue for this computer*, or *Transfer to a new computer*;
   * **Internal notes**: stored only in your history, never in the key.
3. **Review license.** Check the customer, computer, type, dates and any warnings.
4. **Generate activation key.** Then use either button:
   * **Copy WhatsApp message**: a ready-to-send message with the license number, validity and the key split into lines;
   * **Copy activation key**: the key alone.

Each license gets a serial number like `CXMT-7K92-4P6D-8W3H-5R2X`. The customer also sees it under *Settings → License*.

### Rules the generator enforces

* A computer that already has an active license cannot get another *New license* by accident. Choose *Renew or reissue for this computer*: the earlier record is marked **superseded**. The customer's app keeps every valid key and uses the best one.
* *Transfer to a new computer* needs the new computer's request code and the license being moved. Start it from **License history → Transfer to new computer**: the old record is marked **transferred** and the new license keeps the same customer and type. CODE X MART cannot switch the old computer off remotely, because nothing is online. Agree with the customer that the old computer is no longer used.
* Request codes with a typing mistake, or that were edited, are refused.

## License history

* Search by name, phone, serial, device ID, notes or customer number.
* Click a row to see the full record, including the activation key (to send it again), the request code and the key fingerprint used.
* **Renew / reissue** and **Transfer to new computer** start a new license pre-filled from that record.
* **Export CSV** saves the whole history. The file contains customer names and phone numbers: keep it private. It contains no private key material.

## Signing key page

* Shows the key fingerprint, public key, creation date and last backup.
* Shows whether this CODE X MART version trusts the key (*Trusted by CODE X MART 1.1.0*). A red banner appears if the key is not built into this version.
* **Save key backup…** and **Change passphrase**. After changing the passphrase, save a new backup: old backups still open with the old passphrase.

## Customer number

The first time a customer name and phone combination is used, it gets a customer number (1, 2, 3…). The number is written into each license, so it can be matched to the history even though names are not in the key.

## Tests

* Unit tests in `tools/license-generator/src-tauri` cover:
  * key creation and wrong passphrases;
  * backup, restore on another computer, and tampered backups;
  * the pending key, and changing the passphrase;
  * issuing and verifying, and the backup requirement;
  * duplicate, reissue and transfer rules;
  * search and CSV export;
  * damaged request codes.
* End-to-end (`e2e/03-licensing.mjs`) drives the real generator:
  * create a key, lock and unlock;
  * issue, reissue and transfer from history;
  * export, and restore on a second data folder.
