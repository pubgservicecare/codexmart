//! Simple named lookups (categories, brands, expense categories) and units.

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LookupKind {
    Category,
    Brand,
    ExpenseCategory,
}

impl LookupKind {
    fn table(self) -> &'static str {
        match self {
            LookupKind::Category => "categories",
            LookupKind::Brand => "brands",
            LookupKind::ExpenseCategory => "expense_categories",
        }
    }

    fn usage_sql(self) -> &'static str {
        match self {
            LookupKind::Category => "(SELECT COUNT(*) FROM products x WHERE x.category_id = t.id)",
            LookupKind::Brand => "(SELECT COUNT(*) FROM products x WHERE x.brand_id = t.id)",
            LookupKind::ExpenseCategory => "(SELECT COUNT(*) FROM expenses x WHERE x.category_id = t.id)",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LookupKind::Category => "Category",
            LookupKind::Brand => "Brand",
            LookupKind::ExpenseCategory => "Expense category",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupItem {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn map_lookup(r: &Row) -> rusqlite::Result<LookupItem> {
    Ok(LookupItem {
        id: r.get(0)?,
        name: r.get(1)?,
        is_active: r.get(2)?,
        usage_count: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

fn lookup_select(kind: LookupKind) -> String {
    format!(
        "SELECT t.id, t.name, t.is_active, {}, t.created_at, t.updated_at FROM {} t",
        kind.usage_sql(),
        kind.table()
    )
}

pub fn list(conn: &Connection, kind: LookupKind, include_inactive: bool) -> AppResult<Vec<LookupItem>> {
    let filter = if include_inactive { "" } else { " WHERE t.is_active = 1" };
    let sql = format!("{}{filter} ORDER BY t.name COLLATE NOCASE", lookup_select(kind));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_lookup)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, kind: LookupKind, id: &str) -> AppResult<LookupItem> {
    let sql = format!("{} WHERE t.id = ?1", lookup_select(kind));
    conn.query_row(&sql, [id], map_lookup).map_err(|_| AppError::not_found(kind.label()))
}

pub fn is_active(conn: &Connection, kind: LookupKind, id: &str) -> AppResult<bool> {
    let sql = format!("SELECT is_active FROM {} WHERE id = ?1", kind.table());
    conn.query_row(&sql, [id], |r| r.get(0)).map_err(|_| AppError::not_found(kind.label()))
}

pub fn insert(conn: &Connection, kind: LookupKind, id: &str, name: &str, now: &str) -> AppResult<()> {
    let sql = format!(
        "INSERT INTO {} (id, name, is_active, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)",
        kind.table()
    );
    conn.execute(&sql, params![id, name, now])?;
    Ok(())
}

pub fn update(conn: &Connection, kind: LookupKind, id: &str, name: &str, active: bool, now: &str) -> AppResult<()> {
    let sql = format!("UPDATE {} SET name = ?2, is_active = ?3, updated_at = ?4 WHERE id = ?1", kind.table());
    if conn.execute(&sql, params![id, name, active, now])? == 0 {
        return Err(AppError::not_found(kind.label()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub allow_decimal: bool,
    pub is_active: bool,
    pub usage_count: i64,
}

const UNIT_SELECT: &str = "SELECT u.id, u.name, u.short_name, u.allow_decimal, u.is_active, \
    (SELECT COUNT(*) FROM products p WHERE p.unit_id = u.id) FROM units u";

fn map_unit(r: &Row) -> rusqlite::Result<Unit> {
    Ok(Unit {
        id: r.get(0)?,
        name: r.get(1)?,
        short_name: r.get(2)?,
        allow_decimal: r.get(3)?,
        is_active: r.get(4)?,
        usage_count: r.get(5)?,
    })
}

pub fn list_units(conn: &Connection, include_inactive: bool) -> AppResult<Vec<Unit>> {
    let filter = if include_inactive { "" } else { " WHERE u.is_active = 1" };
    let mut stmt = conn.prepare(&format!("{UNIT_SELECT}{filter} ORDER BY u.name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_unit)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_unit(conn: &Connection, id: &str) -> AppResult<Unit> {
    conn.query_row(&format!("{UNIT_SELECT} WHERE u.id = ?1"), [id], map_unit).map_err(|_| AppError::not_found("Unit"))
}

pub fn insert_unit(conn: &Connection, id: &str, name: &str, short: &str, decimal: bool, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO units (id, name, short_name, allow_decimal, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
        params![id, name, short, decimal, now],
    )?;
    Ok(())
}

pub fn update_unit(
    conn: &Connection,
    id: &str,
    name: &str,
    short: &str,
    decimal: bool,
    active: bool,
    now: &str,
) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE units SET name = ?2, short_name = ?3, allow_decimal = ?4, is_active = ?5, updated_at = ?6 WHERE id = ?1",
        params![id, name, short, decimal, active, now],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Unit"));
    }
    Ok(())
}
