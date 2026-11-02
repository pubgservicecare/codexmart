//! Reads the hardware identifiers used for the device id.
//!
//! Only these values are read: the SMBIOS system UUID, system and board
//! serial numbers (from the firmware table Windows exposes to every user),
//! the Windows installation id (MachineGuid), the processor model and the
//! system volume serial number. They are hashed immediately (see
//! `codexmart_license::device`); nothing is stored or sent in readable form.

use codexmart_license::device::{Fingerprint, RawIdentifiers};

pub fn read() -> Fingerprint {
    #[cfg(feature = "e2e")]
    if let Ok(seed) = std::env::var("CODEXMART_TEST_DEVICE") {
        // A simulated computer for testing device replacement: a different
        // seed is a different motherboard in the same Windows installation.
        let mut uuid = [0x5Au8; 16];
        for (i, b) in seed.bytes().take(16).enumerate() {
            uuid[i] = b;
        }
        return Fingerprint::from_raw(&RawIdentifiers {
            smbios_uuid: Some(uuid),
            board_serial: Some(format!("BOARD-{seed}")),
            system_serial: Some(format!("SYSTEM-{seed}")),
            machine_guid: Some("e2e-windows-installation".into()),
            cpu: Some("test cpu".into()),
            system_volume: Some(0x1234_5678),
        });
    }
    let raw = read_raw();
    log::info!(
        "Device identifiers available: smbios uuid {}, board serial {}, system serial {}, machine guid {}, cpu {}, volume {}",
        raw.smbios_uuid.is_some(),
        raw.board_serial.is_some(),
        raw.system_serial.is_some(),
        raw.machine_guid.is_some(),
        raw.cpu.is_some(),
        raw.system_volume.is_some()
    );
    Fingerprint::from_raw(&raw)
}

fn read_raw() -> RawIdentifiers {
    let (smbios_uuid, system_serial, board_serial) = sys::smbios_table().map(|t| parse_smbios(&t)).unwrap_or_default();
    RawIdentifiers {
        smbios_uuid,
        board_serial,
        system_serial,
        machine_guid: sys::machine_guid(),
        cpu: cpu_model(),
        system_volume: sys::system_volume_serial(),
    }
}

/// Extracts (system UUID, system serial, board serial) from raw SMBIOS
/// structures (the table without Windows' 8-byte header).
fn parse_smbios(table: &[u8]) -> (Option<[u8; 16]>, Option<String>, Option<String>) {
    let mut uuid = None;
    let mut system_serial = None;
    let mut board_serial = None;
    let mut p = 0;
    while p + 4 <= table.len() {
        let kind = table[p];
        let len = table[p + 1] as usize;
        if len < 4 || p + len > table.len() {
            break;
        }
        let formatted = &table[p..p + len];
        // Strings follow the formatted area, ending with a double NUL.
        let mut end = p + len;
        while end + 1 < table.len() && !(table[end] == 0 && table[end + 1] == 0) {
            end += 1;
        }
        let strings: Vec<&[u8]> = table[p + len..end].split(|&b| b == 0).collect();
        let string_at = |offset: usize| -> Option<String> {
            let index = *formatted.get(offset)? as usize;
            let s = strings.get(index.checked_sub(1)?)?;
            Some(String::from_utf8_lossy(s).trim().to_string()).filter(|s| !s.is_empty())
        };
        match kind {
            1 => {
                system_serial = string_at(0x07);
                if len >= 0x19 {
                    uuid = formatted[0x08..0x18].try_into().ok();
                }
            }
            2 if board_serial.is_none() => board_serial = string_at(0x07),
            127 => break,
            _ => {}
        }
        p = end + 2;
    }
    (uuid, system_serial, board_serial)
}

#[cfg(target_arch = "x86_64")]
#[allow(unused_unsafe)]
fn cpu_model() -> Option<String> {
    use std::arch::x86_64::__cpuid;
    let regs = |leaf: u32| unsafe { __cpuid(leaf) };
    let v = regs(0);
    let vendor: Vec<u8> = [v.ebx, v.edx, v.ecx].iter().flat_map(|r| r.to_le_bytes()).collect();
    let signature = regs(1).eax;
    let mut brand = Vec::new();
    if regs(0x8000_0000).eax >= 0x8000_0004 {
        for leaf in 0x8000_0002..=0x8000_0004u32 {
            let r = regs(leaf);
            brand.extend([r.eax, r.ebx, r.ecx, r.edx].iter().flat_map(|x| x.to_le_bytes()));
        }
    }
    let brand = String::from_utf8_lossy(&brand).trim_matches(char::from(0)).trim().to_string();
    Some(format!("{} {} {:08X}", String::from_utf8_lossy(&vendor), brand, signature))
}

#[cfg(not(target_arch = "x86_64"))]
fn cpu_model() -> Option<String> {
    None
}

#[cfg(windows)]
pub(crate) mod sys {
    use std::ptr::null_mut;

    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
    };
    use windows_sys::Win32::System::SystemInformation::{GetSystemFirmwareTable, GetWindowsDirectoryW};

    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// The raw SMBIOS structures, without Windows' 8-byte header.
    pub fn smbios_table() -> Option<Vec<u8>> {
        const RSMB: u32 = u32::from_be_bytes(*b"RSMB");
        unsafe {
            let size = GetSystemFirmwareTable(RSMB, 0, null_mut(), 0);
            if size < 8 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let got = GetSystemFirmwareTable(RSMB, 0, buf.as_mut_ptr().cast(), size);
            if got < 8 || got > size {
                return None;
            }
            let len = u32::from_le_bytes(buf[4..8].try_into().ok()?) as usize;
            buf.get(8..8 + len).map(|t| t.to_vec())
        }
    }

    pub fn reg_string(root: HKEY, subkey: &str, value: &str, flags: u32) -> Option<String> {
        let (k, v) = (wide(subkey), wide(value));
        unsafe {
            let mut size: u32 = 0;
            if RegGetValueW(root, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ | flags, null_mut(), null_mut(), &mut size) != 0
            {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
            let mut size = (buf.len() * 2) as u32;
            if RegGetValueW(
                root,
                k.as_ptr(),
                v.as_ptr(),
                RRF_RT_REG_SZ | flags,
                null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            ) != 0
            {
                return None;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..end]))
        }
    }

    pub fn machine_guid() -> Option<String> {
        reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Cryptography", "MachineGuid", RRF_SUBKEY_WOW6464KEY)
    }

    /// `C:\` (the drive Windows is installed on).
    pub fn system_drive_root() -> String {
        let mut buf = [0u16; 260];
        let n = unsafe { GetWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        let dir = String::from_utf16_lossy(&buf[..n.min(buf.len())]);
        match dir.get(..3) {
            Some(root) if root.ends_with(":\\") => root.to_string(),
            _ => "C:\\".to_string(),
        }
    }

    pub fn system_volume_serial() -> Option<u32> {
        let root = wide(&system_drive_root());
        let mut serial: u32 = 0;
        let ok = unsafe {
            GetVolumeInformationW(root.as_ptr(), null_mut(), 0, &mut serial, null_mut(), null_mut(), null_mut(), 0)
        };
        (ok != 0 && serial != 0).then_some(serial)
    }
}

#[cfg(not(windows))]
pub(crate) mod sys {
    pub fn smbios_table() -> Option<Vec<u8>> {
        None
    }
    pub fn machine_guid() -> Option<String> {
        std::fs::read_to_string("/etc/machine-id").ok().map(|s| s.trim().to_string())
    }
    pub fn system_volume_serial() -> Option<u32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structure(kind: u8, formatted: &[u8], strings: &[&str]) -> Vec<u8> {
        let mut s = vec![kind, (formatted.len() + 4) as u8, 0, 0];
        s.extend_from_slice(formatted);
        for text in strings {
            s.extend_from_slice(text.as_bytes());
            s.push(0);
        }
        if strings.is_empty() {
            s.push(0);
        }
        s.push(0);
        s
    }

    #[test]
    fn parses_system_and_board_identifiers() {
        // Type 1: manufacturer(1) product(2) version(3) serial(4) + UUID.
        let mut t1 = vec![1, 2, 3, 4];
        t1.extend_from_slice(&[0xAB; 16]);
        t1.extend_from_slice(&[0x06, 5, 6]); // wake-up type, SKU, family (unused)
        let mut table = structure(0, &[1, 2, 0, 0], &["BIOS vendor"]);
        table.extend(structure(1, &t1, &["Maker", "Model", "1.0", "SYS-SERIAL-9"]));
        table.extend(structure(2, &[1, 2, 3, 4], &["Maker", "Board", "v2", "BOARD-SERIAL-7"]));
        table.extend(structure(127, &[], &[]));
        let (uuid, system, board) = parse_smbios(&table);
        assert_eq!(uuid, Some([0xAB; 16]));
        assert_eq!(system.as_deref(), Some("SYS-SERIAL-9"));
        assert_eq!(board.as_deref(), Some("BOARD-SERIAL-7"));
    }

    #[test]
    fn truncated_tables_do_not_panic() {
        assert_eq!(parse_smbios(&[1, 40, 0]), (None, None, None));
        assert_eq!(parse_smbios(&[]), (None, None, None));
    }

    #[test]
    fn reads_this_computer() {
        // Works on any Windows PC; values may be missing on virtual machines.
        let fp = read();
        assert!(fp.to_array().iter().any(|&v| v != 0));
    }
}
