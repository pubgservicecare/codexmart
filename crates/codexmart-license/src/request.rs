//! License request codes: what the customer sends to the owner.
//!
//! A request code carries the computer's device id plus a little context
//! for the owner (app version, trial start, current status) in 20 bytes,
//! written as `CXMR-` and eight groups of five symbols. A 32-bit tag
//! (HMAC-SHA256 with a key built into CODE X MART) rejects codes that were
//! mistyped or casually edited.
//!
//! The tag key ships inside the app, so the tag is not a security boundary.
//! It does not need to be: a license is signed for the device id in the
//! request, and only a computer with that device id can use it. Changing a
//! request code can only produce a license for some other computer.

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

use crate::codec;
use crate::device::{DeviceId, DEVICE_ID_LEN};
use crate::license::{LicenseError, PRODUCT_CODE};

const REQUEST_VERSION: u8 = 1;
const REQUEST_LEN: usize = 20;
const TAG_LEN: usize = 4;
const TAG_KEY: &[u8] = b"CODE X MART license request code v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestStatus {
    NotStarted,
    Trial,
    TrialExpired,
    Licensed,
    LicenseExpired,
}

impl RequestStatus {
    fn to_u8(self) -> u8 {
        match self {
            RequestStatus::NotStarted => 0,
            RequestStatus::Trial => 1,
            RequestStatus::TrialExpired => 2,
            RequestStatus::Licensed => 3,
            RequestStatus::LicenseExpired => 4,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        [Self::NotStarted, Self::Trial, Self::TrialExpired, Self::Licensed, Self::LicenseExpired]
            .into_iter()
            .find(|s| s.to_u8() == v)
    }

    pub fn label(self) -> &'static str {
        match self {
            RequestStatus::NotStarted => "Trial not started",
            RequestStatus::Trial => "In free trial",
            RequestStatus::TrialExpired => "Free trial ended",
            RequestStatus::Licensed => "Licensed",
            RequestStatus::LicenseExpired => "License expired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInfo {
    pub device_id: DeviceId,
    pub app_major: u8,
    pub app_minor: u8,
    /// Day number (see [`crate::license::day_number`]); `None` before setup.
    pub trial_start_day: Option<u16>,
    pub status: RequestStatus,
    /// Set when the app noticed the clock being set back.
    pub clock_warning: bool,
}

fn tag(data: &[u8]) -> [u8; TAG_LEN] {
    let mut mac = Hmac::<Sha256>::new_from_slice(TAG_KEY).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes()[..TAG_LEN].try_into().unwrap()
}

pub fn encode_request(info: &RequestInfo) -> String {
    let mut b = [0u8; REQUEST_LEN];
    b[0] = REQUEST_VERSION;
    b[1] = PRODUCT_CODE;
    b[2..12].copy_from_slice(&info.device_id);
    b[12] = (info.app_major.min(15) << 4) | info.app_minor.min(15);
    b[13..15].copy_from_slice(&info.trial_start_day.unwrap_or(0).to_be_bytes());
    b[15] = info.status.to_u8() | if info.clock_warning { 0x80 } else { 0 };
    let t = tag(&b[..REQUEST_LEN - TAG_LEN]);
    b[REQUEST_LEN - TAG_LEN..].copy_from_slice(&t);
    codec::format_code(&codec::REQUEST, &codec::encode_symbols(&b, 32))
}

pub fn decode_request(input: &str) -> Result<RequestInfo, LicenseError> {
    let symbols = codec::parse_code(&codec::REQUEST, input)?;
    let b = codec::decode_symbols(&symbols, REQUEST_LEN).ok_or(LicenseError::Malformed)?;
    if tag(&b[..REQUEST_LEN - TAG_LEN]) != b[REQUEST_LEN - TAG_LEN..] {
        return Err(LicenseError::Invalid(
            "This request code was changed or mistyped. Ask the customer to send it again (a photo of the screen works well).".into(),
        ));
    }
    if b[0] != REQUEST_VERSION {
        return Err(LicenseError::Invalid("This request code comes from a newer version of CODE X MART.".into()));
    }
    if b[1] != PRODUCT_CODE {
        return Err(LicenseError::WrongProduct);
    }
    let start = u16::from_be_bytes([b[13], b[14]]);
    Ok(RequestInfo {
        device_id: b[2..2 + DEVICE_ID_LEN].try_into().unwrap(),
        app_major: b[12] >> 4,
        app_minor: b[12] & 0x0F,
        trial_start_day: (start != 0).then_some(start),
        status: RequestStatus::from_u8(b[15] & 0x7F).ok_or(LicenseError::Malformed)?,
        clock_warning: b[15] & 0x80 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> RequestInfo {
        RequestInfo {
            device_id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            app_major: 1,
            app_minor: 1,
            trial_start_day: Some(2496),
            status: RequestStatus::TrialExpired,
            clock_warning: true,
        }
    }

    #[test]
    fn request_round_trip() {
        let code = encode_request(&info());
        assert!(code.starts_with("CXMR-"));
        assert_eq!(code.len(), 4 + 8 * 6);
        assert_eq!(decode_request(&code).unwrap(), info());
        assert_eq!(decode_request(&code.to_lowercase()).unwrap(), info());
    }

    #[test]
    fn edited_request_is_rejected() {
        let mut other = info();
        other.device_id[0] ^= 1;
        let original = encode_request(&info());
        let edited = encode_request(&other);
        // Splice the first groups of the edited code with the original tag.
        let spliced = format!("{}{}", &edited[..20], &original[20..]);
        assert!(decode_request(&spliced).is_err());
    }
}
