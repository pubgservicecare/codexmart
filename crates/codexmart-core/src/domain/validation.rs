//! Input validation and normalisation shared by all services.

use chrono::NaiveDate;

use crate::error::{AppError, AppResult};
use crate::money::{Money, Qty, QTY_SCALE};

/// Trims and collapses internal whitespace.
pub fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn required(field: &str, value: &str, max: usize) -> AppResult<String> {
    let v = clean(value);
    if v.is_empty() {
        return Err(AppError::validation(format!("{field} is required.")));
    }
    if v.chars().count() > max {
        return Err(AppError::validation(format!("{field} must be at most {max} characters.")));
    }
    Ok(v)
}

pub fn optional(field: &str, value: Option<&str>, max: usize) -> AppResult<Option<String>> {
    match value.map(clean) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) if v.chars().count() > max => {
            Err(AppError::validation(format!("{field} must be at most {max} characters.")))
        }
        Some(v) => Ok(Some(v)),
    }
}

/// Like [`optional`] but keeps line breaks (addresses, notes).
pub fn optional_multiline(field: &str, value: Option<&str>, max: usize) -> AppResult<Option<String>> {
    match value.map(|v| v.trim().replace("\r\n", "\n")) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) if v.chars().count() > max => {
            Err(AppError::validation(format!("{field} must be at most {max} characters.")))
        }
        Some(v) => Ok(Some(v)),
    }
}

/// SKUs are stored upper-case; letters, digits and - _ . / only.
pub fn sku(value: &str) -> AppResult<String> {
    let v = value.trim().to_uppercase();
    if v.is_empty() {
        return Err(AppError::validation("SKU is required."));
    }
    if v.len() > 40 {
        return Err(AppError::validation("SKU must be at most 40 characters."));
    }
    if !v.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c)) {
        return Err(AppError::validation("SKU may only contain letters, numbers and - _ . /"));
    }
    Ok(v)
}

/// Barcodes: printable ASCII without spaces. Numeric codes with a standard
/// retail length (EAN-8, UPC-A, EAN-13, GTIN-14) must have a valid check digit.
pub fn barcode(value: &str) -> AppResult<String> {
    let v = value.trim().to_string();
    if v.len() < 3 || v.len() > 48 {
        return Err(AppError::validation("Barcode must be between 3 and 48 characters."));
    }
    if !v.chars().all(|c| c.is_ascii_graphic()) {
        return Err(AppError::validation("Barcode may not contain spaces or special characters."));
    }
    if v.chars().all(|c| c.is_ascii_digit()) && matches!(v.len(), 8 | 12 | 13 | 14) && !is_valid_gtin(&v) {
        return Err(AppError::validation(
            "This barcode's check digit is wrong. Re-scan it or check the number printed on the product.",
        ));
    }
    Ok(v)
}

/// GS1 check digit for the given body (all digits except the check digit).
pub fn gtin_check_digit(body: &str) -> Option<u32> {
    let mut sum = 0;
    for (i, ch) in body.chars().rev().enumerate() {
        let d = ch.to_digit(10)?;
        sum += if i % 2 == 0 { d * 3 } else { d };
    }
    Some((10 - sum % 10) % 10)
}

pub fn is_valid_gtin(code: &str) -> bool {
    if !matches!(code.len(), 8 | 12 | 13 | 14) || !code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let (body, check) = code.split_at(code.len() - 1);
    gtin_check_digit(body) == check.chars().next().and_then(|c| c.to_digit(10))
}

/// In-store EAN-13 ("20" prefix is reserved by GS1 for internal use).
pub fn internal_ean13(seq: i64) -> String {
    let body = format!("20{:010}", seq.rem_euclid(10_000_000_000));
    let check = gtin_check_digit(&body).unwrap_or(0);
    format!("{body}{check}")
}

pub fn date(field: &str, value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::validation(format!("{field} must be a valid date (YYYY-MM-DD).")))
}

pub fn optional_date(field: &str, value: Option<&str>) -> AppResult<Option<String>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => Ok(Some(date(field, v)?.format("%Y-%m-%d").to_string())),
    }
}

/// A transaction date that is not in the future (back-dating is allowed).
pub fn business_date(field: &str, value: Option<&str>, today: NaiveDate) -> AppResult<String> {
    let d = match value.map(str::trim) {
        None | Some("") => today,
        Some(v) => date(field, v)?,
    };
    if d > today {
        return Err(AppError::validation(format!("{field} cannot be in the future.")));
    }
    Ok(d.format("%Y-%m-%d").to_string())
}

pub fn quantity(what: &str, qty: Qty, allow_decimal: bool) -> AppResult<()> {
    if qty <= 0 {
        return Err(AppError::validation(format!("{what}: quantity must be more than zero.")));
    }
    if !allow_decimal && qty % QTY_SCALE != 0 {
        return Err(AppError::validation(format!("{what}: this unit is counted in whole numbers.")));
    }
    if qty > 1_000_000_000_000 {
        return Err(AppError::validation(format!("{what}: quantity is too large.")));
    }
    Ok(())
}

pub fn non_negative(field: &str, amount: Money) -> AppResult<()> {
    if amount < 0 {
        return Err(AppError::validation(format!("{field} cannot be negative.")));
    }
    if amount > 100_000_000_000_000 {
        return Err(AppError::validation(format!("{field} is too large.")));
    }
    Ok(())
}

pub fn positive(field: &str, amount: Money) -> AppResult<()> {
    non_negative(field, amount)?;
    if amount == 0 {
        return Err(AppError::validation(format!("{field} must be more than zero.")));
    }
    Ok(())
}

pub fn email(value: Option<&str>) -> AppResult<Option<String>> {
    let v = optional("Email", value, 120)?;
    if let Some(e) = &v {
        let ok = e.contains('@') && !e.contains(' ') && e.split('@').nth(1).is_some_and(|d| d.contains('.'));
        if !ok {
            return Err(AppError::validation("Email address is not valid."));
        }
    }
    Ok(v)
}

/// Every write that must not be repeated carries a client transaction id.
pub fn client_txn_id(value: &str) -> AppResult<String> {
    let v = value.trim();
    if v.len() < 8 || v.len() > 64 || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::validation("Invalid transaction reference. Please try again."));
    }
    Ok(v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtin_check_digits() {
        assert!(is_valid_gtin("4006381333931"));
        assert!(!is_valid_gtin("4006381333932"));
        assert!(is_valid_gtin("036000291452")); // UPC-A
        assert!(is_valid_gtin("96385074")); // EAN-8
        let code = internal_ean13(42);
        assert_eq!(code.len(), 13);
        assert!(code.starts_with("20"));
        assert!(is_valid_gtin(&code));
    }

    #[test]
    fn barcode_rules() {
        assert!(barcode("4006381333931").is_ok());
        assert!(barcode("4006381333932").is_err());
        assert!(barcode("ABC-123").is_ok());
        assert!(barcode("AB C").is_err());
        assert!(barcode("12").is_err());
    }

    #[test]
    fn sku_rules() {
        assert_eq!(sku(" ab-12 ").unwrap(), "AB-12");
        assert!(sku("").is_err());
        assert!(sku("A B").is_err());
        assert!(sku(&"X".repeat(41)).is_err());
    }

    #[test]
    fn quantities_respect_units() {
        assert!(quantity("Line", 2_000, false).is_ok());
        assert!(quantity("Line", 2_500, false).is_err());
        assert!(quantity("Line", 2_500, true).is_ok());
        assert!(quantity("Line", 0, true).is_err());
    }

    #[test]
    fn text_cleaning() {
        assert_eq!(required("Name", "  Rose   Cream ", 50).unwrap(), "Rose Cream");
        assert!(required("Name", "   ", 50).is_err());
        assert_eq!(optional("Note", Some("  "), 10).unwrap(), None);
        assert!(email(Some("shop@example.com")).is_ok());
        assert!(email(Some("not-an-email")).is_err());
    }

    #[test]
    fn dates() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(business_date("Date", None, today).unwrap(), "2026-11-01");
        assert!(business_date("Date", Some("2026-11-02"), today).is_err());
        assert!(business_date("Date", Some("2026-10-31"), today).is_ok());
        assert!(date("Date", "31/10/2026").is_err());
    }
}
