//! Public keys that CODE X MART trusts for license signatures.
//!
//! Only public (verification) keys belong here. The matching private keys
//! stay on the owner's computer inside the License Generator, protected by
//! Windows (DPAPI) and the owner's passphrase. They are never part of this
//! repository, the customer installer or the customer's database.
//!
//! To add a key (see docs/OWNER_KEY_MANAGEMENT.md): copy the public key
//! shown under *Signing key* in the License Generator into a new entry,
//! keep every existing entry so licenses already issued stay valid, and
//! rebuild the installer.

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedKey {
    pub key_id: u8,
    pub public_key: [u8; 32],
    pub label: &'static str,
}

/// Keys trusted by every build of CODE X MART. Never remove an entry: every
/// license signed with it would stop working.
pub const PRODUCTION_KEYS: &[TrustedKey] = &[
    // Owner key 1, created 2026-11-02 in the License Generator.
    // Fingerprint RPF7-LC9Y-XCWJ-GJBM; public key
    // 67fc023d4c50c169cabe74cafe07879437f232e9d29d81ae7ce0e8ff8cf1301d
    TrustedKey {
        key_id: 105,
        public_key: [
            0x67, 0xfc, 0x02, 0x3d, 0x4c, 0x50, 0xc1, 0x69, 0xca, 0xbe, 0x74, 0xca, 0xfe, 0x07, 0x87, 0x94, 0x37, 0xf2,
            0x32, 0xe9, 0xd2, 0x9d, 0x81, 0xae, 0x7c, 0xe0, 0xe8, 0xff, 0x8c, 0xf1, 0x30, 0x1d,
        ],
        label: "Owner key 1 (RPF7-LC9Y-XCWJ-GJBM)",
    },
];

/// Short key identifier stored in each license: derived from the public
/// key, so no numbering has to be coordinated when a key is replaced.
pub fn key_id_for(public_key: &[u8; 32]) -> u8 {
    Sha256::digest([b"CXM-KEY-ID-1\0".as_slice(), public_key].concat())[0]
}

/// `XXXX-XXXX-XXXX-XXXX` fingerprint of a public key, for comparing keys by eye.
pub fn key_fingerprint(public_key: &[u8; 32]) -> String {
    let digest = Sha256::digest([b"CXM-KEY-FP-1\0".as_slice(), public_key].concat());
    crate::device::format_device_id(&digest[..10].try_into().unwrap())
}

pub fn is_production_key(public_key: &[u8; 32]) -> bool {
    PRODUCTION_KEYS.iter().any(|k| &k.public_key == public_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_keys_are_consistent() {
        assert!(!PRODUCTION_KEYS.is_empty());
        for k in PRODUCTION_KEYS {
            assert_eq!(key_id_for(&k.public_key), k.key_id, "{}", k.label);
            assert!(ed25519_dalek::VerifyingKey::from_bytes(&k.public_key).is_ok(), "{}", k.label);
            assert!(k.label.contains(&key_fingerprint(&k.public_key)), "{}", k.label);
        }
    }
}
