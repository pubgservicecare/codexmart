//! Products, categories, brands and units.

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use super::stock::{self, BatchTarget, Refs};
use super::{audit, begin};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{self, format_amount, Money, Qty, QTY_SCALE};
use crate::repo::catalog::{self as lookups, LookupItem, LookupKind, Unit};
use crate::repo::products::{self as repo, Product, ProductFilter, ProductRow};
use crate::repo::{sequences, Page};

fn lookup_permission(kind: LookupKind) -> Permission {
    match kind {
        LookupKind::ExpenseCategory => Permission::ManageExpenses,
        _ => Permission::ManageCatalog,
    }
}

// --------------------------------------------------------------- lookups

pub fn list_lookups(
    conn: &Connection,
    _ctx: &Ctx,
    kind: LookupKind,
    include_inactive: bool,
) -> AppResult<Vec<LookupItem>> {
    lookups::list(conn, kind, include_inactive)
}

pub fn create_lookup(conn: &mut Connection, ctx: &Ctx, kind: LookupKind, name: &str) -> AppResult<LookupItem> {
    ctx.require(lookup_permission(kind))?;
    let name = v::required("Name", name, 60)?;
    let tx = begin(conn)?;
    let id = new_id();
    lookups::insert(&tx, kind, &id, &name, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "lookup.created",
        kind.label(),
        Some(&id),
        format!("{} '{name}' created", kind.label()),
        None,
    )?;
    tx.commit()?;
    lookups::get(conn, kind, &id)
}

pub fn update_lookup(
    conn: &mut Connection,
    ctx: &Ctx,
    kind: LookupKind,
    id: &str,
    name: &str,
    is_active: bool,
) -> AppResult<LookupItem> {
    ctx.require(lookup_permission(kind))?;
    let name = v::required("Name", name, 60)?;
    let tx = begin(conn)?;
    let old = lookups::get(&tx, kind, id)?;
    lookups::update(&tx, kind, id, &name, is_active, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "lookup.updated",
        kind.label(),
        Some(id),
        format!("{} '{}' updated", kind.label(), old.name),
        Some(json!({ "name": name, "active": is_active })),
    )?;
    tx.commit()?;
    lookups::get(conn, kind, id)
}

// ----------------------------------------------------------------- units

pub fn list_units(conn: &Connection, _ctx: &Ctx, include_inactive: bool) -> AppResult<Vec<Unit>> {
    lookups::list_units(conn, include_inactive)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitInput {
    pub name: String,
    pub short_name: String,
    pub allow_decimal: bool,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn yes() -> bool {
    true
}

pub fn create_unit(conn: &mut Connection, ctx: &Ctx, input: UnitInput) -> AppResult<Unit> {
    ctx.require(Permission::ManageCatalog)?;
    let name = v::required("Unit name", &input.name, 40)?;
    let short = v::required("Short name", &input.short_name, 10)?;
    let tx = begin(conn)?;
    let id = new_id();
    lookups::insert_unit(&tx, &id, &name, &short, input.allow_decimal, &ctx.ts())?;
    audit::record(&tx, ctx, "unit.created", "unit", Some(&id), format!("Unit '{name}' created"), None)?;
    tx.commit()?;
    lookups::get_unit(conn, &id)
}

pub fn update_unit(conn: &mut Connection, ctx: &Ctx, id: &str, input: UnitInput) -> AppResult<Unit> {
    ctx.require(Permission::ManageCatalog)?;
    let name = v::required("Unit name", &input.name, 40)?;
    let short = v::required("Short name", &input.short_name, 10)?;
    let tx = begin(conn)?;
    let old = lookups::get_unit(&tx, id)?;
    if old.allow_decimal && !input.allow_decimal {
        let fractional: i64 =
            tx.query_row("SELECT COUNT(*) FROM products WHERE unit_id = ?1 AND (stock_qty % 1000) <> 0", [id], |r| {
                r.get(0)
            })?;
        if fractional > 0 {
            return Err(AppError::validation(
                "Some products in this unit have fractional stock, so it must keep allowing decimals.",
            ));
        }
    }
    lookups::update_unit(&tx, id, &name, &short, input.allow_decimal, input.is_active, &ctx.ts())?;
    audit::record(&tx, ctx, "unit.updated", "unit", Some(id), format!("Unit '{}' updated", old.name), None)?;
    tx.commit()?;
    lookups::get_unit(conn, id)
}

// -------------------------------------------------------------- products

fn redact(ctx: &Ctx, mut p: Product) -> Product {
    if !ctx.can(Permission::ViewCosts) {
        p.redact_costs();
    }
    p
}

pub fn list_products(conn: &Connection, ctx: &Ctx, filter: &ProductFilter) -> AppResult<Page<Product>> {
    ctx.require(Permission::ViewCatalog)?;
    let page = repo::list(conn, filter)?;
    Ok(Page { total: page.total, items: page.items.into_iter().map(|p| redact(ctx, p)).collect() })
}

pub fn get_product(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Product> {
    ctx.require(Permission::ViewCatalog)?;
    Ok(redact(ctx, repo::get(conn, id)?))
}

pub fn search_products(conn: &Connection, ctx: &Ctx, query: &str, limit: i64) -> AppResult<Vec<Product>> {
    ctx.require(Permission::ViewCatalog)?;
    Ok(repo::search(conn, query, limit)?.into_iter().map(|p| redact(ctx, p)).collect())
}

pub fn find_by_code(conn: &Connection, ctx: &Ctx, code: &str) -> AppResult<Option<Product>> {
    ctx.require(Permission::ViewCatalog)?;
    Ok(repo::find_by_code(conn, code)?.map(|p| redact(ctx, p)))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningStock {
    pub qty: Qty,
    #[serde(default)]
    pub unit_cost: Option<Money>,
    #[serde(default)]
    pub batch_no: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductInput {
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub brand_id: Option<String>,
    pub unit_id: String,
    #[serde(default)]
    pub variant_group_id: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub sale_price: Money,
    #[serde(default)]
    pub wholesale_price: Option<Money>,
    #[serde(default)]
    pub cost_price: Option<Money>,
    #[serde(default)]
    pub reorder_level: Qty,
    #[serde(default)]
    pub track_batches: bool,
    #[serde(default)]
    pub image_ref: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub opening_stock: Option<OpeningStock>,
}

struct Clean {
    name: String,
    sku: Option<String>,
    barcode: Option<String>,
    category_id: Option<String>,
    brand_id: Option<String>,
    variant_group_id: Option<String>,
    size: Option<String>,
    color: Option<String>,
    description: Option<String>,
    image_ref: Option<String>,
    allow_decimal: bool,
}

fn opt_id(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn clean_input(conn: &Connection, input: &ProductInput, existing_id: Option<&str>) -> AppResult<Clean> {
    let name = v::required("Product name", &input.name, 150)?;
    let sku = match input.sku.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let s = v::sku(s)?;
            if repo::sku_taken(conn, &s, existing_id)? {
                return Err(AppError::Conflict(format!("SKU {s} is already used by another product.")));
            }
            Some(s)
        }
        None => None,
    };
    let barcode = match input.barcode.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(b) => {
            let b = v::barcode(b)?;
            if repo::barcode_taken(conn, &b, existing_id)? {
                return Err(AppError::Conflict(format!("Barcode {b} is already used by another product.")));
            }
            Some(b)
        }
        None => None,
    };
    let category_id = opt_id(&input.category_id);
    if let Some(c) = &category_id {
        lookups::get(conn, LookupKind::Category, c)?;
    }
    let brand_id = opt_id(&input.brand_id);
    if let Some(b) = &brand_id {
        lookups::get(conn, LookupKind::Brand, b)?;
    }
    let unit = lookups::get_unit(conn, input.unit_id.trim())?;
    v::non_negative("Sale price", input.sale_price)?;
    if let Some(w) = input.wholesale_price {
        v::non_negative("Wholesale price", w)?;
    }
    if let Some(c) = input.cost_price {
        v::non_negative("Cost price", c)?;
    }
    if input.reorder_level < 0 {
        return Err(AppError::validation("Reorder level cannot be negative."));
    }
    if !unit.allow_decimal && input.reorder_level % QTY_SCALE != 0 {
        return Err(AppError::validation("Reorder level must be a whole number for this unit."));
    }
    Ok(Clean {
        name,
        sku,
        barcode,
        category_id,
        brand_id,
        variant_group_id: opt_id(&input.variant_group_id),
        size: v::optional("Size", input.size.as_deref(), 40)?,
        color: v::optional("Colour / shade", input.color.as_deref(), 40)?,
        description: v::optional_multiline("Description", input.description.as_deref(), 1000)?,
        image_ref: v::optional("Image", input.image_ref.as_deref(), 500)?,
        allow_decimal: unit.allow_decimal,
    })
}

fn generate_sku(conn: &Connection) -> AppResult<String> {
    for _ in 0..1000 {
        let candidate = sequences::next(conn, "sku")?;
        if !repo::sku_taken(conn, &candidate, None)? {
            return Ok(candidate);
        }
    }
    Err(AppError::Internal("Could not generate a unique SKU.".into()))
}

pub fn create_product(conn: &mut Connection, ctx: &Ctx, input: ProductInput) -> AppResult<Product> {
    ctx.require(Permission::ManageCatalog)?;
    let tx = begin(conn)?;
    let c = clean_input(&tx, &input, None)?;
    let id = new_id();
    let sku = match c.sku {
        Some(s) => s,
        None => generate_sku(&tx)?,
    };
    let opening = input.opening_stock.as_ref().filter(|o| o.qty != 0);
    let opening_cost = opening.and_then(|o| o.unit_cost).or(input.cost_price).unwrap_or(0);
    v::non_negative("Opening stock cost", opening_cost)?;
    repo::insert(
        &tx,
        &ProductRow {
            id: &id,
            name: &c.name,
            sku: &sku,
            barcode: c.barcode.as_deref(),
            category_id: c.category_id.as_deref(),
            brand_id: c.brand_id.as_deref(),
            unit_id: input.unit_id.trim(),
            variant_group_id: c.variant_group_id.as_deref(),
            size: c.size.as_deref(),
            color: c.color.as_deref(),
            description: c.description.as_deref(),
            sale_price: input.sale_price,
            wholesale_price: input.wholesale_price,
            cost_price: opening_cost,
            reorder_level: input.reorder_level,
            track_batches: input.track_batches,
            image_ref: c.image_ref.as_deref(),
            is_active: input.is_active.unwrap_or(true),
            now: &ctx.ts(),
        },
    )?;
    if let Some(group) = &c.variant_group_id {
        // Starting a variant group from an existing product links that product too.
        repo::set_variant_group(&tx, group, group, &ctx.ts())?;
    }
    if let Some(o) = opening {
        v::quantity("Opening stock", o.qty, c.allow_decimal)?;
        let expiry = v::optional_date("Expiry date", o.expiry_date.as_deref())?;
        let batch_no = v::optional("Batch number", o.batch_no.as_deref(), 40)?;
        stock::receive(
            &tx,
            ctx,
            &id,
            o.qty,
            money::mul_qty(opening_cost, o.qty),
            "opening",
            &Refs {
                ref_type: "product",
                ref_id: &id,
                ref_no: &sku,
                ref_line_id: None,
                reason: Some("Opening stock"),
                note: None,
            },
            BatchTarget::New {
                batch_no: batch_no.as_deref(),
                expiry_date: expiry.as_deref(),
                source_type: "opening",
                source_id: &id,
            },
        )?;
    }
    audit::record(
        &tx,
        ctx,
        "product.created",
        "product",
        Some(&id),
        format!("Product '{}' ({sku}) created", c.name),
        Some(json!({ "salePrice": input.sale_price, "openingQty": opening.map(|o| o.qty) })),
    )?;
    tx.commit()?;
    get_product(conn, ctx, &id)
}

pub fn update_product(conn: &mut Connection, ctx: &Ctx, id: &str, input: ProductInput) -> AppResult<Product> {
    ctx.require(Permission::ManageCatalog)?;
    let tx = begin(conn)?;
    let old = repo::get(&tx, id)?;
    let c = clean_input(&tx, &input, Some(id))?;
    let sku = c.sku.clone().unwrap_or_else(|| old.sku.clone());
    let old_cost = old.cost_price.unwrap_or(0);
    let cost = match input.cost_price {
        Some(new_cost) if new_cost != old_cost => {
            if old.stock_qty > 0 {
                return Err(AppError::validation(
                    "Cost price is calculated from purchases while stock is on hand. Record a purchase or stock adjustment instead.",
                ));
            }
            new_cost
        }
        _ => old_cost,
    };
    if !c.allow_decimal && old.stock_qty % QTY_SCALE != 0 {
        return Err(AppError::validation("This product has fractional stock, so its unit must allow decimals."));
    }
    repo::update(
        &tx,
        &ProductRow {
            id,
            name: &c.name,
            sku: &sku,
            barcode: c.barcode.as_deref(),
            category_id: c.category_id.as_deref(),
            brand_id: c.brand_id.as_deref(),
            unit_id: input.unit_id.trim(),
            variant_group_id: c.variant_group_id.as_deref().or(old.variant_group_id.as_deref()),
            size: c.size.as_deref(),
            color: c.color.as_deref(),
            description: c.description.as_deref(),
            sale_price: input.sale_price,
            wholesale_price: input.wholesale_price,
            cost_price: cost,
            reorder_level: input.reorder_level,
            track_batches: input.track_batches,
            image_ref: c.image_ref.as_deref(),
            is_active: input.is_active.unwrap_or(old.is_active),
            now: &ctx.ts(),
        },
    )?;
    let mut changes = Vec::new();
    if old.sale_price != input.sale_price {
        changes.push(format!("price {} → {}", format_amount(old.sale_price, 2), format_amount(input.sale_price, 2)));
    }
    if old.name != c.name {
        changes.push(format!("name '{}' → '{}'", old.name, c.name));
    }
    if old.barcode != c.barcode {
        changes.push("barcode changed".into());
    }
    if old_cost != cost {
        changes.push(format!("cost {} → {}", format_amount(old_cost, 2), format_amount(cost, 2)));
    }
    let summary = if changes.is_empty() {
        format!("Product '{}' updated", c.name)
    } else {
        format!("Product '{}' updated: {}", c.name, changes.join(", "))
    };
    audit::record(
        &tx,
        ctx,
        "product.updated",
        "product",
        Some(id),
        summary,
        Some(json!({ "oldSalePrice": old.sale_price, "newSalePrice": input.sale_price })),
    )?;
    tx.commit()?;
    get_product(conn, ctx, id)
}

pub fn set_product_active(conn: &mut Connection, ctx: &Ctx, id: &str, active: bool) -> AppResult<Product> {
    ctx.require(Permission::ManageCatalog)?;
    let tx = begin(conn)?;
    let p = repo::get(&tx, id)?;
    repo::set_active(&tx, id, active, &ctx.ts())?;
    let verb = if active { "reactivated" } else { "deactivated" };
    audit::record(&tx, ctx, "product.status", "product", Some(id), format!("Product '{}' {verb}", p.name), None)?;
    tx.commit()?;
    get_product(conn, ctx, id)
}

/// A new unique in-store EAN-13 barcode (prefix 20).
pub fn generate_barcode(conn: &mut Connection, ctx: &Ctx) -> AppResult<String> {
    ctx.require(Permission::ManageCatalog)?;
    let tx = begin(conn)?;
    for _ in 0..1000 {
        let (_, seq, _) = sequences::next_value(&tx, "internal_barcode")?;
        let code = v::internal_ean13(seq);
        if !repo::barcode_taken(&tx, &code, None)? {
            tx.commit()?;
            return Ok(code);
        }
    }
    Err(AppError::Internal("Could not generate a unique barcode.".into()))
}
