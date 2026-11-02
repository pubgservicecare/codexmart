//! Issued-license history, kept in a local SQLite file on the owner's
//! computer. It holds customer names and phone numbers and every activation
//! key issued (so a key can be sent again), but never the private key.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

pub type Result<T> = std::result::Result<T, String>;

fn db_err(e: rusqlite::Error) -> String {
    format!("License history error: {e}")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedRecord {
    pub serial: String,
    pub customer_no: i64,
    pub customer_name: String,
    pub customer_phone: String,
    pub device_id: String,
    pub edition: String,
    pub edition_label: String,
    pub issued_on: String,
    pub expires_on: Option<String>,
    pub key_fingerprint: String,
    pub activation_key: String,
    pub request_code: String,
    pub notes: String,
    /// `new`, `reissue` or `transfer`.
    pub kind: String,
    pub replaces: Option<String>,
    /// `issued`, `superseded` (a reissue replaced it) or `transferred`.
    pub status: String,
    pub created_at: String,
}

pub struct History {
    conn: Connection,
}

const SELECT: &str = "SELECT l.serial, l.customer_no, c.name, c.phone, l.device_id, l.edition, l.edition_label, l.issued_on,
    l.expires_on, l.key_fingerprint, l.activation_key, l.request_code, l.notes, l.kind, l.replaces, l.status, l.created_at
    FROM licenses l JOIN customers c ON c.no = l.customer_no";

fn record(r: &Row) -> rusqlite::Result<IssuedRecord> {
    Ok(IssuedRecord {
        serial: r.get(0)?,
        customer_no: r.get(1)?,
        customer_name: r.get(2)?,
        customer_phone: r.get(3)?,
        device_id: r.get(4)?,
        edition: r.get(5)?,
        edition_label: r.get(6)?,
        issued_on: r.get(7)?,
        expires_on: r.get(8)?,
        key_fingerprint: r.get(9)?,
        activation_key: r.get(10)?,
        request_code: r.get(11)?,
        notes: r.get(12)?,
        kind: r.get(13)?,
        replaces: r.get(14)?,
        status: r.get(15)?,
        created_at: r.get(16)?,
    })
}

impl History {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(db_err)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS customers (
               no         INTEGER PRIMARY KEY AUTOINCREMENT,
               name       TEXT NOT NULL,
               phone      TEXT NOT NULL DEFAULT '',
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS licenses (
               serial          TEXT PRIMARY KEY,
               customer_no     INTEGER NOT NULL REFERENCES customers(no),
               device_id       TEXT NOT NULL,
               edition         TEXT NOT NULL,
               edition_label   TEXT NOT NULL,
               issued_on       TEXT NOT NULL,
               expires_on      TEXT,
               key_fingerprint TEXT NOT NULL,
               activation_key  TEXT NOT NULL,
               request_code    TEXT NOT NULL,
               notes           TEXT NOT NULL DEFAULT '',
               kind            TEXT NOT NULL CHECK (kind IN ('new','reissue','transfer')),
               replaces        TEXT,
               status          TEXT NOT NULL DEFAULT 'issued' CHECK (status IN ('issued','superseded','transferred')),
               created_at      TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS licenses_device ON licenses(device_id);",
        )
        .map_err(db_err)?;
        Ok(History { conn })
    }

    /// Finds a customer by name and phone (ignoring case), or adds one.
    /// Customer numbers fit in the 16-bit field of a license.
    pub fn customer_no(&self, name: &str, phone: &str, now: &str) -> Result<i64> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT no FROM customers WHERE lower(name) = lower(?1) AND phone = ?2 ORDER BY no LIMIT 1",
                params![name.trim(), phone.trim()],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if let Some(no) = existing {
            return Ok(no);
        }
        self.conn
            .execute(
                "INSERT INTO customers (name, phone, created_at) VALUES (?1, ?2, ?3)",
                params![name.trim(), phone.trim(), now],
            )
            .map_err(db_err)?;
        let no = self.conn.last_insert_rowid();
        if no > u16::MAX as i64 {
            return Err("The customer list is full (65,535 customers).".into());
        }
        Ok(no)
    }

    pub fn serial_exists(&self, serial: &str) -> Result<bool> {
        self.conn
            .query_row("SELECT 1 FROM licenses WHERE serial = ?1", [serial], |_| Ok(()))
            .optional()
            .map(|r| r.is_some())
            .map_err(db_err)
    }

    pub fn insert(&self, r: &IssuedRecord) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO licenses (serial, customer_no, device_id, edition, edition_label, issued_on, expires_on,
                   key_fingerprint, activation_key, request_code, notes, kind, replaces, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    r.serial,
                    r.customer_no,
                    r.device_id,
                    r.edition,
                    r.edition_label,
                    r.issued_on,
                    r.expires_on,
                    r.key_fingerprint,
                    r.activation_key,
                    r.request_code,
                    r.notes,
                    r.kind,
                    r.replaces,
                    r.status,
                    r.created_at
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn set_status(&self, serial: &str, status: &str) -> Result<()> {
        self.conn
            .execute("UPDATE licenses SET status = ?2 WHERE serial = ?1", params![serial, status])
            .map_err(db_err)?;
        Ok(())
    }

    fn query(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<IssuedRecord>> {
        let mut stmt = self.conn.prepare(sql).map_err(db_err)?;
        let rows = stmt.query_map(args, record).map_err(db_err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(db_err)
    }

    /// Licenses issued for a device, newest first.
    pub fn for_device(&self, device_id: &str) -> Result<Vec<IssuedRecord>> {
        self.query(&format!("{SELECT} WHERE l.device_id = ?1 ORDER BY l.created_at DESC"), [device_id])
    }

    pub fn get(&self, serial: &str) -> Result<Option<IssuedRecord>> {
        Ok(self.query(&format!("{SELECT} WHERE l.serial = ?1"), [serial])?.into_iter().next())
    }

    /// Searches by customer name or phone, serial, device id or notes.
    pub fn search(&self, text: &str) -> Result<Vec<IssuedRecord>> {
        let like = format!("%{}%", text.trim().replace(['%', '_'], ""));
        self.query(
            &format!(
                "{SELECT} WHERE c.name LIKE ?1 OR c.phone LIKE ?1 OR l.serial LIKE ?1 OR l.device_id LIKE ?1 OR l.notes LIKE ?1
                 OR CAST(l.customer_no AS TEXT) = ?2
                 ORDER BY l.created_at DESC LIMIT 500"
            ),
            params![like, text.trim()],
        )
    }

    pub fn count(&self) -> Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM licenses", [], |r| r.get(0)).map_err(db_err)
    }

    /// CSV of the history (no key material besides the activation keys
    /// themselves, which only work on their own computers).
    pub fn export_csv(&self, path: &Path) -> Result<usize> {
        let rows = self.query(&format!("{SELECT} ORDER BY l.created_at"), [])?;
        let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("\u{feff}Serial,Customer no,Customer,Phone,Device ID,License type,Issued on,Expires on,Kind,Replaces,Status,Notes,Created at,Activation key\r\n");
        for r in &rows {
            let line = [
                r.serial.clone(),
                r.customer_no.to_string(),
                r.customer_name.clone(),
                r.customer_phone.clone(),
                r.device_id.clone(),
                r.edition_label.clone(),
                r.issued_on.clone(),
                r.expires_on.clone().unwrap_or_else(|| "Lifetime".into()),
                r.kind.clone(),
                r.replaces.clone().unwrap_or_default(),
                r.status.clone(),
                r.notes.clone(),
                r.created_at.clone(),
                r.activation_key.clone(),
            ]
            .iter()
            .map(|v| esc(v))
            .collect::<Vec<_>>()
            .join(",");
            out.push_str(&line);
            out.push_str("\r\n");
        }
        std::fs::write(path, out).map_err(|e| format!("Could not save the export: {e}"))?;
        Ok(rows.len())
    }
}
