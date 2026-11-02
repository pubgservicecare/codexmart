//! Device identity.
//!
//! A computer is identified by a 10-byte device id derived from hardware
//! identifiers. Raw identifiers (serial numbers, UUIDs) never leave this
//! module: each one is reduced to a salted 64-bit hash, and only hashes are
//! stored or shown. The device id itself is a hash of those hashes.
//!
//! * The id is computed from the motherboard's SMBIOS identifiers (system
//!   UUID, board serial and system serial), so it stays the same when
//!   Windows is reinstalled or the disk is replaced.
//! * Values that are missing or are factory placeholders ("To be filled by
//!   O.E.M.", all zeros, ...) are ignored. A computer without usable
//!   SMBIOS identifiers falls back to the Windows installation id
//!   (MachineGuid), which changes when Windows is reinstalled.
//! * Once computed, the id is stored with the component hashes. Later
//!   starts keep the stored id while the hardware still matches it (see
//!   [`Fingerprint::same_device`]), so small changes such as a CPU upgrade
//!   or a BIOS update do not change the id. Replacing the motherboard does,
//!   and then the owner must issue a new license (controlled transfer).

use sha2::{Digest, Sha256};

pub const DEVICE_ID_LEN: usize = 10;
pub type DeviceId = [u8; DEVICE_ID_LEN];

/// Identifiers read from the computer. Every field is optional because
/// Windows or the firmware may not provide it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawIdentifiers {
    pub smbios_uuid: Option<[u8; 16]>,
    pub board_serial: Option<String>,
    pub system_serial: Option<String>,
    pub machine_guid: Option<String>,
    pub cpu: Option<String>,
    pub system_volume: Option<u32>,
}

/// Salted hashes of the identifiers; 0 means "not available".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub smbios_uuid: u64,
    pub board_serial: u64,
    pub system_serial: u64,
    pub machine_guid: u64,
    pub cpu: u64,
    pub system_volume: u64,
}

const PLACEHOLDER_UUIDS: [[u8; 16]; 3] = [
    [0; 16],
    [0xFF; 16],
    // Common BIOS placeholder 03000200-0400-0500-0006-000700080009.
    [0x03, 0x00, 0x02, 0x00, 0x04, 0x00, 0x05, 0x00, 0x00, 0x06, 0x00, 0x07, 0x00, 0x08, 0x00, 0x09],
];

const PLACEHOLDER_WORDS: [&str; 14] = [
    "O.E.M",
    "OEM",
    "DEFAULT",
    "SERIAL",
    "FILLED",
    "NOT SPECIFIED",
    "NOT APPLICABLE",
    "NONE",
    "N/A",
    "INVALID",
    "UNKNOWN",
    "123456789",
    "SYSTEM PRODUCT",
    "EMPTY",
];

/// Returns the identifier if it looks like a real, unique value.
pub fn usable_text(value: &str) -> Option<String> {
    let v = value.trim().to_ascii_uppercase();
    let significant: Vec<char> = v.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if significant.len() < 4 || PLACEHOLDER_WORDS.iter().any(|w| v.contains(w)) {
        return None;
    }
    if significant.iter().all(|&c| c == significant[0]) {
        return None;
    }
    Some(v)
}

fn component_hash(name: &str, value: &[u8]) -> u64 {
    let mut h = Sha256::new();
    h.update(b"CXM-COMPONENT-1\0");
    h.update(name.as_bytes());
    h.update(b"\0");
    h.update(value);
    let v = u64::from_be_bytes(h.finalize()[..8].try_into().unwrap());
    v.max(1)
}

impl Fingerprint {
    pub fn from_raw(raw: &RawIdentifiers) -> Self {
        let text = |name: &str, v: &Option<String>| {
            v.as_deref().and_then(usable_text).map(|t| component_hash(name, t.as_bytes())).unwrap_or(0)
        };
        Fingerprint {
            smbios_uuid: raw
                .smbios_uuid
                .filter(|u| !PLACEHOLDER_UUIDS.contains(u))
                .map(|u| component_hash("smbios-uuid", &u))
                .unwrap_or(0),
            board_serial: text("board-serial", &raw.board_serial),
            system_serial: text("system-serial", &raw.system_serial),
            machine_guid: raw
                .machine_guid
                .as_deref()
                .map(str::trim)
                .filter(|g| g.len() >= 8)
                .map(|g| component_hash("machine-guid", g.to_ascii_lowercase().as_bytes()))
                .unwrap_or(0),
            cpu: raw
                .cpu
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(|c| component_hash("cpu", c.as_bytes()))
                .unwrap_or(0),
            system_volume: raw
                .system_volume
                .filter(|&v| v != 0)
                .map(|v| component_hash("volume", &v.to_be_bytes()))
                .unwrap_or(0),
        }
    }

    fn smbios(&self) -> [u64; 3] {
        [self.smbios_uuid, self.board_serial, self.system_serial]
    }

    pub fn has_hardware_ids(&self) -> bool {
        self.smbios().iter().any(|&v| v != 0)
    }

    /// Device id computed from scratch: from the SMBIOS identifiers when
    /// available, otherwise from the Windows installation id, otherwise
    /// from the processor and system volume.
    pub fn derive_device_id(&self) -> DeviceId {
        let mut h = Sha256::new();
        h.update(b"CXM-DEVICE-1\0");
        if self.has_hardware_ids() {
            h.update(b"hardware\0");
            for v in self.smbios() {
                h.update(v.to_be_bytes());
            }
        } else if self.machine_guid != 0 {
            h.update(b"windows\0");
            h.update(self.machine_guid.to_be_bytes());
        } else {
            h.update(b"weak\0");
            h.update(self.cpu.to_be_bytes());
            h.update(self.system_volume.to_be_bytes());
        }
        h.finalize()[..DEVICE_ID_LEN].try_into().unwrap()
    }

    /// Whether `current` is still the computer this stored fingerprint was
    /// taken on. Requires at least one matching motherboard identifier and
    /// enough matching components overall, so a disk moved to another
    /// computer, or a new motherboard, counts as a different device.
    pub fn same_device(&self, current: &Fingerprint) -> bool {
        let eq = |a: u64, b: u64| a != 0 && a == b;
        if !self.has_hardware_ids() && !current.has_hardware_ids() {
            return eq(self.machine_guid, current.machine_guid);
        }
        let smbios_match = self.smbios().iter().zip(current.smbios()).any(|(&a, b)| eq(a, b));
        let weights = [
            (self.smbios_uuid, current.smbios_uuid, 2),
            (self.board_serial, current.board_serial, 2),
            (self.system_serial, current.system_serial, 1),
            (self.machine_guid, current.machine_guid, 2),
            (self.cpu, current.cpu, 1),
            (self.system_volume, current.system_volume, 1),
        ];
        let score: u32 = weights.iter().filter(|(a, b, _)| eq(*a, *b)).map(|(_, _, w)| w).sum();
        smbios_match && score >= 4
    }

    pub fn to_array(&self) -> [u64; 6] {
        [self.smbios_uuid, self.board_serial, self.system_serial, self.machine_guid, self.cpu, self.system_volume]
    }

    pub fn from_array(a: [u64; 6]) -> Self {
        Fingerprint {
            smbios_uuid: a[0],
            board_serial: a[1],
            system_serial: a[2],
            machine_guid: a[3],
            cpu: a[4],
            system_volume: a[5],
        }
    }
}

/// `7K92-4P6D-8W3H-5R2X`: the device id as 16 symbols in four groups.
pub fn format_device_id(id: &DeviceId) -> String {
    let symbols = crate::codec::encode_symbols(id, 16);
    symbols
        .chunks(4)
        .map(|c| c.iter().map(|&s| crate::codec::ALPHABET[s as usize] as char).collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc() -> RawIdentifiers {
        RawIdentifiers {
            smbios_uuid: Some([
                0x4A, 0x11, 0x9C, 0x02, 0x7E, 0x55, 0x41, 0xD0, 0x8B, 0x3F, 0x10, 0x22, 0x64, 0xA9, 0x0C, 0x77,
            ]),
            board_serial: Some("BSN12345678".into()),
            system_serial: Some("PF3XYZ12".into()),
            machine_guid: Some("6f1c2d3e-aaaa-bbbb-cccc-0123456789ab".into()),
            cpu: Some("GenuineIntel Intel(R) Core(TM) i5-8250U 806EA".into()),
            system_volume: Some(0x1234_ABCD),
        }
    }

    #[test]
    fn placeholders_are_ignored() {
        for v in ["To be filled by O.E.M.", "Default string", "System Serial Number", "0000000", "  ", "N/A", "xxxx"] {
            assert_eq!(usable_text(v), None, "{v}");
        }
        assert!(usable_text("BSN12345678").is_some());
        let mut raw = pc();
        raw.smbios_uuid = Some([0xFF; 16]);
        assert_eq!(Fingerprint::from_raw(&raw).smbios_uuid, 0);
    }

    #[test]
    fn device_id_is_stable_across_windows_reinstall() {
        let before = Fingerprint::from_raw(&pc());
        let mut raw = pc();
        raw.machine_guid = Some("11111111-2222-3333-4444-555555555555".into());
        raw.system_volume = Some(0x9999_0000);
        let after = Fingerprint::from_raw(&raw);
        assert_eq!(before.derive_device_id(), after.derive_device_id());
        assert!(before.same_device(&after));
    }

    #[test]
    fn small_changes_keep_the_device_but_a_new_motherboard_does_not() {
        let stored = Fingerprint::from_raw(&pc());
        let mut cpu_upgrade = pc();
        cpu_upgrade.cpu = Some("GenuineIntel Intel(R) Core(TM) i7-8700 906EA".into());
        assert!(stored.same_device(&Fingerprint::from_raw(&cpu_upgrade)));

        let mut new_board = pc();
        new_board.smbios_uuid = Some([42; 16]);
        new_board.board_serial = Some("OTHERBOARD99".into());
        new_board.system_serial = Some("OTHERSYS77".into());
        let nb = Fingerprint::from_raw(&new_board);
        assert!(!stored.same_device(&nb));
        assert_ne!(stored.derive_device_id(), nb.derive_device_id());
    }

    #[test]
    fn a_cloned_disk_on_another_computer_is_a_different_device() {
        let stored = Fingerprint::from_raw(&pc());
        let mut other = pc();
        other.smbios_uuid = Some([99; 16]);
        other.board_serial = Some("ANOTHERBOARD1".into());
        other.system_serial = Some("ANOTHERSYS2".into());
        // Same Windows installation and volume (disk image copied).
        assert!(!stored.same_device(&Fingerprint::from_raw(&other)));
    }

    #[test]
    fn computers_without_hardware_ids_use_the_windows_id() {
        let raw =
            RawIdentifiers { machine_guid: Some("6f1c2d3e-aaaa-bbbb-cccc-0123456789ab".into()), ..Default::default() };
        let fp = Fingerprint::from_raw(&raw);
        assert!(!fp.has_hardware_ids());
        assert!(fp.same_device(&fp));
        let other =
            RawIdentifiers { machine_guid: Some("00000000-aaaa-bbbb-cccc-999999999999".into()), ..Default::default() };
        assert!(!fp.same_device(&Fingerprint::from_raw(&other)));
    }

    #[test]
    fn device_id_format() {
        let id = Fingerprint::from_raw(&pc()).derive_device_id();
        let text = format_device_id(&id);
        assert_eq!(text.len(), 19);
        assert_eq!(text.matches('-').count(), 3);
    }
}
