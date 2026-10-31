//! Backup and restore.
//!
//! A backup is a single `.cxmbak` file (a zip archive) containing:
//! * `codexmart.db`  – a consistent snapshot made with `VACUUM INTO`
//!   (safe while the app is running), and
//! * `manifest.json` – app/schema version, record counts and the SHA-256 of
//!   the database file.
//!
//! Restore never trusts a file: the manifest, checksum, SQLite integrity
//! check, application marker and schema version are all verified on an
//! extracted copy before the live database is touched. The live database is
//! then replaced atomically (write to a temporary file, then rename).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::context::timestamp;
use crate::db::{self, migrations};
use crate::error::{AppError, AppResult};
use crate::ids::new_id;

pub const EXTENSION: &str = "cxmbak";
pub const FORMAT: &str = "codexmart-backup";
pub const FORMAT_VERSION: u32 = 1;
const DB_ENTRY: &str = "codexmart.db";
const MANIFEST_ENTRY: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub format_version: u32,
    pub app_name: String,
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub kind: String,
    pub db_sha256: String,
    pub db_size_bytes: u64,
    pub db_id: Option<String>,
    pub device_id: Option<String>,
    pub store_name: Option<String>,
    pub counts: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub manifest: BackupManifest,
}

fn sha256_file(path: &Path) -> AppResult<(String, u64)> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}

const COUNTED_TABLES: &[&str] = &[
    "products",
    "sales",
    "sale_returns",
    "customers",
    "suppliers",
    "purchases",
    "expenses",
    "stock_movements",
    "users",
];

fn record_counts(conn: &Connection) -> AppResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for t in COUNTED_TABLES {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?;
        counts.insert((*t).to_string(), n);
    }
    Ok(counts)
}

fn unique_name(dir: &Path, kind: &str, now: DateTime<Utc>) -> PathBuf {
    let stamp = now.with_timezone(&Local).format("%Y%m%d-%H%M%S");
    let mut candidate = dir.join(format!("CODEXMART_{kind}_{stamp}.{EXTENSION}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("CODEXMART_{kind}_{stamp}-{n}.{EXTENSION}"));
        n += 1;
    }
    candidate
}

/// Creates a verified backup of the open database in `dest_dir`.
/// `work_dir` holds temporary files (never the destination drive's root).
pub fn create_backup(
    conn: &Connection,
    dest_dir: &Path,
    work_dir: &Path,
    kind: &str,
    store_name: Option<String>,
    now: DateTime<Utc>,
) -> AppResult<BackupResult> {
    fs::create_dir_all(dest_dir)
        .map_err(|e| AppError::Backup(format!("Cannot use the backup folder {}: {e}", dest_dir.display())))?;
    fs::create_dir_all(work_dir)?;
    db::quick_check(conn)?;

    let snapshot = work_dir.join(format!("snapshot-{}.db", new_id()));
    let snapshot_str = snapshot.to_string_lossy().to_string();
    conn.execute("VACUUM INTO ?1", [&snapshot_str])
        .map_err(|e| AppError::Backup(format!("Could not take a database snapshot: {e}")))?;

    let result = (|| -> AppResult<BackupResult> {
        let (sha, size) = sha256_file(&snapshot)?;
        let manifest = BackupManifest {
            format: FORMAT.into(),
            format_version: FORMAT_VERSION,
            app_name: crate::APP_NAME.into(),
            app_version: crate::APP_VERSION.into(),
            schema_version: migrations::current_version(conn)?,
            created_at: timestamp(now),
            kind: kind.into(),
            db_sha256: sha,
            db_size_bytes: size,
            db_id: db::meta_get(conn, "db_id")?,
            device_id: db::meta_get(conn, "device_id")?,
            store_name,
            counts: record_counts(conn)?,
        };

        let final_path = unique_name(dest_dir, kind, now);
        let part_path = final_path.with_extension(format!("{EXTENSION}.part"));
        {
            let file = File::create(&part_path)
                .map_err(|e| AppError::Backup(format!("Cannot write to {}: {e}", dest_dir.display())))?;
            let mut zip = ZipWriter::new(BufWriter::new(file));
            let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).large_file(true);
            zip.start_file(MANIFEST_ENTRY, stored)?;
            zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
            zip.start_file(DB_ENTRY, deflated)?;
            let mut src = BufReader::new(File::open(&snapshot)?);
            std::io::copy(&mut src, &mut zip)?;
            let writer = zip.finish()?;
            let file = writer.into_inner().map_err(|e| AppError::Io(e.to_string()))?;
            file.sync_all()?;
        }
        fs::rename(&part_path, &final_path)?;

        // Read the finished file back to prove it is restorable.
        let check = inspect(&final_path, work_dir, migrations::latest_version());
        if let Err(e) = check {
            let _ = fs::remove_file(&final_path);
            return Err(AppError::Backup(format!("The backup failed verification and was removed: {e}")));
        }
        let size_bytes = fs::metadata(&final_path)?.len();
        Ok(BackupResult {
            file_name: final_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            path: final_path.to_string_lossy().to_string(),
            size_bytes,
            manifest,
        })
    })();
    let _ = fs::remove_file(&snapshot);
    result
}

/// A backup that passed every check, extracted to a temporary file.
pub struct ValidatedBackup {
    pub manifest: BackupManifest,
    pub extracted_db: PathBuf,
}

impl Drop for ValidatedBackup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.extracted_db);
    }
}

/// Fully validates a backup file and extracts its database.
pub fn inspect(file: &Path, work_dir: &Path, max_schema: i64) -> AppResult<ValidatedBackup> {
    let bad = |msg: &str| AppError::Backup(format!("This is not a valid CODE X MART backup: {msg}"));
    fs::create_dir_all(work_dir)?;
    let f = File::open(file).map_err(|e| AppError::Backup(format!("Cannot open the backup file: {e}")))?;
    let mut zip = ZipArchive::new(BufReader::new(f)).map_err(|_| bad("the file is damaged or not a backup"))?;

    let manifest: BackupManifest = {
        let mut entry = zip.by_name(MANIFEST_ENTRY).map_err(|_| bad("the manifest is missing"))?;
        let mut text = String::new();
        entry.read_to_string(&mut text).map_err(|_| bad("the manifest cannot be read"))?;
        serde_json::from_str(&text).map_err(|_| bad("the manifest is damaged"))?
    };
    if manifest.format != FORMAT || manifest.format_version > FORMAT_VERSION {
        return Err(bad("unsupported backup format"));
    }
    if manifest.schema_version > max_schema {
        return Err(AppError::Backup(format!(
            "This backup was made by a newer version of CODE X MART (database version {}). Update the app before restoring it.",
            manifest.schema_version
        )));
    }

    let extracted = work_dir.join(format!("restore-{}.db", new_id()));
    let validated = ValidatedBackup { manifest, extracted_db: extracted.clone() };
    {
        let mut entry = zip.by_name(DB_ENTRY).map_err(|_| bad("the database is missing"))?;
        let mut out = BufWriter::new(File::create(&extracted)?);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let n = entry.read(&mut buf).map_err(|_| bad("the database inside is damaged"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            total += n as u64;
        }
        out.flush()?;
        if hex::encode(hasher.finalize()) != validated.manifest.db_sha256 || total != validated.manifest.db_size_bytes {
            return Err(bad("the checksum does not match (the file is corrupted)"));
        }
    }

    let conn = Connection::open_with_flags(&extracted, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| bad("the database cannot be opened"))?;
    let problems = db::integrity_report(&conn).map_err(|_| bad("the database cannot be checked"))?;
    if !problems.is_empty() {
        return Err(bad(&format!("integrity check failed ({})", problems.join("; "))));
    }
    if !db::is_codexmart_db(&conn) {
        return Err(bad("the database does not belong to CODE X MART"));
    }
    let schema = migrations::current_version(&conn)?;
    if schema != validated.manifest.schema_version {
        return Err(bad("the database version does not match its manifest"));
    }
    drop(conn);
    Ok(validated)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCheck {
    pub valid: bool,
    pub message: String,
    pub manifest: Option<BackupManifest>,
    pub file_size: u64,
}

/// Validation result suitable for showing to the user before a restore.
pub fn validate_file(file: &Path, work_dir: &Path) -> BackupCheck {
    let file_size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    match inspect(file, work_dir, migrations::latest_version()) {
        Ok(v) => BackupCheck {
            valid: true,
            message: "The backup passed all checks and can be restored.".into(),
            manifest: Some(v.manifest.clone()),
            file_size,
        },
        Err(e) => BackupCheck { valid: false, message: e.to_string(), manifest: None, file_size },
    }
}

/// Replaces the database file with a validated backup. The caller must
/// have closed every connection to `target` first.
pub fn install(validated: &ValidatedBackup, target: &Path) -> AppResult<()> {
    let step = |what: &str, e: std::io::Error| AppError::Backup(format!("Restore failed while {what}: {e}"));
    let tmp = target.with_extension("db.restoring");
    fs::copy(&validated.extracted_db, &tmp).map_err(|e| step("copying the backup into place", e))?;
    fs::OpenOptions::new()
        .write(true)
        .open(&tmp)
        .and_then(|f| f.sync_all())
        .map_err(|e| step("flushing to disk", e))?;
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", target.display()));
        if side.exists() {
            retry(|| fs::remove_file(&side)).map_err(|e| step("removing the old journal", e))?;
        }
    }
    retry(|| fs::rename(&tmp, target)).map_err(|e| step("replacing the database file", e))?;
    Ok(())
}

/// Windows briefly locks freshly written files (indexing, antivirus), so
/// file replacement is retried for a short while before giving up.
fn retry(mut op: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match op() {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 20 && e.kind() == std::io::ErrorKind::PermissionDenied => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => return Err(e),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub path: String,
    pub file_name: String,
    pub kind: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

/// Backup files in a folder, newest first.
pub fn list_files(dir: &Path) -> Vec<BackupFile> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some(EXTENSION) {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let kind = name.strip_prefix("CODEXMART_").and_then(|r| r.split('_').next()).unwrap_or("unknown").to_string();
        let meta = e.metadata().ok();
        let modified_at = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| timestamp(DateTime::<Utc>::from(t)))
            .unwrap_or_default();
        out.push(BackupFile {
            path: path.to_string_lossy().to_string(),
            file_name: name,
            kind,
            size_bytes: meta.map(|m| m.len()).unwrap_or(0),
            modified_at,
        });
    }
    // Newest first by time written (sorting by name would group by kind).
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at).then_with(|| b.file_name.cmp(&a.file_name)));
    out
}

/// Deletes the oldest automatic backups beyond `keep`. Manual, pre-restore
/// and pre-upgrade backups are never deleted automatically.
pub fn prune_auto(dir: &Path, keep: usize) -> Vec<String> {
    let mut removed = Vec::new();
    let autos: Vec<BackupFile> = list_files(dir).into_iter().filter(|f| f.kind == "auto").collect();
    for f in autos.into_iter().skip(keep) {
        if fs::remove_file(&f.path).is_ok() {
            removed.push(f.file_name);
        }
    }
    removed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub at: String,
    pub action: String,
    pub kind: String,
    pub status: String,
    pub path: Option<String>,
    pub size_bytes: Option<u64>,
    pub message: String,
}

/// Backup history is kept in a plain file next to (not inside) the
/// database so that restoring an older database does not erase it.
pub fn history_append(file: &Path, entry: &HistoryEntry) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(file)?;
    writeln!(f, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

pub fn history_read(file: &Path, limit: usize) -> Vec<HistoryEntry> {
    let Ok(text) = fs::read_to_string(file) else { return Vec::new() };
    let mut entries: Vec<HistoryEntry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    entries.reverse();
    entries.truncate(limit);
    entries
}

/// Last successful backup time recorded in the history file.
pub fn last_success(file: &Path) -> Option<String> {
    history_read(file, 500).into_iter().find(|e| e.action == "backup" && e.status == "success").map(|e| e.at)
}
