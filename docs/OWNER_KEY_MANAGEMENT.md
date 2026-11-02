# Owner signing key management

Every activation key is signed with the owner's **Ed25519 private key**. CODE X MART contains only the matching **public key**, which can check signatures but cannot make them. Whoever holds the private key can issue licenses, so it is protected like a bank card PIN.

## Where the key is

| Copy | Protection | Use |
|---|---|---|
| `%APPDATA%\CODE X MART License Generator\signing-key.dat` on the owner's PC | Owner passphrase (Argon2id + XChaCha20-Poly1305) **and** Windows DPAPI (only this Windows user on this PC can open the file) | Day-to-day issuing |
| `.cxkey` backup files (USB drives) | Owner passphrase (Argon2id + XChaCha20-Poly1305) | Recovery on another PC |

The private key is never:

* in the source code or on GitHub (`.gitignore` blocks `*.cxkey`, `signing-key.dat` and `license-history.db`);
* in the CODE X MART program or installer, the customer's database, or its backups;
* written to a log.

## The current key

| | |
|---|---|
| Key fingerprint | `RPF7-LC9Y-XCWJ-GJBM` |
| Key id | 105 |
| Public key | `67fc023d4c50c169cabe74cafe07879437f232e9d29d81ae7ce0e8ff8cf1301d` |
| Created | 2 Nov 2026, on the development PC, by `CODE X MART License Generator.exe --bootstrap-key` |
| Built into | CODE X MART 1.1.0 and later (`crates/codexmart-license/src/keys.rs`) |

The key was created on the owner's development PC so its public half could be built into the 1.1.0 installer. Until the owner opens the License Generator, it is protected **only by the owner's Windows account** (DPAPI), and the generator will not issue licenses with it. **First thing to do:**

1. Open the License Generator and choose your **owner passphrase**.
2. **Signing key → Save key backup…** to a USB drive. Save a second copy on another USB drive.
3. Write the passphrase on paper. Keep it apart from the USB drives.

Two lifetime licenses were issued with this key while testing the production installer: `CXMT-3PZ7-PHNS-M272-DZJ8` and `CXMT-Y75M-68NW-F94K-N7JJ`. Both are valid only on the development PC (device `JUBV-2PH4-PUYF-P3U9`), and their activation records were removed after the test. They are not in the generator's history.

If you would rather have a key that was never in this "Windows only" state, create a new key and rebuild (see *Replacing the key* below). Licenses signed with the current key keep working as long as its entry stays in `keys.rs`.

## Backup and recovery

* **Back up** with *Signing key → Save key backup…*. The file is checked after saving, and the date is shown. Keep two copies in different places.
* **New or repaired computer:**
  1. Install the License Generator.
  2. Choose **Restore from a key backup**, select the `.cxkey` file and enter the passphrase.
  3. The history is not inside the key backup. To keep it, also copy `license-history.db` from the old data folder, or keep a CSV export.
* **Forgotten passphrase:** the backup cannot be opened, and nobody can recover it; that is the point. If the generator on the old PC is still unlocked, use *Change passphrase* there. Otherwise treat it as a lost key.
* **Changing the passphrase:** use *Change passphrase*, then save a new backup. Old backups still use the old passphrase.

## Lost or stolen key

* **Lost (every copy gone):**
  * Customers who already activated keep working: their app only needs the public key, which is built in.
  * You can no longer issue licenses with that key. Create a new key and rebuild CODE X MART (see *Replacing the key*).
  * Customers who need a new license install the new version, then activate.
* **Stolen** (someone has a backup **and** the passphrase, or your unlocked PC): they could issue valid licenses. Create a new key and release a version whose `PRODUCTION_KEYS` no longer contains the stolen key. Then reissue licenses to genuine customers from your history: removing a key invalidates every license signed with it.

## Replacing the key (rotation)

The app trusts a list of public keys, never just one, so keys can be replaced without breaking existing licenses:

1. In the License Generator on a computer without a key (or with a fresh data folder, `CXM_LICENSEGEN_DIR`), choose **Create a new signing key** and save a backup.
2. Copy the public key shown under **Signing key** into a new `TrustedKey` entry in `crates/codexmart-license/src/keys.rs`:
   * `key_id` is the key id shown;
   * the label must contain the fingerprint (the unit test `production_keys_are_consistent` checks both).
3. **Keep every existing entry**, unless you are deliberately revoking a stolen key.
4. `cargo test --workspace`, then build the installer (`npm run app:build`) and the generator (`npm run licensegen:build`). The generator now shows *Trusted by CODE X MART*.

CODE X MART never changes its trusted keys by itself: a key can only be added or removed by a new build.
