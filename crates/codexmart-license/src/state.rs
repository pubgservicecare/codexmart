//! Trial and license state records, and the access decision.
//!
//! The app keeps the same small record in several places (Windows
//! registry, AppData, LocalAppData, ProgramData, other local disks and the
//! shop database). Each copy carries an HMAC tag, so edited or truncated
//! copies are ignored, and the copies are merged conservatively:
//!
//! * trial start: the **earliest** start found anywhere;
//! * last seen time and clock warnings: the **latest** / highest found;
//! * licenses: every signed license found (each re-verified on load).
//!
//! Deleting or restoring some copies therefore never restarts the trial;
//! only removing every copy (and the shop data that shows when setup
//! happened) would. The HMAC key is built into the program, so the tags
//! stop casual editing, not someone who studies the program. See
//! docs/LICENSE_SECURITY.md for what an offline system can and cannot do.

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

use crate::device::{DeviceId, Fingerprint, DEVICE_ID_LEN};
use crate::license::{License, Token, LICENSE_ID_LEN, TOKEN_LEN};

pub const TRIAL_DAYS: i64 = 30;
/// Reminders start when this many days (or fewer) remain: day 25 of 30.
pub const REMINDER_DAYS: i64 = 5;
/// A clock more than this far behind the last trusted time counts as set back.
pub const ROLLBACK_TOLERANCE_SECS: i64 = 3600;
/// After this many clock set-backs the free trial ends early.
pub const ROLLBACK_LIMIT: u16 = 3;
const MAX_TOKENS: usize = 16;

const MAGIC: &[u8; 4] = b"CXMS";
const RECORD_VERSION: u8 = 1;
const TAG_LEN: usize = 16;
const STATE_KEY: &[u8] = b"CODE X MART trial and license state v1";

/// Random id for a new installation record.
pub fn new_install_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    getrandom::getrandom(&mut id).expect("the operating system random number generator failed");
    id
}

/// No legitimate record or clock reading predates this release
/// (2026-11-01 00:00 UTC). Earlier times mean the computer's clock is wrong.
pub fn release_floor() -> i64 {
    Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap().timestamp()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRecord {
    pub install_id: [u8; 16],
    pub device_id: DeviceId,
    pub fingerprint: Fingerprint,
    /// Unix seconds; 0 = trial not started.
    pub trial_start: i64,
    /// Latest trusted time seen (unix seconds).
    pub last_seen: i64,
    pub rollback_count: u16,
    pub write_seq: u64,
    pub tokens: Vec<Token>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("not a state record")]
    Format,
    #[error("state record version {0} is not supported")]
    Version(u8),
    #[error("state record failed its integrity check")]
    Tag,
}

fn mac(data: &[u8]) -> [u8; TAG_LEN] {
    let mut m = Hmac::<Sha256>::new_from_slice(STATE_KEY).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes()[..TAG_LEN].try_into().unwrap()
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], StateError> {
        let s = self.b.get(self.pos..self.pos + n).ok_or(StateError::Format)?;
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, StateError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, StateError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, StateError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, StateError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
}

impl StateRecord {
    pub fn new(install_id: [u8; 16], device_id: DeviceId, fingerprint: Fingerprint, now: i64) -> Self {
        StateRecord {
            install_id,
            device_id,
            fingerprint,
            trial_start: 0,
            last_seen: now,
            rollback_count: 0,
            write_seq: 0,
            tokens: Vec::new(),
        }
    }

    pub fn encode(&self) -> String {
        let mut b = Vec::with_capacity(128 + self.tokens.len() * TOKEN_LEN);
        b.extend_from_slice(MAGIC);
        b.push(RECORD_VERSION);
        b.extend_from_slice(&self.install_id);
        b.extend_from_slice(&self.device_id);
        for v in self.fingerprint.to_array() {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&self.trial_start.to_be_bytes());
        b.extend_from_slice(&self.last_seen.to_be_bytes());
        b.extend_from_slice(&self.rollback_count.to_be_bytes());
        b.extend_from_slice(&self.write_seq.to_be_bytes());
        let tokens = &self.tokens[..self.tokens.len().min(MAX_TOKENS)];
        b.push(tokens.len() as u8);
        for t in tokens {
            b.extend_from_slice(t);
        }
        let tag = mac(&b);
        b.extend_from_slice(&tag);
        hex::encode(b)
    }

    pub fn decode(text: &str) -> Result<Self, StateError> {
        let b = hex::decode(text.trim()).map_err(|_| StateError::Format)?;
        if b.len() < MAGIC.len() + 1 + TAG_LEN || &b[..4] != MAGIC {
            return Err(StateError::Format);
        }
        if b[4] != RECORD_VERSION {
            return Err(StateError::Version(b[4]));
        }
        let (body, tag) = b.split_at(b.len() - TAG_LEN);
        if mac(body) != tag {
            return Err(StateError::Tag);
        }
        let mut r = Reader { b: body, pos: 5 };
        let install_id = r.take(16)?.try_into().unwrap();
        let device_id = r.take(DEVICE_ID_LEN)?.try_into().unwrap();
        let mut fp = [0u64; 6];
        for v in fp.iter_mut() {
            *v = r.u64()?;
        }
        let trial_start = r.i64()?;
        let last_seen = r.i64()?;
        let rollback_count = r.u16()?;
        let write_seq = r.u64()?;
        let n = r.u8()? as usize;
        let mut tokens = Vec::with_capacity(n);
        for _ in 0..n {
            tokens.push(r.take(TOKEN_LEN)?.try_into().unwrap());
        }
        if r.pos != body.len() {
            return Err(StateError::Format);
        }
        Ok(StateRecord {
            install_id,
            device_id,
            fingerprint: Fingerprint::from_array(fp),
            trial_start,
            last_seen,
            rollback_count,
            write_seq,
            tokens,
        })
    }

    /// Folds another copy into this one (identity fields are kept).
    pub fn merge_from(&mut self, other: &StateRecord) {
        if other.trial_start > 0 && (self.trial_start == 0 || other.trial_start < self.trial_start) {
            self.trial_start = other.trial_start;
        }
        self.last_seen = self.last_seen.max(other.last_seen);
        self.rollback_count = self.rollback_count.max(other.rollback_count);
        self.write_seq = self.write_seq.max(other.write_seq);
        for t in &other.tokens {
            self.add_token(*t);
        }
    }

    /// Adds a license token, replacing an older copy of the same license.
    /// Keeps the newest licenses if there are too many.
    pub fn add_token(&mut self, token: Token) {
        let id = license_id(&token);
        if self.tokens.iter().any(|t| license_id(t) == id) {
            return;
        }
        self.tokens.push(token);
        if self.tokens.len() > MAX_TOKENS {
            self.tokens.sort_by_key(|t| std::cmp::Reverse(issued_day(t)));
            self.tokens.truncate(MAX_TOKENS);
        }
    }
}

fn license_id(t: &Token) -> [u8; LICENSE_ID_LEN] {
    t[4..4 + LICENSE_ID_LEN].try_into().unwrap()
}

fn issued_day(t: &Token) -> u16 {
    u16::from_be_bytes([t[15], t[16]])
}

/// What the customer may do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessState {
    /// Before the shop has been set up.
    NotStarted,
    Trial,
    TrialExpired,
    Licensed,
    LicenseExpired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub state: AccessState,
    pub allowed: bool,
    pub today: NaiveDate,
    /// 1-based day of the trial (1..=30 while it runs).
    pub trial_day: Option<i64>,
    /// Days after today that the trial still runs (0 on the last day).
    pub trial_days_left: Option<i64>,
    /// Last day of the trial.
    pub trial_last_day: Option<NaiveDate>,
    /// The trial ended early because the clock was set back repeatedly.
    pub trial_ended_by_clock: bool,
    /// The license in force (or the most recent expired one).
    pub license: Option<License>,
    /// Days after today that a time-limited license still runs.
    pub license_days_left: Option<i64>,
    /// Show the "expires soon" reminder.
    pub reminder: bool,
}

pub fn local_date(unix: i64, tz_offset_secs: i32) -> NaiveDate {
    let tz = FixedOffset::east_opt(tz_offset_secs).unwrap_or(FixedOffset::east_opt(0).unwrap());
    DateTime::from_timestamp(unix, 0).unwrap_or_default().with_timezone(&tz).date_naive()
}

/// Decides access from the merged state. `now` must already be the trusted
/// time (never earlier than the last time seen); `licenses` must contain
/// only licenses whose signatures were verified for this computer.
pub fn evaluate(
    trial_start: i64,
    now: i64,
    tz_offset_secs: i32,
    licenses: &[License],
    rollback_count: u16,
) -> Evaluation {
    let today = local_date(now, tz_offset_secs);
    let mut e = Evaluation {
        state: AccessState::NotStarted,
        allowed: true,
        today,
        trial_day: None,
        trial_days_left: None,
        trial_last_day: None,
        trial_ended_by_clock: false,
        license: None,
        license_days_left: None,
        reminder: false,
    };

    let mut trial_active = trial_start == 0;
    if trial_start > 0 {
        let start = local_date(trial_start, tz_offset_secs);
        let last = start + chrono::Duration::days(TRIAL_DAYS - 1);
        e.trial_day = Some((today - start).num_days() + 1);
        e.trial_days_left = Some((last - today).num_days());
        e.trial_last_day = Some(last);
        e.trial_ended_by_clock = rollback_count >= ROLLBACK_LIMIT;
        trial_active = today <= last && !e.trial_ended_by_clock;
    }

    // Prefer a lifetime license, then the one that runs longest.
    let best_active = licenses
        .iter()
        .filter(|l| l.is_active_on(today))
        .max_by_key(|l| (l.expires_on().is_none(), l.expires_on(), l.issued_day));
    if let Some(l) = best_active {
        e.state = AccessState::Licensed;
        e.license_days_left = l.expires_on().map(|end| (end - today).num_days());
        e.reminder = e.license_days_left.is_some_and(|d| d <= REMINDER_DAYS);
        e.license = Some(l.clone());
        return e;
    }
    let latest_expired = licenses.iter().max_by_key(|l| l.expires_on());

    if trial_start == 0 {
        e.state = AccessState::NotStarted;
    } else if trial_active {
        e.state = AccessState::Trial;
        e.reminder = e.trial_days_left.is_some_and(|d| d <= REMINDER_DAYS);
    } else if let Some(l) = latest_expired {
        e.state = AccessState::LicenseExpired;
        e.allowed = false;
        e.license = Some(l.clone());
    } else {
        e.state = AccessState::TrialExpired;
        e.allowed = false;
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::RawIdentifiers;

    fn record() -> StateRecord {
        let fp =
            Fingerprint::from_raw(&RawIdentifiers { machine_guid: Some("abcdef12-3456".into()), ..Default::default() });
        let mut r = StateRecord::new([5; 16], fp.derive_device_id(), fp, release_floor() + 100);
        r.trial_start = release_floor() + 50;
        r.tokens.push([7; TOKEN_LEN]);
        r
    }

    #[test]
    fn record_round_trip() {
        let r = record();
        assert_eq!(StateRecord::decode(&r.encode()).unwrap(), r);
    }

    #[test]
    fn edited_records_are_rejected() {
        let text = record().encode();
        let mut bytes = hex::decode(&text).unwrap();
        // Move the trial start one day later.
        let pos = 4 + 1 + 16 + 10 + 48 + 6;
        bytes[pos] = bytes[pos].wrapping_add(1);
        assert_eq!(StateRecord::decode(&hex::encode(&bytes)), Err(StateError::Tag));
        assert_eq!(StateRecord::decode(&text[..text.len() - 2]), Err(StateError::Tag));
        assert_eq!(StateRecord::decode("not hex"), Err(StateError::Format));
    }

    #[test]
    fn merge_keeps_the_earliest_trial_start_and_latest_time() {
        let mut a = record();
        let mut b = record();
        b.trial_start = a.trial_start - 86_400 * 3;
        b.last_seen = a.last_seen + 500;
        b.rollback_count = 2;
        b.tokens = vec![[9; TOKEN_LEN]];
        a.merge_from(&b);
        assert_eq!(a.trial_start, b.trial_start);
        assert_eq!(a.last_seen, b.last_seen);
        assert_eq!(a.rollback_count, 2);
        assert_eq!(a.tokens.len(), 2);
        // A record without a trial start never clears one.
        let mut empty = record();
        empty.trial_start = 0;
        let before = a.trial_start;
        a.merge_from(&empty);
        assert_eq!(a.trial_start, before);
    }

    const DAY: i64 = 86_400;
    // Noon Pakistan time on the first day.
    fn start() -> i64 {
        Utc.with_ymd_and_hms(2026, 11, 2, 7, 0, 0).unwrap().timestamp()
    }
    const PKT: i32 = 5 * 3600;

    #[test]
    fn trial_days_and_reminders() {
        for day in 1..=24 {
            let e = evaluate(start(), start() + (day - 1) * DAY, PKT, &[], 0);
            assert_eq!(e.state, AccessState::Trial);
            assert_eq!(e.trial_day, Some(day));
            assert!(!e.reminder, "no reminder on day {day}");
            assert!(e.allowed);
        }
        for day in 25..=30 {
            let e = evaluate(start(), start() + (day - 1) * DAY, PKT, &[], 0);
            assert_eq!(e.state, AccessState::Trial);
            assert_eq!(e.trial_days_left, Some(30 - day));
            assert!(e.reminder, "reminder on day {day}");
            assert!(e.allowed);
        }
        let e = evaluate(start(), start() + 30 * DAY, PKT, &[], 0);
        assert_eq!(e.state, AccessState::TrialExpired);
        assert!(!e.allowed);
        assert_eq!(e.trial_day, Some(31));
    }

    #[test]
    fn trial_follows_calendar_days_in_local_time() {
        // Started 23:30 local on 2 Nov: day 2 begins at local midnight.
        let late = Utc.with_ymd_and_hms(2026, 11, 2, 18, 30, 0).unwrap().timestamp();
        let after_midnight = late + 3600;
        assert_eq!(evaluate(late, after_midnight, PKT, &[], 0).trial_day, Some(2));
        let e = evaluate(late, late, PKT, &[], 0);
        assert_eq!(e.trial_last_day, Some(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()));
    }

    #[test]
    fn repeated_clock_setbacks_end_the_trial() {
        let e = evaluate(start(), start() + DAY, PKT, &[], ROLLBACK_LIMIT);
        assert_eq!(e.state, AccessState::TrialExpired);
        assert!(e.trial_ended_by_clock);
        assert!(evaluate(start(), start() + DAY, PKT, &[], ROLLBACK_LIMIT - 1).allowed);
    }

    #[test]
    fn not_started_is_allowed() {
        let e = evaluate(0, start(), PKT, &[], 0);
        assert_eq!(e.state, AccessState::NotStarted);
        assert!(e.allowed);
    }
}
