use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Every error a service can return. Messages are written for shop staff;
/// `code` lets the UI react programmatically (e.g. return to login).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Your session has ended. Please sign in again.")]
    Unauthenticated,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    InsufficientStock(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("File error: {0}")]
    Io(String),
    #[error("{0}")]
    Backup(String),
    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn not_found(what: &str) -> Self {
        Self::NotFound(format!("{what} was not found."))
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "VALIDATION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::Unauthenticated => "UNAUTHENTICATED",
            AppError::Forbidden(_) => "FORBIDDEN",
            AppError::InsufficientStock(_) => "INSUFFICIENT_STOCK",
            AppError::Database(_) => "DATABASE",
            AppError::Io(_) => "IO",
            AppError::Backup(_) => "BACKUP",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("The requested record was not found.".into()),
            rusqlite::Error::SqliteFailure(err, msg) if err.code == rusqlite::ErrorCode::ConstraintViolation => {
                AppError::Conflict(friendly_constraint(msg.as_deref().unwrap_or_default()))
            }
            _ => AppError::Database(e.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("Data format error: {e}"))
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(e: zip::result::ZipError) -> Self {
        AppError::Backup(format!("The backup file could not be read: {e}"))
    }
}

/// Turns SQLite constraint messages into sentences staff can act on.
fn friendly_constraint(msg: &str) -> String {
    const KNOWN: &[(&str, &str)] = &[
        ("products.sku", "Another product already uses this SKU."),
        ("products.barcode", "Another product already uses this barcode."),
        ("categories.name", "A category with this name already exists."),
        ("brands.name", "A brand with this name already exists."),
        ("units.name", "A unit with this name already exists."),
        ("expense_categories.name", "An expense category with this name already exists."),
        ("users.username", "This username is already taken."),
        (
            "purchases.supplier_id, purchases.supplier_invoice_no",
            "This supplier invoice number has already been recorded for this supplier.",
        ),
        ("cash_closings.business_date", "The cash for this day has already been closed."),
    ];
    for (needle, friendly) in KNOWN {
        if msg.contains(needle) {
            return (*friendly).to_string();
        }
    }
    if msg.starts_with("FOREIGN KEY") {
        return "This record is linked to other records and cannot be changed this way.".into();
    }
    if msg.is_empty() {
        "The change conflicts with existing data.".into()
    } else {
        msg.to_string()
    }
}
