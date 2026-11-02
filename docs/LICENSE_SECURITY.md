# License security

## Building blocks

| Purpose | Choice | Library |
|---|---|---|
| License authenticity | Ed25519 signatures (`verify_strict`) | `ed25519-dalek` 2 |
| Private key encryption | Argon2id (64 MiB, 3 passes) + XChaCha20-Poly1305 | `argon2` 0.5, `chacha20poly1305` 0.10 |
| Private key at rest on the owner's PC | Windows DPAPI (current user) on top of the passphrase layer | Windows `CryptProtectData` |
| Trial record and request code tags | HMAC-SHA256 | `hmac`, `sha2` |
| Typing-error detection | Luhn mod 32 check symbol per group | own code (`codec.rs`), not a security feature |
| Randomness (keys, license ids, salts, nonces) | operating system RNG | `getrandom` |

No cryptography is invented here: only well-known primitives from maintained Rust crates.

## License format (version 1)

The payload is 21 bytes and fixed-length, so its serialization is canonical:

| Bytes | Field |
|---|---|
| 0 | format version (1) |
| 1 | product code (1 = CODE X MART) |
| 2 | signing key id |
| 3 | edition (low 4 bits) and flags (reissue, transfer) |
| 4-12 | license id (9 random bytes; shown as the serial `CXMT-XXXX-XXXX-XXXX-XXXX`) |
| 13-14 | first two bytes of the device id (only used to explain a refusal) |
| 15-16 | issue date (days since 2020-01-01) |
| 17-18 | last valid day, or 65535 for lifetime |
| 19-20 | customer number |

**Signed message** = `"CODE_X_MART\0LICENSE\0"`, then the version, the full 10-byte **device id**, then the 21 payload bytes. Every field is signed, including the product id and device. The activation key carries the payload and the 64-byte signature (85 bytes) as 136 symbols in 34 groups, each with a check symbol. The device id is not repeated in the key, because the customer's computer supplies its own. A key for another computer cannot verify.

The app rejects:

* a wrong or unknown version;
* a wrong product;
* malformed fields (lifetime without "no expiry", or an expiry before the issue date);
* a wrong device (hint, then signature);
* an unknown key id;
* any signature that does not verify strictly;
* expired licenses;
* an issue date more than a day ahead of the computer's clock.

**A correct check symbol never makes a key valid:** the signature is always checked. The test `a_valid_checksum_with_a_forged_signature_is_rejected`, and the forged key in `e2e/03-licensing.mjs`, prove it.

### Key length

An Ed25519 signature is 64 bytes, so a signed key cannot be as short as a bank card number. The 9-byte license id is shown in a card-like format (`CXMT-7K92-4P6D-8W3H-5R2X`, Luhn check symbol), but only the full activation key proves anything. To keep the long key usable, typing is checked group by group, so a mistake is located immediately.

## Request codes

20 bytes, written as `CXMR-` and 8 groups:

* version and product;
* the device id;
* the app version, trial start day and status flags;
* a 32-bit HMAC tag with a key built into the app.

The tag catches typing mistakes and casual edits, but it is **not a secret**: the key is in the public source. That is acceptable, because a request code grants nothing. The owner signs a license for the device id inside it, and only a computer with that device id can use the license. Editing a request code can only produce a license for some other computer.

## What is in the customer's app

* The **public** keys in `PRODUCTION_KEYS` (`crates/codexmart-license/src/keys.rs`). The unit test `production_keys_are_consistent` checks them.
* Verification code only. Signing code is behind the `issuer` cargo feature, which only the License Generator enables.
* No private key, no generator, no owner database.

The test hooks (shifted clock, isolated license folder, simulated device, extra trusted key) exist only with the `e2e` cargo feature. The installer is built without it. The production installer test checks that the installed app ignores these variables.

## Threat model

| Threat | Protection | Result |
|---|---|---|
| Forge or edit an activation key | Ed25519 signature over every field | Not feasible without the private key |
| Use one key on several computers | Device id is part of the signed message | Refused ("different computer") |
| Copy the license state to another computer | Licenses are verified against that computer's device id | Refused |
| Restore an old backup, reinstall, delete the data folder, create a new account | Earliest trial start across all copies, plus the setup time in the database | Trial not reset |
| Set the clock back | Trusted time never goes backwards; set-backs counted; trial ends after three | No days gained |
| Edit a state copy | HMAC tag | The copy is ignored |
| Steal the owner's key file | DPAPI (Windows account) and passphrase (Argon2id) | Useless without both, or without the backup **and** the passphrase |
| Someone who studies the public source and controls the PC | none (offline limit) | Can forge **trial** records, delete every copy, or patch the program. Cannot forge a **license**. |

## Offline limits (stated plainly)

* No offline system can stop a person with full administrator control from patching the program, faking hardware identifiers or deleting every record. The goal is to make cheating clearly more effort than buying a license, and to make license forgery impossible.
* Clock checks are relative: the app knows the latest time it has seen, not the true time.
* A license cannot be revoked remotely. A transferred license keeps working on the old computer until that computer's license state is removed. Transfers rely on the customer's honesty and the owner's records.
* The repository is public. Everything except the private key is visible, including the HMAC keys that tag trial records and request codes. This weakens the trial records only, not licenses.

## Privacy

* The device id is a hash of hashes. Serial numbers and UUIDs are never stored, shown or sent.
* State records contain no names, shop data or passwords. Request codes contain only the items listed above.
* Nothing is transmitted. The app has no network code and blocks nothing it would need to call home for. The "Contact on WhatsApp" button only opens `https://wa.me/923486825408` in the browser.
