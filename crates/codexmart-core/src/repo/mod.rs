//! Data access. All SQL lives in this module tree; services never build SQL
//! themselves. Functions take `&Connection` so they work equally on a plain
//! connection or inside a transaction (which derefs to a connection).
//! All values are bound as parameters, never interpolated.

pub mod audit;
pub mod catalog;
pub mod customers;
pub mod expenses;
pub mod products;
pub mod purchases;
pub mod sales;
pub mod sequences;
pub mod settings;
pub mod stock;
pub mod suppliers;
pub mod users;

use rusqlite::types::Value;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
}

/// `%term%` with LIKE wildcards escaped (use with `ESCAPE '\'`).
pub(crate) fn like(term: &str) -> String {
    format!("%{}%", term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

/// Builder for optional WHERE clauses with bound parameters.
#[derive(Default)]
pub(crate) struct Where {
    clauses: Vec<String>,
    pub args: Vec<Value>,
}

impl Where {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raw(&mut self, clause: &str) -> &mut Self {
        self.clauses.push(clause.to_string());
        self
    }

    pub fn eq(&mut self, clause: &str, value: impl Into<Value>) -> &mut Self {
        self.clauses.push(clause.to_string());
        self.args.push(value.into());
        self
    }

    /// Adds `clause` with `n` copies of the same value (for OR'ed LIKEs).
    pub fn repeat(&mut self, clause: &str, value: &str, n: usize) -> &mut Self {
        self.clauses.push(clause.to_string());
        for _ in 0..n {
            self.args.push(Value::Text(value.to_string()));
        }
        self
    }

    pub fn opt(&mut self, clause: &str, value: &Option<String>) -> &mut Self {
        if let Some(v) = value.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty()) {
            self.eq(clause, v.to_string());
        }
        self
    }

    pub fn sql(&self) -> String {
        if self.clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", self.clauses.join(" AND "))
        }
    }
}

pub(crate) fn limit_offset(limit: Option<i64>, offset: Option<i64>, default: i64) -> (i64, i64) {
    (limit.unwrap_or(default).clamp(1, 1000), offset.unwrap_or(0).max(0))
}
