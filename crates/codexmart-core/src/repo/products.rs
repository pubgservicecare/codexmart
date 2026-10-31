use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::money::{Money, Qty};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: String,
    pub name: String,
    /// Name with variant attributes, e.g. "Matte Lipstick — Red / M".
    pub display_name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub brand_id: Option<String>,
    pub brand_name: Option<String>,
    pub unit_id: String,
    pub unit_name: String,
    pub unit_short: String,
    pub allow_decimal: bool,
    pub variant_group_id: Option<String>,
    pub size: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub sale_price: Money,
    pub wholesale_price: Option<Money>,
    /// Hidden (None) for roles that may not see costs.
    pub cost_price: Option<Money>,
    pub stock_qty: Qty,
    pub stock_value: Option<Money>,
    pub reorder_level: Qty,
    pub track_batches: bool,
    pub image_ref: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Product {
    pub fn redact_costs(&mut self) {
        self.cost_price = None;
        self.stock_value = None;
    }
}

pub fn display_name(name: &str, size: Option<&str>, color: Option<&str>) -> String {
    let attrs: Vec<&str> = [color, size].into_iter().flatten().filter(|s| !s.is_empty()).collect();
    if attrs.is_empty() {
        name.to_string()
    } else {
        format!("{name} — {}", attrs.join(" / "))
    }
}

const COLUMNS: &str = "p.id, p.name, p.sku, p.barcode, p.category_id, c.name, p.brand_id, b.name, \
    p.unit_id, u.name, u.short_name, u.allow_decimal, p.variant_group_id, p.size, p.color, \
    p.description, p.sale_price, p.wholesale_price, p.cost_price, p.stock_qty, p.stock_value, \
    p.reorder_level, p.track_batches, p.image_ref, p.is_active, p.created_at, p.updated_at";

const FROM: &str = " FROM products p LEFT JOIN categories c ON c.id = p.category_id \
    LEFT JOIN brands b ON b.id = p.brand_id JOIN units u ON u.id = p.unit_id";

fn map(r: &Row) -> rusqlite::Result<Product> {
    let name: String = r.get(1)?;
    let size: Option<String> = r.get(13)?;
    let color: Option<String> = r.get(14)?;
    Ok(Product {
        display_name: display_name(&name, size.as_deref(), color.as_deref()),
        id: r.get(0)?,
        name,
        sku: r.get(2)?,
        barcode: r.get(3)?,
        category_id: r.get(4)?,
        category_name: r.get(5)?,
        brand_id: r.get(6)?,
        brand_name: r.get(7)?,
        unit_id: r.get(8)?,
        unit_name: r.get(9)?,
        unit_short: r.get(10)?,
        allow_decimal: r.get(11)?,
        variant_group_id: r.get(12)?,
        size,
        color,
        description: r.get(15)?,
        sale_price: r.get(16)?,
        wholesale_price: r.get(17)?,
        cost_price: Some(r.get(18)?),
        stock_qty: r.get(19)?,
        stock_value: Some(r.get(20)?),
        reorder_level: r.get(21)?,
        track_batches: r.get(22)?,
        image_ref: r.get(23)?,
        is_active: r.get(24)?,
        created_at: r.get(25)?,
        updated_at: r.get(26)?,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProductFilter {
    pub search: Option<String>,
    pub category_id: Option<String>,
    pub brand_id: Option<String>,
    /// "active" (default), "inactive" or "all".
    pub status: Option<String>,
    /// "low", "out", "attention" (low or out) or "in".
    pub stock: Option<String>,
    pub variant_group_id: Option<String>,
    /// "name" (default), "stock", "updated", "sku", "price".
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const SEARCH_CLAUSE: &str = "(p.name LIKE ? ESCAPE '\\' OR p.sku LIKE ? ESCAPE '\\' \
    OR IFNULL(p.barcode,'') LIKE ? ESCAPE '\\' OR IFNULL(b.name,'') LIKE ? ESCAPE '\\' \
    OR IFNULL(c.name,'') LIKE ? ESCAPE '\\' OR IFNULL(p.size,'') LIKE ? ESCAPE '\\' \
    OR IFNULL(p.color,'') LIKE ? ESCAPE '\\')";

fn add_search(w: &mut Where, search: &str) {
    for term in search.split_whitespace().take(6) {
        w.repeat(SEARCH_CLAUSE, &like(term), 7);
    }
}

pub fn list(conn: &Connection, f: &ProductFilter) -> AppResult<Page<Product>> {
    let mut w = Where::new();
    match f.status.as_deref() {
        Some("inactive") => {
            w.raw("p.is_active = 0");
        }
        Some("all") => {}
        _ => {
            w.raw("p.is_active = 1");
        }
    }
    w.opt("p.category_id = ?", &f.category_id);
    w.opt("p.brand_id = ?", &f.brand_id);
    w.opt("p.variant_group_id = ?", &f.variant_group_id);
    match f.stock.as_deref() {
        Some("low") => {
            w.raw("p.stock_qty > 0 AND p.reorder_level > 0 AND p.stock_qty <= p.reorder_level");
        }
        Some("out") => {
            w.raw("p.stock_qty <= 0");
        }
        Some("attention") => {
            w.raw("(p.stock_qty <= 0 OR (p.reorder_level > 0 AND p.stock_qty <= p.reorder_level))");
        }
        Some("in") => {
            w.raw("p.stock_qty > 0");
        }
        _ => {}
    }
    if let Some(s) = &f.search {
        add_search(&mut w, s);
    }
    let order = match f.sort.as_deref() {
        Some("stock") => "p.stock_qty ASC, p.name COLLATE NOCASE",
        Some("updated") => "p.updated_at DESC",
        Some("sku") => "p.sku COLLATE NOCASE",
        Some("price") => "p.sale_price DESC",
        _ => "p.name COLLATE NOCASE, p.size, p.color",
    };
    let where_sql = w.sql();
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*){FROM}{where_sql}"), params_from_iter(w.args.iter()), |r| r.get(0))?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 200);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let sql = format!("SELECT {COLUMNS}{FROM}{where_sql} ORDER BY {order} LIMIT ? OFFSET ?");
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt.query_map(params_from_iter(args.iter()), map)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

/// Billing search: active products only; exact barcode/SKU matches first.
pub fn search(conn: &Connection, query: &str, limit: i64) -> AppResult<Vec<Product>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut w = Where::new();
    w.raw("p.is_active = 1");
    add_search(&mut w, q);
    let mut args = w.args.clone();
    args.push(Value::Text(q.to_string()));
    args.push(Value::Text(q.to_string()));
    args.push(Value::Text(format!("{}%", q.replace('%', ""))));
    args.push(Value::Integer(limit.clamp(1, 100)));
    let sql = format!(
        "SELECT {COLUMNS}{FROM}{} ORDER BY CASE WHEN p.barcode = ? OR p.sku = ? COLLATE NOCASE THEN 0 \
         WHEN p.name LIKE ? THEN 1 ELSE 2 END, p.name COLLATE NOCASE LIMIT ?",
        w.sql()
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt.query_map(params_from_iter(args.iter()), map)?.collect::<Result<_, _>>()?;
    Ok(items)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Product> {
    conn.query_row(&format!("SELECT {COLUMNS}{FROM} WHERE p.id = ?1"), [id], map)
        .optional()?
        .ok_or_else(|| AppError::not_found("Product"))
}

/// Exact barcode or SKU match (what a scanner sends).
pub fn find_by_code(conn: &Connection, code: &str) -> AppResult<Option<Product>> {
    let code = code.trim();
    if code.is_empty() {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS}{FROM} WHERE p.barcode = ?1 OR p.sku = ?1 COLLATE NOCASE \
                 ORDER BY p.is_active DESC, CASE WHEN p.barcode = ?1 THEN 0 ELSE 1 END LIMIT 1"
            ),
            [code],
            map,
        )
        .optional()?)
}

pub fn sku_taken(conn: &Connection, sku: &str, except_id: Option<&str>) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM products WHERE sku = ?1 COLLATE NOCASE AND id <> IFNULL(?2, '')",
        params![sku, except_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn barcode_taken(conn: &Connection, barcode: &str, except_id: Option<&str>) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM products WHERE barcode = ?1 AND id <> IFNULL(?2, '')",
        params![barcode, except_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub struct ProductRow<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub sku: &'a str,
    pub barcode: Option<&'a str>,
    pub category_id: Option<&'a str>,
    pub brand_id: Option<&'a str>,
    pub unit_id: &'a str,
    pub variant_group_id: Option<&'a str>,
    pub size: Option<&'a str>,
    pub color: Option<&'a str>,
    pub description: Option<&'a str>,
    pub sale_price: Money,
    pub wholesale_price: Option<Money>,
    pub cost_price: Money,
    pub reorder_level: Qty,
    pub track_batches: bool,
    pub image_ref: Option<&'a str>,
    pub is_active: bool,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, p: &ProductRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO products (id, name, sku, barcode, category_id, brand_id, unit_id, variant_group_id,
            size, color, description, sale_price, wholesale_price, cost_price, stock_qty, stock_value,
            reorder_level, track_batches, image_ref, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0, 0, ?15, ?16, ?17, ?18, ?19, ?19)",
        params![
            p.id,
            p.name,
            p.sku,
            p.barcode,
            p.category_id,
            p.brand_id,
            p.unit_id,
            p.variant_group_id,
            p.size,
            p.color,
            p.description,
            p.sale_price,
            p.wholesale_price,
            p.cost_price,
            p.reorder_level,
            p.track_batches,
            p.image_ref,
            p.is_active,
            p.now
        ],
    )?;
    Ok(())
}

/// Updates master data. Stock quantity and value are never touched here.
pub fn update(conn: &Connection, p: &ProductRow) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE products SET name = ?2, sku = ?3, barcode = ?4, category_id = ?5, brand_id = ?6,
            unit_id = ?7, variant_group_id = ?8, size = ?9, color = ?10, description = ?11,
            sale_price = ?12, wholesale_price = ?13, cost_price = ?14, reorder_level = ?15,
            track_batches = ?16, image_ref = ?17, is_active = ?18, updated_at = ?19
         WHERE id = ?1",
        params![
            p.id,
            p.name,
            p.sku,
            p.barcode,
            p.category_id,
            p.brand_id,
            p.unit_id,
            p.variant_group_id,
            p.size,
            p.color,
            p.description,
            p.sale_price,
            p.wholesale_price,
            p.cost_price,
            p.reorder_level,
            p.track_batches,
            p.image_ref,
            p.is_active,
            p.now
        ],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Product"));
    }
    Ok(())
}

pub fn set_active(conn: &Connection, id: &str, active: bool, now: &str) -> AppResult<()> {
    if conn.execute("UPDATE products SET is_active = ?2, updated_at = ?3 WHERE id = ?1", params![id, active, now])? == 0
    {
        return Err(AppError::not_found("Product"));
    }
    Ok(())
}

pub fn set_sale_price(conn: &Connection, id: &str, price: Money, now: &str) -> AppResult<()> {
    conn.execute("UPDATE products SET sale_price = ?2, updated_at = ?3 WHERE id = ?1", params![id, price, now])?;
    Ok(())
}

pub fn set_variant_group(conn: &Connection, id: &str, group: &str, now: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE products SET variant_group_id = ?2, updated_at = ?3 WHERE id = ?1 AND variant_group_id IS NULL",
        params![id, group, now],
    )?;
    Ok(())
}

/// Current stock position: (qty, value, unit cost, batch tracked).
pub fn stock_state(conn: &Connection, id: &str) -> AppResult<(Qty, Money, Money, bool)> {
    conn.query_row("SELECT stock_qty, stock_value, cost_price, track_batches FROM products WHERE id = ?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .optional()?
    .ok_or_else(|| AppError::not_found("Product"))
}

/// Only called by the stock engine together with a stock movement.
pub(crate) fn write_stock(
    conn: &Connection,
    id: &str,
    qty: Qty,
    value: Money,
    cost: Money,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE products SET stock_qty = ?2, stock_value = ?3, cost_price = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, qty, value, cost, now],
    )?;
    Ok(())
}
