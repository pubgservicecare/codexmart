//! Store, receipt, feature and backup settings.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{audit, begin};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::repo::{sequences, settings as repo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // Store identity (printed on receipts)
    pub store_name: String,
    pub store_tagline: String,
    pub store_address: String,
    pub store_phone: String,
    pub store_email: String,
    pub store_tax_id: String,
    // Money
    pub currency_symbol: String,
    /// 0 or 2 decimals when displaying amounts.
    pub currency_decimals: u8,
    // Receipts and printing
    pub receipt_header: String,
    pub receipt_footer: String,
    /// "80mm", "58mm" or "a4".
    pub receipt_paper: String,
    /// "dialog" (Windows print dialog) or "escpos" (direct to thermal printer).
    pub receipt_print_mode: String,
    pub receipt_printer: String,
    pub receipt_copies: u8,
    pub auto_print_receipt: bool,
    pub open_cash_drawer: bool,
    pub label_size: String,
    // Numbering
    pub invoice_prefix: String,
    // Features and rules
    pub allow_negative_stock: bool,
    pub enable_credit_sales: bool,
    pub enable_batch_tracking: bool,
    pub enable_variants: bool,
    pub enable_wholesale: bool,
    /// Largest bill discount a cashier may give, in basis points (10000 = no limit).
    pub max_cashier_discount_bp: i64,
    pub expiry_warning_days: i64,
    // Backup
    pub backup_auto_enabled: bool,
    pub backup_interval_hours: i64,
    pub backup_retention: i64,
    pub backup_on_exit: bool,
    /// Extra folder (USB drive / second disk) that receives a copy of every automatic backup.
    pub backup_secondary_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            store_name: "CODE X MART".into(),
            store_tagline: String::new(),
            store_address: String::new(),
            store_phone: String::new(),
            store_email: String::new(),
            store_tax_id: String::new(),
            currency_symbol: "Rs".into(),
            currency_decimals: 2,
            receipt_header: String::new(),
            receipt_footer: "Thank you for shopping with us!".into(),
            receipt_paper: "80mm".into(),
            receipt_print_mode: "dialog".into(),
            receipt_printer: String::new(),
            receipt_copies: 1,
            auto_print_receipt: false,
            open_cash_drawer: false,
            label_size: "38x25".into(),
            invoice_prefix: "INV-".into(),
            allow_negative_stock: false,
            enable_credit_sales: true,
            enable_batch_tracking: true,
            enable_variants: true,
            enable_wholesale: false,
            max_cashier_discount_bp: 10_000,
            expiry_warning_days: 60,
            backup_auto_enabled: true,
            backup_interval_hours: 24,
            backup_retention: 30,
            backup_on_exit: true,
            backup_secondary_dir: String::new(),
        }
    }
}

/// Settings merged over defaults (missing keys fall back to defaults, so
/// new settings added by an upgrade work immediately).
pub fn load(conn: &Connection) -> AppResult<Settings> {
    let stored = repo::load_all(conn)?;
    let mut base = serde_json::to_value(Settings::default())?;
    if let Value::Object(map) = &mut base {
        for (k, val) in stored {
            if map.contains_key(&k) {
                map.insert(k, val);
            }
        }
    }
    Ok(serde_json::from_value(base).unwrap_or_default())
}

pub fn get(conn: &Connection, _ctx: &Ctx) -> AppResult<Settings> {
    load(conn)
}

fn validate(mut s: Settings) -> AppResult<Settings> {
    s.store_name = v::required("Store name", &s.store_name, 80)?;
    s.store_tagline = v::optional("Tagline", Some(&s.store_tagline), 120)?.unwrap_or_default();
    s.store_address = v::optional_multiline("Address", Some(&s.store_address), 300)?.unwrap_or_default();
    s.store_phone = v::optional("Phone", Some(&s.store_phone), 60)?.unwrap_or_default();
    s.store_email = v::email(Some(&s.store_email))?.unwrap_or_default();
    s.store_tax_id = v::optional("Tax ID", Some(&s.store_tax_id), 60)?.unwrap_or_default();
    s.currency_symbol = v::required("Currency symbol", &s.currency_symbol, 6)?;
    if s.currency_decimals != 0 && s.currency_decimals != 2 {
        return Err(AppError::validation("Currency decimals must be 0 or 2."));
    }
    s.receipt_header = v::optional_multiline("Receipt header", Some(&s.receipt_header), 300)?.unwrap_or_default();
    s.receipt_footer = v::optional_multiline("Receipt footer", Some(&s.receipt_footer), 300)?.unwrap_or_default();
    if !["80mm", "58mm", "a4"].contains(&s.receipt_paper.as_str()) {
        return Err(AppError::validation("Receipt paper must be 80mm, 58mm or A4."));
    }
    if !["dialog", "escpos"].contains(&s.receipt_print_mode.as_str()) {
        return Err(AppError::validation("Unknown print mode."));
    }
    if s.receipt_print_mode == "escpos" && s.receipt_printer.trim().is_empty() {
        return Err(AppError::validation("Choose a printer for direct thermal printing."));
    }
    s.receipt_copies = s.receipt_copies.clamp(1, 3);
    if !["38x25", "50x25", "a4-65"].contains(&s.label_size.as_str()) {
        s.label_size = "38x25".into();
    }
    let prefix = s.invoice_prefix.trim().to_uppercase();
    if prefix.len() > 10 || !prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/') {
        return Err(AppError::validation("Invoice prefix may have up to 10 letters, numbers, - or /."));
    }
    s.invoice_prefix = prefix;
    s.max_cashier_discount_bp = s.max_cashier_discount_bp.clamp(0, 10_000);
    s.expiry_warning_days = s.expiry_warning_days.clamp(1, 730);
    s.backup_interval_hours = s.backup_interval_hours.clamp(1, 168);
    s.backup_retention = s.backup_retention.clamp(3, 365);
    s.backup_secondary_dir = s.backup_secondary_dir.trim().to_string();
    Ok(s)
}

pub fn save(conn: &mut Connection, ctx: &Ctx, input: Settings) -> AppResult<Settings> {
    ctx.require(Permission::ManageSettings)?;
    let new = validate(input)?;
    let tx = begin(conn)?;
    let old = load(&tx)?;
    let map = match serde_json::to_value(&new)? {
        Value::Object(m) => m,
        _ => unreachable!(),
    };
    repo::save_all(&tx, &map, &ctx.ts())?;
    if sequences::prefix(&tx, "sale")? != new.invoice_prefix {
        sequences::set_prefix(&tx, "sale", &new.invoice_prefix)?;
    }
    let old_map = serde_json::to_value(&old)?;
    let changed: Vec<&String> = map.keys().filter(|k| old_map.get(k.as_str()) != map.get(k.as_str())).collect();
    if !changed.is_empty() {
        audit::record(
            &tx,
            ctx,
            "settings.updated",
            "settings",
            None,
            format!("Settings changed: {}", changed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
            Some(json!({ "changed": changed })),
        )?;
    }
    tx.commit()?;
    Ok(new)
}

/// Writes initial store details during first-run setup (no permission
/// check: there is no user yet).
pub(crate) fn save_initial(conn: &Connection, s: &Settings, now: &str) -> AppResult<()> {
    let s = validate(s.clone())?;
    if let Value::Object(m) = serde_json::to_value(&s)? {
        repo::save_all(conn, &m, now)?;
    }
    Ok(())
}
