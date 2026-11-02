//! Issue-and-verify tests for activation keys, using throwaway keys.

use chrono::NaiveDate;
use codexmart_license::codec::{self, CodeError};
use codexmart_license::device::DeviceId;
use codexmart_license::keys::{key_id_for, TrustedKey};
use codexmart_license::license::*;
use codexmart_license::request::{decode_request, encode_request, RequestInfo, RequestStatus};
use codexmart_license::state::{evaluate, AccessState};
use ed25519_dalek::SigningKey;

const DEVICE: DeviceId = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA];

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn trusted(sk: &SigningKey) -> Vec<TrustedKey> {
    let public_key = sk.verifying_key().to_bytes();
    vec![TrustedKey { key_id: key_id_for(&public_key), public_key, label: "test" }]
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn issue(sk: &SigningKey, edition: Edition, device: DeviceId) -> String {
    let input =
        IssueInput { device_id: device, edition, issued: date(2026, 12, 1), expires: None, customer_no: 42, flags: 0 };
    let expires = if edition == Edition::Custom { Some(date(2027, 2, 15)) } else { None };
    issue_license(sk, &IssueInput { expires, ..input }, new_license_id()).unwrap().1
}

fn verify(key: &str, device: &DeviceId, keys: &[TrustedKey]) -> Result<License, LicenseError> {
    let (_, token) = decode_activation_key(key)?;
    verify_token(&token, device, keys)
}

/// Re-encodes a token after `edit`, so every check symbol is valid again.
fn tamper(key: &str, edit: impl FnOnce(&mut Token)) -> String {
    let (_, mut token) = decode_activation_key(key).unwrap();
    edit(&mut token);
    encode_activation_key(&token)
}

#[test]
fn every_edition_issues_and_verifies_offline() {
    let sk = signing_key(1);
    for edition in Edition::ALL {
        let key = issue(&sk, edition, DEVICE);
        assert!(key.starts_with("CXMA-"));
        assert_eq!(key.len(), 4 + 34 * 6, "fixed length");
        let lic = verify(&key, &DEVICE, &trusted(&sk)).unwrap();
        assert_eq!(lic.edition, edition);
        assert_eq!(lic.customer_no, 42);
        assert_eq!(lic.issued_on(), date(2026, 12, 1));
        assert_eq!(lic.expires_on().is_none(), edition == Edition::Lifetime);
        // Case and separators do not matter.
        let messy = key.to_lowercase().replace('-', " ");
        assert_eq!(verify(&messy, &DEVICE, &trusted(&sk)).unwrap(), lic);
    }
}

#[test]
fn durations_are_calendar_based() {
    let issued = date(2026, 11, 1);
    assert_eq!(Edition::Trial30.default_expiry(issued), Some(date(2026, 11, 30)));
    assert_eq!(Edition::Month1.default_expiry(issued), Some(date(2026, 11, 30)));
    assert_eq!(Edition::Month3.default_expiry(issued), Some(date(2027, 1, 31)));
    assert_eq!(Edition::Month6.default_expiry(issued), Some(date(2027, 4, 30)));
    assert_eq!(Edition::Year1.default_expiry(issued), Some(date(2027, 10, 31)));
    assert_eq!(Edition::Lifetime.default_expiry(issued), None);
}

#[test]
fn expiry_and_lifetime() {
    let sk = signing_key(2);
    let month = verify(&issue(&sk, Edition::Month1, DEVICE), &DEVICE, &trusted(&sk)).unwrap();
    assert_eq!(month.expires_on(), Some(date(2026, 12, 31)));
    assert!(month.is_active_on(date(2026, 12, 31)));
    assert!(!month.is_active_on(date(2027, 1, 1)));
    let lifetime = verify(&issue(&sk, Edition::Lifetime, DEVICE), &DEVICE, &trusted(&sk)).unwrap();
    assert!(lifetime.is_active_on(date(2099, 12, 31)));
}

#[test]
fn a_valid_checksum_with_a_forged_signature_is_rejected() {
    let sk = signing_key(3);
    let key = issue(&sk, Edition::Year1, DEVICE);
    let forged = tamper(&key, |t| t[PAYLOAD_LEN + 10] ^= 0x01);
    // Every group's check symbol is correct...
    assert!(codec::parse_code(&codec::ACTIVATION, &forged).is_ok());
    // ...but the signature is not.
    assert_eq!(verify(&forged, &DEVICE, &trusted(&sk)), Err(LicenseError::BadSignature));
}

#[test]
fn changed_license_fields_are_rejected() {
    let sk = signing_key(4);
    let key = issue(&sk, Edition::Month1, DEVICE);
    // Turn a 1-month license into one that runs until 2030.
    let longer = tamper(&key, |t| t[17..19].copy_from_slice(&day_number(date(2030, 1, 1)).unwrap().to_be_bytes()));
    assert_eq!(verify(&longer, &DEVICE, &trusted(&sk)), Err(LicenseError::BadSignature));
    // Change the customer number.
    let other_customer = tamper(&key, |t| t[20] ^= 0x05);
    assert_eq!(verify(&other_customer, &DEVICE, &trusted(&sk)), Err(LicenseError::BadSignature));
    // Claim lifetime without the perpetual expiry: structurally invalid.
    let lifetime_flag = tamper(&key, |t| t[3] = (t[3] & 0xF0) | 6);
    assert_eq!(verify(&lifetime_flag, &DEVICE, &trusted(&sk)), Err(LicenseError::Malformed));
}

#[test]
fn wrong_device_is_rejected() {
    let sk = signing_key(5);
    let key = issue(&sk, Edition::Lifetime, DEVICE);
    let other: DeviceId = [9; 10];
    assert_eq!(verify(&key, &other, &trusted(&sk)), Err(LicenseError::WrongDevice));
    // Same first two bytes (passes the hint), different computer.
    let mut lookalike = DEVICE;
    lookalike[9] ^= 0xFF;
    assert_eq!(verify(&key, &lookalike, &trusted(&sk)), Err(LicenseError::BadSignature));
}

#[test]
fn wrong_product_and_unsupported_versions_are_rejected() {
    let sk = signing_key(6);
    let key = issue(&sk, Edition::Lifetime, DEVICE);
    let other_product = tamper(&key, |t| t[1] = 2);
    assert_eq!(verify(&other_product, &DEVICE, &trusted(&sk)), Err(LicenseError::WrongProduct));
    let newer = tamper(&key, |t| t[0] = FORMAT_VERSION + 1);
    assert_eq!(verify(&newer, &DEVICE, &trusted(&sk)), Err(LicenseError::UnsupportedVersion));
}

#[test]
fn keys_signed_by_an_untrusted_key_are_rejected() {
    let owner = signing_key(7);
    let attacker = signing_key(8);
    let key = issue(&attacker, Edition::Lifetime, DEVICE);
    let result = verify(&key, &DEVICE, &trusted(&owner));
    assert!(matches!(result, Err(LicenseError::UnknownSigningKey | LicenseError::BadSignature)));
    // Even claiming the owner's key id does not help.
    let relabelled = tamper(&key, |t| t[2] = key_id_for(&owner.verifying_key().to_bytes()));
    assert_eq!(verify(&relabelled, &DEVICE, &trusted(&owner)), Err(LicenseError::BadSignature));
}

#[test]
fn typing_mistakes_are_caught_before_the_signature_check() {
    let sk = signing_key(9);
    let key = issue(&sk, Edition::Month3, DEVICE);
    let mut chars: Vec<char> = key.chars().collect();
    // A character inside group 12.
    let pos = 4 + 1 + 11 * 6 + 2;
    chars[pos] = if chars[pos] == 'K' { 'L' } else { 'K' };
    let typo: String = chars.into_iter().collect();
    match verify(&typo, &DEVICE, &trusted(&sk)) {
        Err(LicenseError::Code(CodeError::Checksum { groups })) => assert_eq!(groups[0].0, 12),
        other => panic!("expected a checksum error, got {other:?}"),
    }
    let short = &key[..key.len() - 12];
    assert!(matches!(verify(short, &DEVICE, &trusted(&sk)), Err(LicenseError::Code(CodeError::Incomplete { .. }))));
}

#[test]
fn custom_licenses_need_a_valid_end_date() {
    let sk = signing_key(10);
    let base = IssueInput {
        device_id: DEVICE,
        edition: Edition::Custom,
        issued: date(2026, 12, 1),
        expires: None,
        customer_no: 1,
        flags: 0,
    };
    assert!(issue_license(&sk, &base, new_license_id()).is_err());
    let backwards = IssueInput { expires: Some(date(2026, 11, 1)), ..base.clone() };
    assert!(issue_license(&sk, &backwards, new_license_id()).is_err());
    let ok = IssueInput { expires: Some(date(2027, 3, 1)), ..base };
    let (lic, _) = issue_license(&sk, &ok, new_license_id()).unwrap();
    assert_eq!(lic.expires_on(), Some(date(2027, 3, 1)));
}

#[test]
fn licenses_drive_the_access_decision() {
    let sk = signing_key(11);
    let keys = trusted(&sk);
    let month = verify(&issue(&sk, Edition::Month1, DEVICE), &DEVICE, &keys).unwrap();
    let lifetime = verify(&issue(&sk, Edition::Lifetime, DEVICE), &DEVICE, &keys).unwrap();
    let trial_start = chrono::Utc.with_ymd_and_hms(2026, 11, 1, 7, 0, 0).unwrap().timestamp();
    let at = |d: NaiveDate| d.and_hms_opt(7, 0, 0).unwrap().and_utc().timestamp();

    // Trial over, month license active, reminder in its last 5 days.
    let e = evaluate(trial_start, at(date(2026, 12, 20)), 18_000, std::slice::from_ref(&month), 0);
    assert_eq!(e.state, AccessState::Licensed);
    assert!(e.allowed && !e.reminder);
    let e = evaluate(trial_start, at(date(2026, 12, 27)), 18_000, std::slice::from_ref(&month), 0);
    assert!(e.reminder);
    assert_eq!(e.license_days_left, Some(4));
    // Month license over: blocked again.
    let e = evaluate(trial_start, at(date(2027, 1, 2)), 18_000, std::slice::from_ref(&month), 0);
    assert_eq!(e.state, AccessState::LicenseExpired);
    assert!(!e.allowed);
    // Lifetime wins and is never affected by the clock warnings.
    let e = evaluate(trial_start, at(date(2040, 1, 1)), 18_000, &[month, lifetime], 99);
    assert_eq!(e.state, AccessState::Licensed);
    assert_eq!(e.license.unwrap().edition, Edition::Lifetime);
}

#[test]
fn request_codes_carry_the_device_id() {
    let info = RequestInfo {
        device_id: DEVICE,
        app_major: 1,
        app_minor: 1,
        trial_start_day: day_number(date(2026, 11, 1)),
        status: RequestStatus::TrialExpired,
        clock_warning: false,
    };
    let code = encode_request(&info);
    let decoded = decode_request(&code).unwrap();
    assert_eq!(decoded.device_id, DEVICE);
    // The owner issues for the decoded device; the customer's app accepts it.
    let sk = signing_key(12);
    let input = IssueInput {
        device_id: decoded.device_id,
        edition: Edition::Lifetime,
        issued: date(2026, 12, 2),
        expires: None,
        customer_no: 7,
        flags: 0,
    };
    let (_, key) = issue_license(&sk, &input, new_license_id()).unwrap();
    assert!(verify(&key, &DEVICE, &trusted(&sk)).is_ok());
    // An activation key typed where a request code belongs is explained.
    assert!(decode_request(&key).is_err());
}

#[test]
fn license_serials_are_unique_and_checked() {
    let a = new_license_id();
    let b = new_license_id();
    assert_ne!(a, b);
    let serial = codec::format_serial(&a);
    assert_eq!(codec::parse_serial(&serial), Some(a));
}

use chrono::TimeZone;
