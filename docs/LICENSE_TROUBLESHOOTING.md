# License troubleshooting

| Problem | What to do |
|---|---|
| "Typing mistake in group N" | Compare that group (5 characters) with the key received. Codes never contain 0, 1, I or O. |
| "This is a request code, not an activation key" | The code typed is the one to *send*. Enter the key received from the owner (it starts with `CXMA-`). |
| "…issued for a different computer" | The key belongs to another computer (or this computer's motherboard was replaced). Send this computer's request code to the owner; for a replacement computer the owner uses *Transfer*. |
| "…not genuine, or a character in it was changed" | Every group is typed correctly but the signature does not match. Check the key was copied completely from the owner's message; otherwise ask the owner to send it again (it is in the License History). |
| "…not signed by the CODE X MART license key" | The key was not made by the owner's License Generator, or it was signed with a newer owner key: update CODE X MART. |
| "This activation key expired on …" | The license period is over. Ask the owner for a renewal. |
| "This computer's date is earlier than the key's issue date" | Correct the date and time in Windows settings (*Settings → Time & language*), then activate again. |
| "Check date & time" chip, or "computer's date and time are earlier than when CODE X MART was last used" | The Windows clock was set back. Correct it. Repeated set-backs end the free trial early (licenses keep working). A weak CMOS battery that loses the date at every start must be replaced. |
| "The computer's date was set back several times, so the free trial ended early" | Ask the owner for a license (or a 30-day trial extension). |
| The trial or license looks expired after the clock was set far ahead by mistake | Correct the clock, send the request code to the owner and activate the new key. A newer license resets the stored time. |
| Activation screen after moving to a new computer | Expected: licenses are per computer. The owner transfers the license. |
| Activation screen after a motherboard repair | Same as a new computer: transfer. |
| The request code changed | It includes the status (trial or expired) and the trial start, so it changes when the status does. Send the newest one. |
| "Contact on WhatsApp" does nothing / shows a browser error | The computer has no internet. Call or message +92 348 6825408 from a phone. |
| Need to back up while activation is required | The activation screen has **Back up now** and **Back up to USB / folder…**. Shop data is never touched by the license check. |
| CODE X MART says it could not finish deleting old data | An earlier "Delete all my data" uninstall left files that were in use. Close programs that may use `%APPDATA%\CODE X MART` (backup or sync tools) and start again. See [Uninstalling](UNINSTALLATION.md). |

## For the owner

| Problem | What to do |
|---|---|
| Generator: "The generator is locked" | Enter the passphrase again (it locks after 15 minutes without use). |
| Generator: "Save a backup of your signing key first" | *Signing key → Save key backup…* |
| Generator: "Windows could not open the signing key" | The key file belongs to another Windows user or PC. Use **Restore from a key backup**. |
| Generator: red banner "not built into this version of CODE X MART" | Keys signed with this key are refused by customers. Use the original key (restore its backup), or follow *Replacing the key* in [Owner key management](OWNER_KEY_MANAGEMENT.md). |
| Customer lost their key | Find the license in **License history**. The activation key is in the details and can be sent again; it works again on the same computer. |
| Same computer needs a new key (renewal, upgrade to lifetime) | *Renew or reissue for this computer*. |

## Where to look

* **App logs:** `%LOCALAPPDATA%\com.codexmart.pos\logs`. They record how many state copies were found, set-backs, activations and refusals, but never keys or secrets.
* **License state copies:** see [Trial system](TRIAL_SYSTEM.md). Do not edit or delete them: that never helps, and edited copies are ignored.
