# Device binding

Each license works on one computer: the one whose request code was used.

## Device id

The device id is 10 bytes, shown as `7K92-4P6D-8W3H-5R2X`. It is derived (`crates/codexmart-license/src/device.rs`, read by `src-tauri/src/licensing/fingerprint.rs`) from:

| Identifier | Source | Changes when |
|---|---|---|
| System UUID | SMBIOS type 1 (firmware table readable by every user) | the motherboard is replaced |
| Board serial | SMBIOS type 2 | the motherboard is replaced |
| System serial | SMBIOS type 1 | the motherboard is replaced (sometimes a BIOS update) |
| Windows installation id | `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` | Windows is reinstalled |
| Processor model | CPUID | the CPU is replaced |
| System volume serial | `GetVolumeInformation` on the Windows drive | the drive is formatted |

Rules:

* **Not used:** the computer name, Windows user name, MAC addresses and IP addresses. They are too easy to change.
* Factory placeholder values such as "To be filled by O.E.M.", "Default string" or all-zero UUIDs are ignored.
* Each identifier is reduced to a salted 64-bit hash immediately. The device id is a hash of the motherboard hashes, so it stays the same when Windows is reinstalled or the disk replaced.
* Computers without usable SMBIOS identifiers (some virtual machines and cheap boards) fall back to the Windows installation id. On those computers, a Windows reinstall needs a license transfer.

## Locally protected installation identity

On first start the device id, the component hashes and a random installation id are stored in the authenticated state record ([Trial system](TRIAL_SYSTEM.md)). Later starts keep the stored id while the hardware still matches it:

* at least one motherboard identifier is unchanged, **and**
* enough components match overall: the weights are UUID 2, board serial 2, Windows id 2, system serial 1, CPU 1, volume 1, and at least 4 points are needed.

| Change | Same device? |
|---|---|
| Windows reinstalled, disk replaced, drive formatted | Yes (motherboard ids unchanged) |
| CPU upgrade, RAM, graphics card | Yes |
| BIOS update that rewrites one serial | Yes |
| Motherboard replaced | **No**: transfer needed |
| Disk (with Windows) moved to another computer | **No**: other motherboard |
| Whole new computer | **No**: transfer needed |

## Transferring a license (new computer or motherboard)

1. On the new computer, install CODE X MART and restore the shop backup if wanted. The license from the old computer does not apply here, so the activation screen (or the trial) shows.
2. The customer sends the new **request code**.
3. Owner: in **License history**, open the customer's license and choose **Transfer to new computer**. Paste the new request code, review and generate. The old record is marked *transferred*. The new license keeps the type and customer.
4. The customer enters the new key.

The old computer is not switched off automatically, because nothing is online. Agree with the customer that it is no longer used. A license never moves to another device by itself, and the app never authorizes extra devices without a new signed key.

## Tests

* Unit tests (`device.rs`) cover:
  * placeholders ignored;
  * the id stays the same across a Windows reinstall;
  * a CPU upgrade keeps the device, a new motherboard does not;
  * a cloned disk on another computer is a different device;
  * the fallback for computers without hardware ids;
  * parsing a synthetic SMBIOS table;
  * reading this PC's identifiers.
* End-to-end (`e2e/03-licensing.mjs`) simulates a motherboard replacement:
  * the old key is refused as "issued for a different computer";
  * the new device id differs;
  * the owner transfers the license from history and the new computer activates.
* Real hardware: tested only on the development PC. Other models, virtual machines and Windows 11 are **not tested**.
