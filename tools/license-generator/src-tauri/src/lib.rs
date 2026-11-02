//! CODE X MART License Generator (owner only).
//!
//! Turns a customer's request code into a signed, device-bound activation
//! key. Runs only on the owner's computer; it is a separate program and is
//! never part of the customer installer. Everything works offline.

mod history;
mod keystore;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};
use codexmart_license::device::format_device_id;
use codexmart_license::keys::key_fingerprint;
use codexmart_license::license::{
    date_of_day, issue_license, new_license_id, Edition, IssueInput, FLAG_REISSUE, FLAG_TRANSFER,
};
use codexmart_license::request::{decode_request, RequestInfo};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

pub use history::{History, IssuedRecord};
pub use keystore::{KeyInfo, KeyState, KeyStore};

pub type CmdResult<T> = Result<T, String>;

/// Locks again after this long without use.
const IDLE_LOCK: Duration = Duration::from_secs(15 * 60);
const MAX_FAILURES: u32 = 5;
const FAILURE_PAUSE: Duration = Duration::from_secs(30);

/// `%APPDATA%\CODE X MART License Generator`, or `CXM_LICENSEGEN_DIR`.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CXM_LICENSEGEN_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("CODE X MART License Generator")
}

struct Unlocked {
    key: SigningKey,
    last_used: Instant,
}

#[derive(Default)]
struct Failures {
    count: u32,
    until: Option<Instant>,
}

pub struct Gen {
    pub store: KeyStore,
    history: Mutex<History>,
    unlocked: Mutex<Option<Unlocked>>,
    failures: Mutex<Failures>,
    pub dir: PathBuf,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenStatus {
    /// `none`, `pending`, `locked` or `unlocked`.
    pub key_state: &'static str,
    pub key: Option<KeyInfo>,
    pub data_dir: String,
    pub history_count: i64,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPreview {
    pub device_id: String,
    pub app_version: String,
    pub trial_started_on: Option<String>,
    pub status: String,
    pub status_label: &'static str,
    pub clock_warning: bool,
    /// Licenses already issued for this computer, newest first.
    pub previous: Vec<IssuedRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRequest {
    pub request_code: String,
    pub customer_name: String,
    #[serde(default)]
    pub customer_phone: String,
    pub edition: String,
    pub issued_on: String,
    #[serde(default)]
    pub expires_on: Option<String>,
    #[serde(default)]
    pub notes: String,
    /// `new`, `reissue` or `transfer`.
    pub kind: String,
    #[serde(default)]
    pub replaces: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePreview {
    pub device_id: String,
    pub customer_name: String,
    pub customer_phone: String,
    pub edition: String,
    pub edition_label: &'static str,
    pub issued_on: String,
    pub expires_on: Option<String>,
    pub kind: String,
    pub replaces: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueResult {
    pub record: IssuedRecord,
    pub whatsapp_message: String,
}

struct Plan {
    request: RequestInfo,
    device_text: String,
    edition: Edition,
    issued: NaiveDate,
    expires: Option<NaiveDate>,
    preview: IssuePreview,
}

fn parse_date(label: &str, text: &str) -> CmdResult<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").map_err(|_| format!("Enter a valid {label}."))
}

/// Activation key split into lines of six groups, for WhatsApp.
pub fn key_lines(key: &str) -> String {
    let parts: Vec<&str> = key.split('-').collect();
    let mut lines = vec![parts[0].to_string()];
    for chunk in parts[1..].chunks(6) {
        lines.push(chunk.join("-"));
    }
    lines.join("\n")
}

fn whatsapp_message(r: &IssuedRecord) -> String {
    let validity = match &r.expires_on {
        Some(d) => format!("valid until {d}"),
        None => "lifetime (no expiry)".into(),
    };
    format!(
        "CODE X MART activation key\nCustomer: {}\nLicense: {}, {}\nLicense no.: {}\nComputer: {}\n\n{}\n\nIn CODE X MART, enter this key under Activation key and press Activate License. No internet is needed.",
        r.customer_name,
        r.edition_label,
        validity,
        r.serial,
        r.device_id,
        key_lines(&r.activation_key)
    )
}

impl Gen {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        let history = History::open(&dir.join("license-history.db"))?;
        Ok(Gen {
            store: KeyStore::new(dir.clone()),
            history: Mutex::new(history),
            unlocked: Mutex::new(None),
            failures: Mutex::new(Failures::default()),
            dir,
        })
    }

    pub fn status(&self) -> CmdResult<GenStatus> {
        self.expire_idle();
        let (key_state, key) = match self.store.state()? {
            KeyState::None => ("none", None),
            KeyState::Pending(i) => ("pending", Some(i)),
            KeyState::Ready(i) => (if lock(&self.unlocked).is_some() { "unlocked" } else { "locked" }, Some(i)),
        };
        Ok(GenStatus {
            key_state,
            key,
            data_dir: self.dir.display().to_string(),
            history_count: lock(&self.history).count()?,
            version: env!("CARGO_PKG_VERSION"),
        })
    }

    fn expire_idle(&self) {
        let mut u = lock(&self.unlocked);
        if u.as_ref().is_some_and(|s| s.last_used.elapsed() > IDLE_LOCK) {
            *u = None;
        }
    }

    fn set_unlocked(&self, key: SigningKey) {
        *lock(&self.unlocked) = Some(Unlocked { key, last_used: Instant::now() });
    }

    /// The signing key, if unlocked and a backup has been saved.
    fn signing_key(&self) -> CmdResult<SigningKey> {
        self.expire_idle();
        let mut u = lock(&self.unlocked);
        let s = u.as_mut().ok_or("The generator is locked. Enter your passphrase.")?;
        s.last_used = Instant::now();
        Ok(s.key.clone())
    }

    fn require_backup(&self) -> CmdResult<()> {
        match self.store.current_info()? {
            Some(i) if i.last_backup_at.is_some() => Ok(()),
            _ => Err("Save a backup of your signing key first (Signing key → Save key backup). Without a backup, losing this computer means you can no longer issue licenses.".into()),
        }
    }

    pub fn create_key(&self, passphrase: &str) -> CmdResult<KeyInfo> {
        let (info, key) = self.store.create(passphrase)?;
        self.set_unlocked(key);
        Ok(info)
    }

    pub fn restore_key(&self, path: &str, passphrase: &str) -> CmdResult<KeyInfo> {
        let (info, key) = self.store.restore(std::path::Path::new(path), passphrase)?;
        self.set_unlocked(key);
        Ok(info)
    }

    pub fn set_passphrase(&self, passphrase: &str) -> CmdResult<()> {
        let key = self.store.set_passphrase(passphrase)?;
        self.set_unlocked(key);
        Ok(())
    }

    pub fn unlock(&self, passphrase: &str) -> CmdResult<()> {
        {
            let f = lock(&self.failures);
            if let Some(until) = f.until.filter(|u| *u > Instant::now()) {
                return Err(format!(
                    "Too many wrong passphrases. Try again in {} seconds.",
                    (until - Instant::now()).as_secs() + 1
                ));
            }
        }
        match self.store.unlock(passphrase) {
            Ok(key) => {
                *lock(&self.failures) = Failures::default();
                self.set_unlocked(key);
                Ok(())
            }
            Err(e) => {
                let mut f = lock(&self.failures);
                f.count += 1;
                if f.count >= MAX_FAILURES {
                    f.count = 0;
                    f.until = Some(Instant::now() + FAILURE_PAUSE);
                }
                Err(e)
            }
        }
    }

    pub fn lock_now(&self) {
        *lock(&self.unlocked) = None;
    }

    pub fn change_passphrase(&self, current: &str, new: &str) -> CmdResult<()> {
        self.signing_key()?;
        self.store.change_passphrase(current, new)
    }

    pub fn backup_key(&self, path: &str) -> CmdResult<KeyInfo> {
        self.signing_key()?;
        self.store.export_backup(std::path::Path::new(path))
    }

    pub fn decode(&self, code: &str) -> CmdResult<RequestPreview> {
        let r = decode_request(code).map_err(|e| e.to_string())?;
        let device_id = format_device_id(&r.device_id);
        Ok(RequestPreview {
            previous: lock(&self.history).for_device(&device_id)?,
            device_id,
            app_version: format!("{}.{}", r.app_major, r.app_minor),
            trial_started_on: r.trial_start_day.map(|d| date_of_day(d).to_string()),
            status: format!("{:?}", r.status),
            status_label: r.status.label(),
            clock_warning: r.clock_warning,
        })
    }

    fn plan(&self, input: &IssueRequest) -> CmdResult<Plan> {
        let request = decode_request(&input.request_code).map_err(|e| e.to_string())?;
        let device_text = format_device_id(&request.device_id);
        let name = input.customer_name.trim();
        if name.is_empty() {
            return Err("Enter the customer's name or reference.".into());
        }
        let edition = Edition::from_code(&input.edition).ok_or("Choose the license type.")?;
        let issued = parse_date("issue date", &input.issued_on)?;
        let expires = match edition {
            Edition::Lifetime => None,
            Edition::Custom => Some(parse_date(
                "expiry date",
                input.expires_on.as_deref().ok_or("Choose the expiry date for a custom license.")?,
            )?),
            e => e.default_expiry(issued),
        };
        if expires.is_some_and(|end| end < issued) {
            return Err("The expiry date cannot be before the issue date.".into());
        }

        let history = lock(&self.history);
        let previous: Vec<IssuedRecord> =
            history.for_device(&device_text)?.into_iter().filter(|r| r.status == "issued").collect();
        let mut warnings = Vec::new();
        match input.kind.as_str() {
            "new" if !previous.is_empty() => {
                return Err(format!(
                    "This computer already has license {} ({}). Choose \"Renew or reissue for this computer\" to issue another one.",
                    previous[0].serial, previous[0].edition_label
                ))
            }
            "new" => {}
            "reissue" => {
                if previous.is_empty() {
                    warnings.push("No earlier license for this computer is in the history.".into());
                } else {
                    warnings.push(format!("Earlier licenses for this computer will be marked as replaced: {}.", previous.iter().map(|r| r.serial.as_str()).collect::<Vec<_>>().join(", ")));
                }
            }
            "transfer" => {
                let serial = input.replaces.as_deref().ok_or("Choose the license that moves to the new computer.")?;
                let old = history.get(serial)?.ok_or("The license being transferred was not found in the history.")?;
                if old.device_id == device_text {
                    return Err("This request code is from the same computer as the original license. Use \"Renew or reissue\" instead.".into());
                }
                warnings.push(format!(
                    "License {} for computer {} will be marked as transferred. CODE X MART cannot switch the old computer off remotely: make sure the customer has stopped using it.",
                    old.serial, old.device_id
                ));
            }
            _ => return Err("Choose new, reissue or transfer.".into()),
        }
        if issued != Local::now().date_naive() {
            warnings.push(format!("The issue date is not today ({}).", Local::now().date_naive()));
        }
        Ok(Plan {
            preview: IssuePreview {
                device_id: device_text.clone(),
                customer_name: name.into(),
                customer_phone: input.customer_phone.trim().into(),
                edition: edition.code().into(),
                edition_label: edition.label(),
                issued_on: issued.to_string(),
                expires_on: expires.map(|d| d.to_string()),
                kind: input.kind.clone(),
                replaces: input.replaces.clone(),
                warnings,
            },
            request,
            device_text,
            edition,
            issued,
            expires,
        })
    }

    pub fn review(&self, input: &IssueRequest) -> CmdResult<IssuePreview> {
        Ok(self.plan(input)?.preview)
    }

    pub fn issue(&self, input: &IssueRequest) -> CmdResult<IssueResult> {
        let key = self.signing_key()?;
        self.require_backup()?;
        let plan = self.plan(input)?;
        let history = lock(&self.history);
        let created = now();
        let customer_no = history.customer_no(&plan.preview.customer_name, &plan.preview.customer_phone, &created)?;
        let flags = match input.kind.as_str() {
            "reissue" => FLAG_REISSUE,
            "transfer" => FLAG_TRANSFER,
            _ => 0,
        };
        let issue_input = IssueInput {
            device_id: plan.request.device_id,
            edition: plan.edition,
            issued: plan.issued,
            expires: plan.expires,
            customer_no: customer_no as u16,
            flags,
        };
        // License ids are random; make sure this one is new to the history.
        let (license, activation_key) = loop {
            let (l, k) = issue_license(&key, &issue_input, new_license_id()).map_err(|e| e.to_string())?;
            if !history.serial_exists(&l.serial())? {
                break (l, k);
            }
        };
        let previous = history.for_device(&plan.device_text)?;
        let record = IssuedRecord {
            serial: license.serial(),
            customer_no,
            customer_name: plan.preview.customer_name.clone(),
            customer_phone: plan.preview.customer_phone.clone(),
            device_id: plan.device_text.clone(),
            edition: plan.edition.code().into(),
            edition_label: plan.edition.label().into(),
            issued_on: license.issued_on().to_string(),
            expires_on: license.expires_on().map(|d| d.to_string()),
            key_fingerprint: key_fingerprint(&key.verifying_key().to_bytes()),
            activation_key,
            request_code: input.request_code.trim().to_uppercase(),
            notes: input.notes.trim().into(),
            kind: input.kind.clone(),
            replaces: input.replaces.clone().filter(|_| input.kind == "transfer"),
            status: "issued".into(),
            created_at: created,
        };
        history.insert(&record)?;
        if input.kind == "reissue" {
            for p in previous.iter().filter(|p| p.status == "issued") {
                history.set_status(&p.serial, "superseded")?;
            }
        }
        if let (Some(old), "transfer") = (&record.replaces, input.kind.as_str()) {
            history.set_status(old, "transferred")?;
        }
        log::info!("Issued {} ({}) for customer {}", record.serial, record.edition_label, customer_no);
        Ok(IssueResult { whatsapp_message: whatsapp_message(&record), record })
    }

    pub fn search(&self, text: &str) -> CmdResult<Vec<IssuedRecord>> {
        lock(&self.history).search(text)
    }

    pub fn export(&self, path: &str) -> CmdResult<usize> {
        lock(&self.history).export_csv(std::path::Path::new(path))
    }
}

// ------------------------------------------------------------- commands

fn check_new_passphrase(passphrase: &str, confirm: &str) -> CmdResult<()> {
    if passphrase != confirm {
        return Err("The two passphrases do not match.".into());
    }
    Ok(())
}

#[tauri::command]
fn gen_status(g: State<'_, Gen>) -> CmdResult<GenStatus> {
    g.status()
}

#[tauri::command]
fn gen_create_key(g: State<'_, Gen>, passphrase: String, confirm: String) -> CmdResult<KeyInfo> {
    check_new_passphrase(&passphrase, &confirm)?;
    g.create_key(&passphrase)
}

#[tauri::command]
fn gen_restore_key(g: State<'_, Gen>, path: String, passphrase: String) -> CmdResult<KeyInfo> {
    g.restore_key(&path, &passphrase)
}

#[tauri::command]
fn gen_set_passphrase(g: State<'_, Gen>, passphrase: String, confirm: String) -> CmdResult<()> {
    check_new_passphrase(&passphrase, &confirm)?;
    g.set_passphrase(&passphrase)
}

#[tauri::command]
fn gen_unlock(g: State<'_, Gen>, passphrase: String) -> CmdResult<()> {
    g.unlock(&passphrase)
}

#[tauri::command]
fn gen_lock(g: State<'_, Gen>) {
    g.lock_now()
}

#[tauri::command]
fn gen_change_passphrase(g: State<'_, Gen>, current: String, passphrase: String, confirm: String) -> CmdResult<()> {
    check_new_passphrase(&passphrase, &confirm)?;
    g.change_passphrase(&current, &passphrase)
}

#[tauri::command]
fn gen_backup_key(g: State<'_, Gen>, path: String) -> CmdResult<KeyInfo> {
    g.backup_key(&path)
}

#[tauri::command]
fn gen_decode_request(g: State<'_, Gen>, code: String) -> CmdResult<RequestPreview> {
    g.decode(&code)
}

#[tauri::command]
fn gen_review(g: State<'_, Gen>, input: IssueRequest) -> CmdResult<IssuePreview> {
    g.review(&input)
}

#[tauri::command]
fn gen_issue(g: State<'_, Gen>, input: IssueRequest) -> CmdResult<IssueResult> {
    g.issue(&input)
}

#[tauri::command]
fn gen_history(g: State<'_, Gen>, query: String) -> CmdResult<Vec<IssuedRecord>> {
    g.search(&query)
}

#[tauri::command]
fn gen_export_history(g: State<'_, Gen>, path: String) -> CmdResult<usize> {
    g.export(&path)
}

/// `--bootstrap-key <file>`: creates the first signing key protected by
/// Windows only (if none exists) and writes its public key to `<file>`, so
/// the key can be built into CODE X MART. The owner sets a passphrase and
/// saves a backup the first time the generator is opened.
pub fn bootstrap(out: &str) -> Result<(), String> {
    let store = KeyStore::new(data_dir());
    let info = match store.current_info()? {
        Some(info) => info,
        None => store.create_pending()?,
    };
    std::fs::write(out, serde_json::to_vec_pretty(&info).unwrap()).map_err(|e| e.to_string())
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--bootstrap-key") {
        let code = match args.get(pos + 1).map(|out| bootstrap(out)) {
            Some(Ok(())) => 0,
            _ => 1,
        };
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let g = Gen::open(data_dir())?;
            app.manage(g);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            gen_status,
            gen_create_key,
            gen_restore_key,
            gen_set_passphrase,
            gen_unlock,
            gen_lock,
            gen_change_passphrase,
            gen_backup_key,
            gen_decode_request,
            gen_review,
            gen_issue,
            gen_history,
            gen_export_history,
        ])
        .run(tauri::generate_context!())
        .expect("CODE X MART License Generator could not start");
}

#[cfg(test)]
mod tests {
    use codexmart_license::device::DeviceId;
    use codexmart_license::keys::{key_id_for, TrustedKey};
    use codexmart_license::license::{decode_activation_key, verify_token};
    use codexmart_license::request::{encode_request, RequestStatus};

    use super::*;

    const PASS: &str = "owner passphrase 1";

    fn request(device: DeviceId) -> String {
        encode_request(&RequestInfo {
            device_id: device,
            app_major: 1,
            app_minor: 1,
            trial_start_day: None,
            status: RequestStatus::TrialExpired,
            clock_warning: false,
        })
    }

    fn input(code: &str, edition: &str, kind: &str) -> IssueRequest {
        IssueRequest {
            request_code: code.into(),
            customer_name: "Al-Noor Mart".into(),
            customer_phone: "0300-1234567".into(),
            edition: edition.into(),
            issued_on: Local::now().date_naive().to_string(),
            expires_on: None,
            notes: "paid cash".into(),
            kind: kind.into(),
            replaces: None,
        }
    }

    fn ready_gen() -> (tempfile::TempDir, Gen) {
        let dir = tempfile::tempdir().unwrap();
        let g = Gen::open(dir.path().to_path_buf()).unwrap();
        g.create_key(PASS).unwrap();
        g.backup_key(dir.path().join("backup.cxkey").to_str().unwrap()).unwrap();
        (dir, g)
    }

    fn trusted(g: &Gen) -> Vec<TrustedKey> {
        let pk = g.signing_key().unwrap().verifying_key().to_bytes();
        vec![TrustedKey { key_id: key_id_for(&pk), public_key: pk, label: "t" }]
    }

    #[test]
    fn issued_keys_verify_on_the_requesting_computer_only() {
        let (_d, g) = ready_gen();
        let device = [3u8; 10];
        let r = g.issue(&input(&request(device), "year1", "new")).unwrap();
        let (_, token) = decode_activation_key(&r.record.activation_key).unwrap();
        let lic = verify_token(&token, &device, &trusted(&g)).unwrap();
        assert_eq!(lic.serial(), r.record.serial);
        assert!(verify_token(&token, &[4u8; 10], &trusted(&g)).is_err());
        assert!(r.whatsapp_message.contains(&r.record.serial));
        assert!(r.whatsapp_message.contains(
            "CXMA
"
        ));
    }

    #[test]
    fn issuing_needs_an_unlocked_key_and_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let g = Gen::open(dir.path().to_path_buf()).unwrap();
        g.create_key(PASS).unwrap();
        let code = request([5; 10]);
        assert!(g.issue(&input(&code, "lifetime", "new")).unwrap_err().contains("backup"));
        g.backup_key(dir.path().join("b.cxkey").to_str().unwrap()).unwrap();
        g.lock_now();
        assert!(g.issue(&input(&code, "lifetime", "new")).unwrap_err().contains("locked"));
        assert!(g.unlock("wrong one!!").is_err());
        g.unlock(PASS).unwrap();
        assert!(g.issue(&input(&code, "lifetime", "new")).is_ok());
    }

    #[test]
    fn duplicates_reissues_and_transfers() {
        let (_d, g) = ready_gen();
        let first = request([6; 10]);
        let a = g.issue(&input(&first, "month1", "new")).unwrap().record;
        // Same computer again as "new": refused with an explanation.
        assert!(g.issue(&input(&first, "month3", "new")).unwrap_err().contains("already has license"));
        // As a reissue: allowed, and the earlier one is marked.
        let preview = g.review(&input(&first, "month3", "reissue")).unwrap();
        assert!(!preview.warnings.is_empty());
        let b = g.issue(&input(&first, "month3", "reissue")).unwrap().record;
        assert_eq!(g.search(&a.serial).unwrap()[0].status, "superseded");
        // Transfer to a new computer.
        let second = request([7; 10]);
        let mut t = input(&second, "month3", "transfer");
        t.replaces = Some(b.serial.clone());
        let c = g.issue(&t).unwrap().record;
        assert_eq!(c.device_id, format_device_id(&[7; 10]));
        assert_eq!(g.search(&b.serial).unwrap()[0].status, "transferred");
        // Transfer onto the same computer is refused.
        let mut same = input(&second, "month3", "transfer");
        same.replaces = Some(c.serial.clone());
        assert!(g.issue(&same).is_err());
        // Same customer reused.
        assert_eq!(a.customer_no, c.customer_no);
    }

    #[test]
    fn history_search_and_export() {
        let (d, g) = ready_gen();
        g.issue(&input(&request([8; 10]), "lifetime", "new")).unwrap();
        g.issue(&input(&request([9; 10]), "custom", "new")).unwrap_err();
        let mut custom = input(&request([9; 10]), "custom", "new");
        custom.expires_on = Some("2030-01-31".into());
        let r = g.issue(&custom).unwrap().record;
        assert_eq!(r.expires_on.as_deref(), Some("2030-01-31"));
        assert_eq!(g.search("al-noor").unwrap().len(), 2);
        assert_eq!(g.search("0300").unwrap().len(), 2);
        assert_eq!(g.search(&r.device_id).unwrap().len(), 1);
        let csv = d.path().join("history.csv");
        assert_eq!(g.export(csv.to_str().unwrap()).unwrap(), 2);
        let text = std::fs::read_to_string(csv).unwrap();
        assert!(text.contains("Lifetime") && text.contains("2030-01-31"));
        assert!(!text.to_lowercase().contains("passphrase"));
    }

    #[test]
    fn bad_request_codes_are_rejected() {
        let (_d, g) = ready_gen();
        assert!(g.decode("CXMR-22222-22222").is_err());
        let code = request([1; 10]);
        let mut chars: Vec<char> = code.chars().collect();
        chars[7] = if chars[7] == 'A' { 'B' } else { 'A' };
        assert!(g.decode(&chars.into_iter().collect::<String>()).is_err());
        assert!(g.decode(&code).is_ok());
    }

    #[test]
    fn key_lines_for_whatsapp() {
        let lines = key_lines("CXMA-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE-FFFFF-GGGGG");
        assert_eq!(lines, "CXMA\nAAAAA-BBBBB-CCCCC-DDDDD-EEEEE-FFFFF\nGGGGG");
    }

    /// Issues a real license with the owner's *pending* production key for
    /// the request code in `CXM_REQUEST_CODE`, writing the activation key to
    /// `CXM_KEY_OUT`. Used once to test the production installer end to end
    /// before the owner sets a passphrase. Run explicitly:
    /// `cargo test -p codexmart-license-generator -- --ignored production_smoke --nocapture`
    #[test]
    #[ignore]
    fn production_smoke_issue_with_pending_key() {
        let code = std::env::var("CXM_REQUEST_CODE").expect("CXM_REQUEST_CODE");
        let out = std::env::var("CXM_KEY_OUT").expect("CXM_KEY_OUT");
        let edition = std::env::var("CXM_EDITION").unwrap_or_else(|_| "lifetime".into());
        let store = KeyStore::new(data_dir());
        let key = store.pending_key().expect("the production key must still be pending");
        let request = decode_request(&code).unwrap();
        let issue_input = IssueInput {
            device_id: request.device_id,
            edition: Edition::from_code(&edition).unwrap(),
            issued: Local::now().date_naive(),
            expires: None,
            customer_no: 0,
            flags: 0,
        };
        let (_, activation_key) = issue_license(&key, &issue_input, new_license_id()).unwrap();
        std::fs::write(out, activation_key).unwrap();
    }
}
