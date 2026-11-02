//! Signed licenses and their activation keys.
//!
//! A license is a fixed-length binary payload (format version 1, 21 bytes)
//! signed with the owner's Ed25519 private key. The signature covers a
//! canonical message made of a domain string naming the product, the full
//! 10-byte device id of the licensed computer, and every payload field. The
//! activation key the customer types is the payload plus the 64-byte
//! signature, written as 34 groups of five symbols:
//!
//! ```text
//! CXMA-XXXXX-XXXXX-... (34 groups; the 5th symbol of each group is a check)
//! ```
//!
//! The full device id is not repeated inside the key: the customer's
//! computer supplies its own id when it checks the signature, so a key
//! issued for another computer cannot verify. A 2-byte device hint inside
//! the payload only lets the app explain *why* a key was refused.

use chrono::{Duration, Months, NaiveDate};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Serialize;

use crate::codec::{self, CodeError};
use crate::device::DeviceId;
use crate::keys::TrustedKey;

pub const PRODUCT_ID: &str = "CODE_X_MART";
pub const PRODUCT_CODE: u8 = 1;
pub const FORMAT_VERSION: u8 = 1;
pub const PAYLOAD_LEN: usize = 21;
pub const SIGNATURE_LEN: usize = 64;
pub const TOKEN_LEN: usize = PAYLOAD_LEN + SIGNATURE_LEN;
pub const LICENSE_ID_LEN: usize = 9;
/// `expiry_day` value of a license that never expires.
pub const PERPETUAL: u16 = u16::MAX;
const TOKEN_SYMBOLS: usize = 136;

/// Raw activation key: payload followed by the signature.
pub type Token = [u8; TOKEN_LEN];

/// Payload flag: this license replaces an earlier one for the same computer.
pub const FLAG_REISSUE: u8 = 0b0001;
/// Payload flag: this license moves an earlier one to a new computer.
pub const FLAG_TRANSFER: u8 = 0b0010;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Edition {
    Trial30,
    Month1,
    Month3,
    Month6,
    Year1,
    Lifetime,
    Custom,
}

impl Edition {
    pub const ALL: [Edition; 7] = [
        Edition::Trial30,
        Edition::Month1,
        Edition::Month3,
        Edition::Month6,
        Edition::Year1,
        Edition::Lifetime,
        Edition::Custom,
    ];

    fn to_u8(self) -> u8 {
        match self {
            Edition::Trial30 => 1,
            Edition::Month1 => 2,
            Edition::Month3 => 3,
            Edition::Month6 => 4,
            Edition::Year1 => 5,
            Edition::Lifetime => 6,
            Edition::Custom => 7,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        Edition::ALL.into_iter().find(|e| e.to_u8() == v)
    }

    pub fn code(self) -> &'static str {
        match self {
            Edition::Trial30 => "trial30",
            Edition::Month1 => "month1",
            Edition::Month3 => "month3",
            Edition::Month6 => "month6",
            Edition::Year1 => "year1",
            Edition::Lifetime => "lifetime",
            Edition::Custom => "custom",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Edition::ALL.into_iter().find(|e| e.code() == code)
    }

    pub fn label(self) -> &'static str {
        match self {
            Edition::Trial30 => "30-day trial",
            Edition::Month1 => "1 month",
            Edition::Month3 => "3 months",
            Edition::Month6 => "6 months",
            Edition::Year1 => "1 year",
            Edition::Lifetime => "Lifetime",
            Edition::Custom => "Custom period",
        }
    }

    pub fn is_perpetual(self) -> bool {
        self == Edition::Lifetime
    }

    /// Last valid day for a license issued on `issued`: 30 calendar days
    /// for the trial edition, whole calendar months for the others. Lifetime
    /// licenses have no expiry; custom licenses need an explicit date.
    pub fn default_expiry(self, issued: NaiveDate) -> Option<NaiveDate> {
        let months = |n: u32| issued.checked_add_months(Months::new(n)).map(|d| d - Duration::days(1));
        match self {
            Edition::Trial30 => Some(issued + Duration::days(29)),
            Edition::Month1 => months(1),
            Edition::Month3 => months(3),
            Edition::Month6 => months(6),
            Edition::Year1 => months(12),
            Edition::Lifetime | Edition::Custom => None,
        }
    }
}

/// Day numbers count from 2020-01-01 (day 0).
pub fn day_epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()
}

pub fn day_number(date: NaiveDate) -> Option<u16> {
    let days = (date - day_epoch()).num_days();
    (0..PERPETUAL as i64).contains(&days).then_some(days as u16)
}

pub fn date_of_day(day: u16) -> NaiveDate {
    day_epoch() + Duration::days(day as i64)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LicenseError {
    #[error("{0}")]
    Code(#[from] CodeError),
    #[error("This activation key was made for a newer version of CODE X MART. Update CODE X MART, then enter the key again.")]
    UnsupportedVersion,
    #[error("This activation key is not for CODE X MART.")]
    WrongProduct,
    #[error(
        "This activation key was issued for a different computer. Send this computer's request code to the software owner to get a key for it."
    )]
    WrongDevice,
    #[error(
        "This activation key was not signed by the CODE X MART license key, so it is not genuine (or it needs a newer version of CODE X MART)."
    )]
    UnknownSigningKey,
    #[error("This activation key is not genuine, or a character in it was changed. Check every group against the key you received.")]
    BadSignature,
    #[error("This activation key is not valid.")]
    Malformed,
    #[error("This activation key expired on {}.", .0.format("%d %b %Y"))]
    Expired(NaiveDate),
    #[error(
        "This computer's date is earlier than the key's issue date ({}). Correct the date and time in Windows, then try again.",
        .0.format("%d %b %Y")
    )]
    ClockBehind(NaiveDate),
    #[error("{0}")]
    Invalid(String),
}

/// The fields of a license, as signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct License {
    pub version: u8,
    pub product: u8,
    pub key_id: u8,
    pub edition: Edition,
    pub flags: u8,
    pub license_id: [u8; LICENSE_ID_LEN],
    pub device_hint: [u8; 2],
    pub issued_day: u16,
    pub expiry_day: u16,
    pub customer_no: u16,
}

impl License {
    pub fn payload_bytes(&self) -> [u8; PAYLOAD_LEN] {
        let mut b = [0u8; PAYLOAD_LEN];
        b[0] = self.version;
        b[1] = self.product;
        b[2] = self.key_id;
        b[3] = self.edition.to_u8() | (self.flags << 4);
        b[4..13].copy_from_slice(&self.license_id);
        b[13..15].copy_from_slice(&self.device_hint);
        b[15..17].copy_from_slice(&self.issued_day.to_be_bytes());
        b[17..19].copy_from_slice(&self.expiry_day.to_be_bytes());
        b[19..21].copy_from_slice(&self.customer_no.to_be_bytes());
        b
    }

    pub fn parse_payload(b: &[u8]) -> Result<Self, LicenseError> {
        if b.len() != PAYLOAD_LEN {
            return Err(LicenseError::Malformed);
        }
        match b[0] {
            0 => return Err(LicenseError::Malformed),
            v if v > FORMAT_VERSION => return Err(LicenseError::UnsupportedVersion),
            _ => {}
        }
        if b[1] != PRODUCT_CODE {
            return Err(LicenseError::WrongProduct);
        }
        let edition = Edition::from_u8(b[3] & 0x0F).ok_or(LicenseError::UnsupportedVersion)?;
        let license = License {
            version: b[0],
            product: b[1],
            key_id: b[2],
            edition,
            flags: b[3] >> 4,
            license_id: b[4..13].try_into().unwrap(),
            device_hint: b[13..15].try_into().unwrap(),
            issued_day: u16::from_be_bytes([b[15], b[16]]),
            expiry_day: u16::from_be_bytes([b[17], b[18]]),
            customer_no: u16::from_be_bytes([b[19], b[20]]),
        };
        // Lifetime licenses have no expiry date; every other edition has one,
        // and it cannot be before the issue date.
        let perpetual = license.expiry_day == PERPETUAL;
        if perpetual != edition.is_perpetual() || (!perpetual && license.expiry_day < license.issued_day) {
            return Err(LicenseError::Malformed);
        }
        Ok(license)
    }

    /// The exact bytes that are signed: product domain, full device id and
    /// every payload field.
    pub fn signed_message(&self, device_id: &DeviceId) -> Vec<u8> {
        let mut m = Vec::with_capacity(64);
        m.extend_from_slice(PRODUCT_ID.as_bytes());
        m.extend_from_slice(b"\0LICENSE\0");
        m.push(self.version);
        m.extend_from_slice(device_id);
        m.extend_from_slice(&self.payload_bytes());
        m
    }

    pub fn issued_on(&self) -> NaiveDate {
        date_of_day(self.issued_day)
    }

    pub fn expires_on(&self) -> Option<NaiveDate> {
        (self.expiry_day != PERPETUAL).then(|| date_of_day(self.expiry_day))
    }

    /// Valid through the end of its expiry day (shop's local date).
    pub fn is_active_on(&self, today: NaiveDate) -> bool {
        self.expires_on().is_none_or(|end| today <= end)
    }

    pub fn serial(&self) -> String {
        codec::format_serial(&self.license_id)
    }
}

/// Parses an activation key as typed. Checks the format, check symbols and
/// payload fields; does not check the signature (see [`verify_token`]).
pub fn decode_activation_key(input: &str) -> Result<(License, Token), LicenseError> {
    let symbols = codec::parse_code(&codec::ACTIVATION, input)?;
    let bytes = codec::decode_symbols(&symbols, TOKEN_LEN).ok_or(LicenseError::Malformed)?;
    let token: Token = bytes.try_into().map_err(|_| LicenseError::Malformed)?;
    let license = License::parse_payload(&token[..PAYLOAD_LEN])?;
    Ok((license, token))
}

pub fn encode_activation_key(token: &Token) -> String {
    codec::format_code(&codec::ACTIVATION, &codec::encode_symbols(token, TOKEN_SYMBOLS))
}

/// Verifies a license token for this computer against the trusted public
/// keys. Expiry is checked by the caller, which knows the trusted date.
pub fn verify_token(token: &Token, device_id: &DeviceId, trusted: &[TrustedKey]) -> Result<License, LicenseError> {
    let license = License::parse_payload(&token[..PAYLOAD_LEN])?;
    if license.device_hint != device_id[..2] {
        return Err(LicenseError::WrongDevice);
    }
    let signature = Signature::from_bytes(token[PAYLOAD_LEN..].try_into().unwrap());
    let message = license.signed_message(device_id);
    let mut candidates = trusted.iter().filter(|k| k.key_id == license.key_id).peekable();
    if candidates.peek().is_none() {
        return Err(LicenseError::UnknownSigningKey);
    }
    for key in candidates {
        if let Ok(vk) = VerifyingKey::from_bytes(&key.public_key) {
            if vk.verify_strict(&message, &signature).is_ok() {
                return Ok(license);
            }
        }
    }
    Err(LicenseError::BadSignature)
}

#[cfg(feature = "issuer")]
pub use issue::*;

#[cfg(feature = "issuer")]
mod issue {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::keys::key_id_for;

    /// What the owner decided to issue.
    #[derive(Debug, Clone)]
    pub struct IssueInput {
        pub device_id: DeviceId,
        pub edition: Edition,
        pub issued: NaiveDate,
        /// Required for [`Edition::Custom`]; ignored for Lifetime; defaults to
        /// [`Edition::default_expiry`] for the others.
        pub expires: Option<NaiveDate>,
        pub customer_no: u16,
        pub flags: u8,
    }

    pub fn new_license_id() -> [u8; LICENSE_ID_LEN] {
        let mut id = [0u8; LICENSE_ID_LEN];
        getrandom::getrandom(&mut id).expect("the operating system random number generator failed");
        id
    }

    /// Signs a license. Returns the license and its activation key text.
    pub fn issue_license(
        signing_key: &SigningKey,
        input: &IssueInput,
        license_id: [u8; LICENSE_ID_LEN],
    ) -> Result<(License, String), LicenseError> {
        let expires = match input.edition {
            Edition::Lifetime => None,
            Edition::Custom => Some(
                input
                    .expires
                    .ok_or_else(|| LicenseError::Invalid("Choose the expiry date for a custom license.".into()))?,
            ),
            e => Some(input.expires.or_else(|| e.default_expiry(input.issued)).ok_or(LicenseError::Malformed)?),
        };
        if let Some(end) = expires {
            if end < input.issued {
                return Err(LicenseError::Invalid("The expiry date cannot be before the issue date.".into()));
            }
        }
        let issued_day = day_number(input.issued)
            .ok_or_else(|| LicenseError::Invalid("The issue date is outside the supported range.".into()))?;
        let expiry_day = match expires {
            None => PERPETUAL,
            Some(d) => day_number(d)
                .ok_or_else(|| LicenseError::Invalid("The expiry date is outside the supported range.".into()))?,
        };
        let license = License {
            version: FORMAT_VERSION,
            product: PRODUCT_CODE,
            key_id: key_id_for(&signing_key.verifying_key().to_bytes()),
            edition: input.edition,
            flags: input.flags & 0x0F,
            license_id,
            device_hint: input.device_id[..2].try_into().unwrap(),
            issued_day,
            expiry_day,
            customer_no: input.customer_no,
        };
        let signature = signing_key.sign(&license.signed_message(&input.device_id));
        let mut token = [0u8; TOKEN_LEN];
        token[..PAYLOAD_LEN].copy_from_slice(&license.payload_bytes());
        token[PAYLOAD_LEN..].copy_from_slice(&signature.to_bytes());
        Ok((license, encode_activation_key(&token)))
    }
}
