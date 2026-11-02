//! Free trial and offline license enforcement.
//!
//! [`Licensing`] decides whether business operations are allowed. It is
//! loaded at start-up from every copy of the state record (see
//! [`anchors`]) plus the shop database, refreshed every few minutes while
//! the app runs, and consulted by `AppState::run` before every business
//! command. Enforcement never touches shop data: signing in, settings,
//! backups, restore and activation always work, and nothing is deleted,
//! hidden or encrypted when the trial or a license ends.
//!
//! Time: the "trusted now" is the latest of the Windows clock, the last
//! time seen (stored in the records), the time elapsed since start-up and
//! this release's date. Setting the clock back therefore never gives back
//! trial days; it is counted, and after repeated set-backs the trial ends.

mod anchors;
mod fingerprint;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveDate, Utc};
use codexmart_core::db::{self, Connection};
use codexmart_core::{AppError, AppResult, APP_VERSION};
use codexmart_license::codec::{self, CodeError, GroupStatus};
use codexmart_license::device::{format_device_id, Fingerprint};
use codexmart_license::keys::{TrustedKey, PRODUCTION_KEYS};
use codexmart_license::license::{day_number, decode_activation_key, verify_token, License, LicenseError};
use codexmart_license::request::{encode_request, RequestInfo, RequestStatus};
use codexmart_license::state::{self as st, evaluate, release_floor, AccessState, Evaluation, StateRecord};
use codexmart_license::{OWNER_NAME, OWNER_PHONE, OWNER_WHATSAPP_URL};
use serde::Serialize;

pub use anchors::Locations;

/// Key of the state record in the shop database's `app_meta` table.
const DB_KEY: &str = "license_state";
/// How often the running app refreshes the state record.
pub const REFRESH_EVERY: Duration = Duration::from_secs(300);

struct Clock {
    offset: i64,
}

impl Clock {
    fn from_env() -> Self {
        // Automated tests move the clock to reach day 25, day 31, ...
        #[cfg(feature = "e2e")]
        if let Some(offset) = std::env::var("CODEXMART_TEST_CLOCK_OFFSET").ok().and_then(|v| v.trim().parse().ok()) {
            return Clock { offset };
        }
        Clock { offset: 0 }
    }

    fn wall(&self) -> i64 {
        Utc::now().timestamp() + self.offset
    }
}

fn tz_offset() -> i32 {
    Local::now().offset().local_minus_utc()
}

fn trusted_keys() -> Vec<TrustedKey> {
    #[allow(unused_mut)]
    let mut keys = PRODUCTION_KEYS.to_vec();
    #[cfg(feature = "e2e")]
    if let Some(public_key) = std::env::var("CODEXMART_TEST_TRUSTED_KEY")
        .ok()
        .and_then(|h| hex::decode(h.trim()).ok())
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    {
        keys.push(TrustedKey {
            key_id: codexmart_license::keys::key_id_for(&public_key),
            public_key,
            label: "end-to-end test key",
        });
    }
    keys
}

struct Inner {
    record: StateRecord,
    /// Licenses in `record` whose signatures were verified for this computer.
    licenses: Vec<License>,
    /// Wall clock, trusted time and monotonic instant at the session's start.
    base_wall: i64,
    base_trusted: i64,
    base_instant: Instant,
    rollback_noticed: bool,
    /// Trusted time the clock was found to be behind, for the warning.
    clock_warning: Option<i64>,
}

pub struct Licensing {
    inner: Mutex<Inner>,
    locations: Locations,
    trusted: Vec<TrustedKey>,
    clock: Clock,
}

fn read_db(c: &Connection) -> Option<StateRecord> {
    let text = db::meta_get(c, DB_KEY).ok().flatten()?;
    StateRecord::decode(&text).map_err(|e| log::warn!("Ignored license state in the database: {e}")).ok()
}

/// When the shop was set up (first user created), if it has been.
fn setup_time(c: &Connection) -> Option<i64> {
    let first: Option<String> = c.query_row("SELECT MIN(created_at) FROM users", [], |r| r.get(0)).ok().flatten();
    DateTime::parse_from_rfc3339(&first?).ok().map(|d| d.timestamp())
}

/// Chooses this computer's identity and merges every copy found.
fn build_record(records: &[StateRecord], fp: Fingerprint) -> StateRecord {
    let mine = records.iter().filter(|r| r.fingerprint.same_device(&fp)).max_by_key(|r| r.write_seq);
    let (install_id, device_id) = match mine {
        Some(r) => (r.install_id, r.device_id),
        None => (st::new_install_id(), fp.derive_device_id()),
    };
    let mut record = StateRecord::new(install_id, device_id, fp, 0);
    for r in records {
        record.merge_from(r);
    }
    record
}

/// Verifies stored licenses; drops those that can never be valid here.
fn verified(record: &mut StateRecord, trusted: &[TrustedKey]) -> Vec<License> {
    let device = record.device_id;
    let mut ok = Vec::new();
    record.tokens.retain(|t| match verify_token(t, &device, trusted) {
        Ok(l) => {
            ok.push(l);
            true
        }
        // Signed by a key a newer CODE X MART may trust: keep it.
        Err(LicenseError::UnknownSigningKey) => true,
        Err(e) => {
            log::warn!("A stored license is not valid on this computer: {e}");
            false
        }
    });
    ok
}

fn date_text(d: NaiveDate) -> String {
    d.format("%a, %d %b %Y").to_string()
}

fn in_days(n: i64) -> String {
    match n {
        0 => "today".into(),
        1 => "tomorrow".into(),
        n => format!("in {n} days"),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerContact {
    pub name: &'static str,
    pub phone: &'static str,
    pub whatsapp_url: &'static str,
}

const OWNER: OwnerContact = OwnerContact { name: OWNER_NAME, phone: OWNER_PHONE, whatsapp_url: OWNER_WHATSAPP_URL };

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub serial: String,
    pub edition: String,
    pub edition_label: &'static str,
    pub issued_on: String,
    pub expires_on: Option<String>,
    pub customer_no: u16,
}

impl From<&License> for LicenseInfo {
    fn from(l: &License) -> Self {
        LicenseInfo {
            serial: l.serial(),
            edition: l.edition.code().into(),
            edition_label: l.edition.label(),
            issued_on: l.issued_on().to_string(),
            expires_on: l.expires_on().map(|d| d.to_string()),
            customer_no: l.customer_no,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub state: AccessState,
    /// Business operations are allowed.
    pub allowed: bool,
    pub today: String,
    pub trial_days: i64,
    pub trial_started_on: Option<String>,
    pub trial_day: Option<i64>,
    pub trial_days_left: Option<i64>,
    pub trial_last_day: Option<String>,
    pub trial_ended_by_clock: bool,
    pub license: Option<LicenseInfo>,
    pub license_days_left: Option<i64>,
    /// Show the once-a-day "expires soon" reminder.
    pub reminder: bool,
    pub clock_warning: Option<String>,
    pub device_id: String,
    pub owner: OwnerContact,
    pub headline: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestCodeInfo {
    pub request_code: String,
    pub device_id: String,
    pub app_version: &'static str,
    pub product: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInspection {
    pub groups: Vec<GroupStatus>,
    pub expected_groups: usize,
    pub complete: bool,
    /// `empty`, `incomplete`, `typo`, `character`, `wrongKind`, `prefix`, `tooLong`.
    pub problem: Option<&'static str>,
    pub message: Option<String>,
}

pub fn inspect_key(input: &str) -> KeyInspection {
    let i = codec::inspect(&codec::ACTIVATION, input);
    let problem = i.result.as_ref().err().map(|e| match e {
        CodeError::Empty { .. } => "empty",
        CodeError::Incomplete { .. } => "incomplete",
        CodeError::Checksum { .. } => "typo",
        CodeError::InvalidCharacter { .. } => "character",
        CodeError::WrongKind { .. } => "wrongKind",
        CodeError::WrongPrefix { .. } => "prefix",
        CodeError::TooLong { .. } => "tooLong",
    });
    KeyInspection {
        groups: i.groups,
        expected_groups: codec::ACTIVATION.groups,
        complete: i.result.is_ok(),
        problem,
        message: i.result.err().map(|e| e.to_string()),
    }
}

impl Licensing {
    pub fn load(locations: Locations, db: Option<&Connection>) -> Self {
        Self::load_with(locations, db, Clock::from_env(), trusted_keys())
    }

    fn load_with(locations: Locations, db: Option<&Connection>, clock: Clock, trusted: Vec<TrustedKey>) -> Self {
        let fp = fingerprint::read();
        let wall = clock.wall();

        let mut records = Vec::new();
        for anchor in locations.all() {
            if let Some(text) = anchor.read() {
                match StateRecord::decode(&text) {
                    Ok(r) => records.push(r),
                    Err(e) => log::warn!("Ignored license state at {}: {e}", anchor.describe()),
                }
            }
        }
        if let Some(r) = db.and_then(read_db) {
            records.push(r);
        }
        log::info!("License state copies found: {}", records.len());

        let mut record = build_record(&records, fp);
        let mut clock_warning = None;
        if !records.is_empty() && wall + st::ROLLBACK_TOLERANCE_SECS < record.last_seen {
            record.rollback_count = record.rollback_count.saturating_add(1);
            clock_warning = Some(record.last_seen);
            log::warn!(
                "The computer clock is behind the last time CODE X MART was used (set-back {} of {})",
                record.rollback_count,
                st::ROLLBACK_LIMIT
            );
        } else if wall < release_floor() {
            clock_warning = Some(release_floor());
        }
        let trusted_now = wall.max(record.last_seen).max(release_floor());
        record.last_seen = trusted_now;
        let licenses = verified(&mut record, &trusted);

        let me = Licensing {
            inner: Mutex::new(Inner {
                record,
                licenses,
                base_wall: wall,
                base_trusted: trusted_now,
                base_instant: Instant::now(),
                rollback_noticed: clock_warning.is_some(),
                clock_warning,
            }),
            locations,
            trusted,
            clock,
        };
        {
            let mut inner = me.lock();
            if let Some(c) = db {
                me.apply_database(&mut inner, c);
            }
            me.persist(&mut inner, db, true);
            let e = me.evaluate(&inner);
            log::info!("Device {}: {:?}", format_device_id(&inner.record.device_id), e.state);
        }
        me
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn trusted_now(&self, inner: &Inner) -> i64 {
        let elapsed = inner.base_instant.elapsed().as_secs() as i64;
        self.clock.wall().max(inner.base_trusted + elapsed).max(inner.record.last_seen).max(release_floor())
    }

    fn evaluate(&self, inner: &Inner) -> Evaluation {
        let r = &inner.record;
        evaluate(r.trial_start, self.trusted_now(inner), tz_offset(), &inner.licenses, r.rollback_count)
    }

    /// Merges the state stored in the shop database and starts the trial
    /// at the shop's setup time (or keeps an earlier start found elsewhere).
    fn apply_database(&self, inner: &mut Inner, c: &Connection) {
        if let Some(r) = read_db(c) {
            inner.record.merge_from(&r);
            inner.licenses = verified(&mut inner.record, &self.trusted);
        }
        if let Some(setup) = setup_time(c) {
            let start = setup.max(release_floor()).min(self.trusted_now(inner));
            if inner.record.trial_start == 0 || start < inner.record.trial_start {
                inner.record.trial_start = start;
            }
        }
    }

    /// Called when the database is opened or replaced (setup, restore).
    pub fn on_database(&self, c: &Connection) {
        let mut inner = self.lock();
        self.apply_database(&mut inner, c);
        self.persist(&mut inner, Some(c), true);
    }

    fn persist(&self, inner: &mut Inner, db: Option<&Connection>, everywhere: bool) {
        inner.record.write_seq += 1;
        let text = inner.record.encode();
        let extra = if everywhere { &self.locations.extra[..] } else { &[] };
        for anchor in self.locations.fast.iter().chain(extra) {
            if let Err(e) = anchor.write(&text) {
                log::debug!("Could not update license state at {}: {e}", anchor.describe());
            }
        }
        if let Some(c) = db {
            if let Err(e) = db::meta_set(c, DB_KEY, &text) {
                log::warn!("Could not save license state in the database: {e}");
            }
        }
    }

    /// Periodic refresh: records the time, notices a clock set back while
    /// running, and rewrites the state copies.
    pub fn refresh(&self, db: Option<&Connection>) {
        let mut inner = self.lock();
        let wall = self.clock.wall();
        let expected = inner.base_wall + inner.base_instant.elapsed().as_secs() as i64;
        if !inner.rollback_noticed && wall + st::ROLLBACK_TOLERANCE_SECS < expected {
            inner.rollback_noticed = true;
            inner.record.rollback_count = inner.record.rollback_count.saturating_add(1);
            inner.clock_warning = Some(expected);
            log::warn!("The computer clock was set back while CODE X MART was running");
        }
        let now = self.trusted_now(&inner);
        inner.record.last_seen = now;
        self.persist(&mut inner, db, false);
    }

    pub fn access_allowed(&self) -> bool {
        self.evaluate(&self.lock()).allowed
    }

    pub fn require_access(&self) -> AppResult<()> {
        if self.access_allowed() {
            Ok(())
        } else {
            Err(AppError::LicenseRequired(self.status().message))
        }
    }

    pub fn status(&self) -> LicenseStatus {
        let inner = self.lock();
        let e = self.evaluate(&inner);
        let r = &inner.record;
        let tz = tz_offset();
        let license = e.license.as_ref().map(LicenseInfo::from);
        let (headline, message) = match e.state {
            AccessState::NotStarted => (
                "Free trial".to_string(),
                format!("Your {}-day free trial starts when the shop is set up.", st::TRIAL_DAYS),
            ),
            AccessState::Trial if e.reminder => {
                let left = e.trial_days_left.unwrap_or(0);
                (
                    format!("Your free trial expires {}", in_days(left)),
                    format!(
                        "Your free trial expires {} (on {}). Contact {OWNER_NAME} to activate CODE X MART.",
                        in_days(left),
                        date_text(e.trial_last_day.unwrap_or(e.today))
                    ),
                )
            }
            AccessState::Trial => (
                "Free trial".to_string(),
                format!(
                    "Day {} of {} · the free trial ends on {}.",
                    e.trial_day.unwrap_or(1),
                    st::TRIAL_DAYS,
                    date_text(e.trial_last_day.unwrap_or(e.today))
                ),
            ),
            AccessState::TrialExpired if e.trial_ended_by_clock => (
                "Your free trial has ended.".to_string(),
                "The computer's date was set back several times, so the free trial ended early. Please contact the software owner to purchase and activate your license.".to_string(),
            ),
            AccessState::TrialExpired => (
                "Your free trial has expired.".to_string(),
                "Your CODE X MART free trial has expired. Please contact the software owner to purchase and activate your license.".to_string(),
            ),
            AccessState::Licensed => {
                let l = e.license.as_ref().unwrap();
                match (l.expires_on(), e.license_days_left) {
                    (Some(end), Some(left)) if e.reminder => (
                        format!("Your license expires {}", in_days(left)),
                        format!(
                            "Your CODE X MART license expires {} (on {}). Contact {OWNER_NAME} to renew it.",
                            in_days(left),
                            date_text(end)
                        ),
                    ),
                    (Some(end), _) => (
                        "Licensed".to_string(),
                        format!("{} license, valid until {}.", l.edition.label(), date_text(end)),
                    ),
                    (None, _) => ("Licensed".to_string(), "Lifetime license. Thank you for choosing CODE X MART.".to_string()),
                }
            }
            AccessState::LicenseExpired => {
                let end = e.license.as_ref().and_then(|l| l.expires_on()).unwrap_or(e.today);
                (
                    "Your license has expired.".to_string(),
                    format!(
                        "Your CODE X MART license expired on {}. Please contact the software owner to renew your license.",
                        date_text(end)
                    ),
                )
            }
        };
        let clock_warning = inner.clock_warning.map(|t| {
            format!(
                "This computer's date and time are earlier than when CODE X MART was last used ({}). Please correct the date and time in Windows settings.",
                st::local_date(t, tz).format("%d %b %Y")
            )
        });
        LicenseStatus {
            state: e.state,
            allowed: e.allowed,
            today: e.today.to_string(),
            trial_days: st::TRIAL_DAYS,
            trial_started_on: (r.trial_start > 0).then(|| st::local_date(r.trial_start, tz).to_string()),
            trial_day: e.trial_day,
            trial_days_left: e.trial_days_left,
            trial_last_day: e.trial_last_day.map(|d| d.to_string()),
            trial_ended_by_clock: e.trial_ended_by_clock,
            license,
            license_days_left: e.license_days_left,
            reminder: e.reminder && e.allowed,
            clock_warning,
            device_id: format_device_id(&r.device_id),
            owner: OWNER,
            headline,
            message,
        }
    }

    pub fn request_code(&self) -> RequestCodeInfo {
        let status_state = self.status().state;
        let inner = self.lock();
        let mut version = APP_VERSION.split('.').map(|p| p.parse::<u8>().unwrap_or(0));
        let info = RequestInfo {
            device_id: inner.record.device_id,
            app_major: version.next().unwrap_or(0),
            app_minor: version.next().unwrap_or(0),
            trial_start_day: (inner.record.trial_start > 0)
                .then(|| day_number(st::local_date(inner.record.trial_start, tz_offset())))
                .flatten(),
            status: match status_state {
                AccessState::NotStarted => RequestStatus::NotStarted,
                AccessState::Trial => RequestStatus::Trial,
                AccessState::TrialExpired => RequestStatus::TrialExpired,
                AccessState::Licensed => RequestStatus::Licensed,
                AccessState::LicenseExpired => RequestStatus::LicenseExpired,
            },
            clock_warning: inner.clock_warning.is_some() || inner.record.rollback_count > 0,
        };
        RequestCodeInfo {
            request_code: encode_request(&info),
            device_id: format_device_id(&inner.record.device_id),
            app_version: APP_VERSION,
            product: codexmart_core::APP_NAME,
        }
    }

    /// Verifies an activation key offline and, if it is valid for this
    /// computer, stores it in every state copy.
    pub fn activate(&self, input: &str, db: Option<&Connection>) -> AppResult<License> {
        let invalid = |e: LicenseError| AppError::Validation(e.to_string());
        let (_, token) = decode_activation_key(input).map_err(invalid)?;
        let mut inner = self.lock();
        let license = verify_token(&token, &inner.record.device_id, &self.trusted).map_err(invalid)?;

        let tz = tz_offset();
        let wall = self.clock.wall();
        let issued_start = license.issued_on().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() - tz as i64;
        if wall + 86_400 < issued_start {
            return Err(invalid(LicenseError::ClockBehind(license.issued_on())));
        }
        // The owner's signed issue date confirms today's date. If the stored
        // "last seen" time is far beyond it, the clock was once set ahead by
        // mistake: a new license resets it so the license is not cut short.
        let known = inner.record.tokens.iter().any(|t| t[4..13] == license.license_id);
        let newest = inner.licenses.iter().all(|l| l.issued_day <= license.issued_day);
        if !known && newest && inner.record.last_seen > wall.max(issued_start) + 2 * 86_400 {
            log::warn!("Resetting a clock that was set ahead, confirmed by license {}", license.serial());
            inner.record.last_seen = wall.max(issued_start);
            inner.record.rollback_count = 0;
            inner.base_wall = wall;
            inner.base_trusted = inner.record.last_seen;
            inner.base_instant = Instant::now();
            inner.clock_warning = None;
        }
        let today = st::local_date(self.trusted_now(&inner), tz);
        if let Some(end) = license.expires_on().filter(|_| !license.is_active_on(today)) {
            return Err(invalid(LicenseError::Expired(end)));
        }

        inner.record.add_token(token);
        inner.licenses = verified(&mut inner.record, &self.trusted);
        self.persist(&mut inner, db, true);
        log::info!("License {} ({}) activated", license.serial(), license.edition.label());
        Ok(license)
    }
}

/// Refreshes the state record every few minutes while the app runs.
pub fn start_refresh(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        std::thread::sleep(REFRESH_EVERY);
        let state = app.state::<crate::state::AppState>();
        let refreshed = state.with_conn(|c| {
            state.licensing.refresh(Some(c));
            Ok(())
        });
        if refreshed.is_err() {
            // The database cannot be opened right now: refresh the other copies.
            state.licensing.refresh(None);
        }
    });
}

#[cfg(test)]
mod tests {
    use codexmart_license::license::{issue_license, new_license_id, Edition, IssueInput};

    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cxm-licensing-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn shop_db() -> Connection {
        let mut c = db::open_in_memory().unwrap();
        codexmart_core::db::migrations::migrate(&mut c, "2026-11-01T00:00:00.000Z").unwrap();
        c
    }

    const DAY: i64 = 86_400;

    fn load(dir: &std::path::Path, c: &Connection, days_later: i64) -> Licensing {
        Licensing::load_with(Locations::isolated(dir), Some(c), Clock { offset: days_later * DAY }, test_keys())
    }

    fn owner_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[42; 32])
    }

    fn test_keys() -> Vec<TrustedKey> {
        let public_key = owner_key().verifying_key().to_bytes();
        vec![TrustedKey { key_id: codexmart_license::keys::key_id_for(&public_key), public_key, label: "test" }]
    }

    fn issue_for(l: &Licensing, edition: Edition, issued: NaiveDate) -> String {
        let request = codexmart_license::request::decode_request(&l.request_code().request_code).unwrap();
        let input =
            IssueInput { device_id: request.device_id, edition, issued, expires: None, customer_no: 1, flags: 0 };
        issue_license(&owner_key(), &input, new_license_id()).unwrap().1
    }

    fn today_plus(days: i64) -> NaiveDate {
        Local::now().date_naive() + chrono::Duration::days(days)
    }

    fn set_up(c: &mut Connection, at: i64) {
        let input = codexmart_core::services::auth::SetupInput {
            store_name: "Test Mart".into(),
            store_phone: None,
            store_address: None,
            currency_symbol: None,
            admin_full_name: "Owner".into(),
            admin_username: "owner".into(),
            admin_password: "owner123".into(),
        };
        codexmart_core::services::auth::setup(c, input, DateTime::from_timestamp(at, 0).unwrap()).unwrap();
    }

    #[test]
    fn trial_starts_at_setup_and_survives_losing_copies() {
        let dir = temp_dir("copies");
        let mut c = shop_db();
        let l = load(&dir, &c, 0);
        assert_eq!(l.status().state, AccessState::NotStarted);
        set_up(&mut c, Utc::now().timestamp());
        l.on_database(&c);
        let s = l.status();
        assert_eq!(s.state, AccessState::Trial);
        assert_eq!(s.trial_day, Some(1));
        assert!(!s.reminder);
        assert_eq!(load(&dir, &c, 3).status().trial_day, Some(4));

        // Delete every file copy: the database copy and setup time remain.
        for f in std::fs::read_dir(&dir).unwrap() {
            std::fs::remove_file(f.unwrap().path()).unwrap();
        }
        assert_eq!(load(&dir, &c, 3).status().trial_day, Some(4));

        // A brand-new database (shop data deleted): the file copies written
        // by the previous load still hold the original start.
        let fresh = shop_db();
        assert_eq!(load(&dir, &fresh, 3).status().trial_day, Some(4));
    }

    #[test]
    fn edited_copies_are_ignored() {
        let dir = temp_dir("edited");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        let l = load(&dir, &c, 10);
        assert_eq!(l.status().trial_day, Some(11));
        let path = dir.join("appdata.dat");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.replace_range(80..82, "00");
        std::fs::write(&path, text).unwrap();
        assert_eq!(load(&dir, &c, 10).status().trial_day, Some(11));
    }

    #[test]
    fn blocked_state_rejects_business_access_but_keeps_data() {
        let dir = temp_dir("blocked");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        let users_before: i64 = c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap();
        let l = load(&dir, &c, 40);
        let s = l.status();
        assert_eq!(s.state, AccessState::TrialExpired);
        assert!(!l.access_allowed());
        assert!(matches!(l.require_access(), Err(AppError::LicenseRequired(_))));
        let users_after: i64 = c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap();
        assert_eq!(users_before, users_after);
        let code = l.request_code();
        assert!(codexmart_license::request::decode_request(&code.request_code).is_ok());
    }

    #[test]
    fn invalid_keys_are_explained_and_change_nothing() {
        let dir = temp_dir("invalid");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        let l = load(&dir, &c, 40);
        let before = std::fs::read_to_string(dir.join("appdata.dat")).unwrap();
        let err = l.activate("CXMA-2222", Some(&c)).unwrap_err().to_string();
        assert!(err.contains("incomplete"), "{err}");
        assert!(!l.access_allowed());
        assert_eq!(std::fs::read_to_string(dir.join("appdata.dat")).unwrap(), before);
    }

    #[test]
    fn clock_set_back_is_detected_and_gives_no_days_back() {
        let dir = temp_dir("rollback");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        assert_eq!(load(&dir, &c, 10).status().trial_day, Some(11));
        // Clock moved back five days: still day 11, with a warning.
        let back = load(&dir, &c, 5);
        let s = back.status();
        assert_eq!(s.trial_day, Some(11));
        assert!(s.clock_warning.is_some());
        assert!(s.allowed);
        // Repeated set-backs end the trial.
        load(&dir, &c, 5);
        let third = load(&dir, &c, 5).status();
        assert_eq!(third.state, AccessState::TrialExpired);
        assert!(third.trial_ended_by_clock);
    }

    #[test]
    fn activation_works_offline_and_survives_restart() {
        let dir = temp_dir("activate");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        let l = load(&dir, &c, 31);
        assert_eq!(l.status().state, AccessState::TrialExpired);
        let key = issue_for(&l, Edition::Year1, today_plus(31));
        let license = l.activate(&key, Some(&c)).unwrap();
        assert_eq!(license.edition, Edition::Year1);
        let s = l.status();
        assert_eq!(s.state, AccessState::Licensed);
        assert!(s.allowed);
        // Restart: every copy holds the license.
        assert_eq!(load(&dir, &c, 31).status().state, AccessState::Licensed);
        // Even with only the database copy left.
        for f in std::fs::read_dir(&dir).unwrap() {
            std::fs::remove_file(f.unwrap().path()).unwrap();
        }
        assert_eq!(load(&dir, &c, 32).status().state, AccessState::Licensed);
        // A year and a bit later the license has expired.
        let later = load(&dir, &c, 31 + 370);
        assert_eq!(later.status().state, AccessState::LicenseExpired);
        assert!(!later.access_allowed());
    }

    #[test]
    fn lifetime_license_and_wrong_device_and_expired_keys() {
        let dir = temp_dir("kinds");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        let l = load(&dir, &c, 40);
        // A key for another computer.
        let input = IssueInput {
            device_id: [7; 10],
            edition: Edition::Lifetime,
            issued: today_plus(40),
            expires: None,
            customer_no: 1,
            flags: 0,
        };
        let other = issue_license(&owner_key(), &input, new_license_id()).unwrap().1;
        assert!(l.activate(&other, Some(&c)).unwrap_err().to_string().contains("different computer"));
        // An expired key.
        let old = issue_for(&l, Edition::Month1, today_plus(-20));
        assert!(l.activate(&old, Some(&c)).unwrap_err().to_string().contains("expired"));
        assert!(!l.access_allowed());
        // Lifetime.
        let lifetime = issue_for(&l, Edition::Lifetime, today_plus(40));
        l.activate(&lifetime, Some(&c)).unwrap();
        assert_eq!(load(&dir, &c, 4000).status().state, AccessState::Licensed);
    }

    #[test]
    fn restoring_an_old_backup_does_not_restart_the_trial() {
        let dir = temp_dir("restore");
        let mut c = shop_db();
        set_up(&mut c, Utc::now().timestamp());
        load(&dir, &c, 0);
        // The database copy as it was on day 1, i.e. what a day-1 backup holds.
        let backup_state = db::meta_get(&c, DB_KEY).unwrap().unwrap();
        let running = load(&dir, &c, 26);
        assert!(running.status().reminder);
        // "Restore" the day-1 backup: its database copy is older.
        db::meta_set(&c, DB_KEY, &backup_state).unwrap();
        running.on_database(&c);
        assert_eq!(running.status().trial_day, Some(27));
    }

    #[test]
    fn key_inspection_reports_groups() {
        let i = inspect_key("CXMA-2345");
        assert!(!i.complete);
        assert_eq!(i.problem, Some("incomplete"));
        assert_eq!(i.groups.len(), 1);
    }
}
