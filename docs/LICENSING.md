# Licensing overview

CODE X MART is free for 30 days. After that, a license from the software owner is needed to keep using the business screens. Everything works **offline**: the customer's computer never needs internet, an online account or an activation server.

| Who | Document |
|---|---|
| Shop owner (customer) | [Offline activation](OFFLINE_ACTIVATION.md), [Troubleshooting](LICENSE_TROUBLESHOOTING.md) |
| Software owner (license issuer) | [License Generator](LICENSE_GENERATOR.md), [Owner key management](OWNER_KEY_MANAGEMENT.md) |
| Developers | [Trial system](TRIAL_SYSTEM.md), [Device binding](DEVICE_BINDING.md), [License security](LICENSE_SECURITY.md) |
| Everyone | [Uninstalling](UNINSTALLATION.md) |

**Software owner:** Hamad Mansha, +92 348 6825408 (WhatsApp and phone). These details are defined once in `crates/codexmart-license/src/lib.rs`.

## The process in one picture

```text
 customer's computer (offline)                 owner's computer
 -----------------------------                 ----------------
 Day 1      shop set up -> 30-day trial starts
 Day 25-30  daily reminder: "expires in N days"
 Day 31     sign in -> activation screen
            "Request Activation Code"
            CXMR-XXXXX-... (8 groups)   ---- WhatsApp / phone / photo ---->  License Generator
                                                                               checks the request
                                                                               owner picks the license
                                                                               signs it (private key)
            Activation key                <--------- WhatsApp ------------   CXMA-XXXXX-... (34 groups)
            CXMA-XXXXX-... typed in
            checked with the built-in public key
            -> unlocked, offline, for this computer only
```

## License types

| Type | Valid |
|---|---|
| 30-day trial extension | 30 calendar days from the issue date |
| 1 month, 3 months, 6 months, 1 year | whole calendar months from the issue date (for example 1 Nov to 30 Nov) |
| Lifetime | no expiry date |
| Custom | until a date the owner chooses |

No prices are built into the software.

## What is implemented, tested and not tested

| Area | Status |
|---|---|
| 30-day trial from shop setup, reminders from day 25, activation screen from day 31 | Implemented. Tested automatically (unit tests, and end-to-end on the real app with a shifted clock). |
| Request code, activation key, Ed25519 signatures, Luhn mod 32 typing checks | Implemented. Tested (unit tests, forged and mistyped keys in the real app). |
| Owner License Generator (key setup, passphrase, backup/restore, issuing, history, export, reissue, transfer) | Implemented. Tested (unit tests and end-to-end on the real generator). |
| Trial protection: several authenticated copies, clock set-back detection, restore and reinstall do not reset | Implemented. Tested (unit tests, end-to-end, production installer). |
| Uninstall choice: keep or delete all data, license record kept | Implemented. Tested with the production installer on Windows 10 (see [Uninstalling](UNINSTALLATION.md)). |
| Activation with the real owner key on the production installer | Tested once on the development PC (Windows 10 Pro 22H2). |
| Windows 11, other hardware, virtual machines | **Not tested.** Expected to work; see [Device binding](DEVICE_BINDING.md) for computers without hardware identifiers. |
| A full Windows reinstall that wipes the system drive | **Not tested.** The design keeps the trial if another local disk holds a license copy or a backup is restored; see [Trial system](TRIAL_SYSTEM.md). |

## Honest limits

An offline program cannot be made impossible to bypass. Someone with full control of the computer who studies the program could edit or remove every local record, or change what the program sees. The signatures make it practically impossible to **forge a license** without the owner's private key. They cannot stop a determined person from cheating the **trial** on their own computer. Details: [License security](LICENSE_SECURITY.md).
