//! Business reports and the dashboard.
//!
//! Accounting rules used throughout:
//! * Net sales = invoice totals (after discounts) − refunds given on returns.
//! * Cost of goods = cost recorded on each sale line at the moment of sale
//!   − cost of returned goods that went back on the shelf.
//! * Gross profit = net sales − cost of goods.
//! * Net operating result = gross profit − operating expenses
//!   + stock adjustment gains − stock write-offs.
//! * Purchases and supplier payments are NOT expenses: purchases become
//!   inventory and reach profit only through cost of goods when sold.
//!   Supplier payments only settle the payable. Nothing is counted twice.
//! * Voided sales and voided expenses are excluded everywhere.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::money::{self, Money, Qty, PERCENT_SCALE};
use crate::repo::customers::{self, Customer, CustomerFilter};
use crate::repo::products::{self as products_repo, Product, ProductFilter};
use crate::repo::purchases::{self, DocFilter, PurchaseSummary};
use crate::repo::sales::{self as sales_repo, ReturnFilter, SaleFilter, SaleReturnSummary, SaleSummary};
use crate::repo::stock::{self as stock_repo, Batch, BatchFilter};
use crate::repo::suppliers::{self, Supplier, SupplierFilter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

impl DateRange {
    pub fn validated(&self) -> AppResult<(String, String)> {
        let from = v::date("From date", &self.from)?;
        let to = v::date("To date", &self.to)?;
        if from > to {
            return Err(AppError::validation("The start date must be before the end date."));
        }
        if (to - from).num_days() > 3700 {
            return Err(AppError::validation("Reports can cover at most 10 years."));
        }
        Ok((from.format("%Y-%m-%d").to_string(), to.format("%Y-%m-%d").to_string()))
    }
}

fn margin_bp(profit: Money, sales: Money) -> i64 {
    if sales == 0 {
        0
    } else {
        money::prorate(PERCENT_SCALE, profit, sales)
    }
}

fn opt_sum(conn: &Connection, sql: &str, from: &str, to: &str) -> AppResult<Money> {
    Ok(conn.query_row(sql, params![from, to], |r| r.get::<_, Option<Money>>(0))?.unwrap_or(0))
}

// ------------------------------------------------------- profit and loss

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedAmount {
    pub name: String,
    pub amount: Money,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfitLoss {
    pub from: String,
    pub to: String,
    pub invoices: i64,
    pub gross_sales: Money,
    pub discounts: Money,
    pub sales_total: Money,
    pub returns: Money,
    pub net_sales: Money,
    pub cogs_sold: Money,
    pub cogs_returned: Money,
    pub cogs: Money,
    pub gross_profit: Money,
    pub gross_margin_bp: i64,
    pub expenses: Vec<NamedAmount>,
    pub expenses_total: Money,
    /// Stock gains minus write-offs (opening stock excluded). Negative = loss.
    pub inventory_adjustments: Money,
    pub net_operating_result: Money,
    pub average_bill: Money,
}

pub fn profit_loss_raw(conn: &Connection, from: &str, to: &str) -> AppResult<ProfitLoss> {
    let (invoices, gross_sales, discounts, sales_total, cogs_sold): (i64, Money, Money, Money, Money) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(subtotal),0), COALESCE(SUM(line_discount_total + bill_discount),0),
            COALESCE(SUM(total),0), COALESCE(SUM(cogs_total),0)
         FROM sales WHERE status = 'completed' AND business_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let (returns, cogs_returned): (Money, Money) = conn.query_row(
        "SELECT COALESCE(SUM(total_refund),0), COALESCE(SUM(cost_reversed),0)
         FROM sale_returns WHERE business_date BETWEEN ?1 AND ?2",
        params![from, to],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut stmt = conn.prepare(
        "SELECT c.name, SUM(e.amount), COUNT(*) FROM expenses e JOIN expense_categories c ON c.id = e.category_id
         WHERE e.status = 'active' AND e.business_date BETWEEN ?1 AND ?2 GROUP BY c.id ORDER BY 2 DESC",
    )?;
    let expenses: Vec<NamedAmount> = stmt
        .query_map(params![from, to], |r| Ok(NamedAmount { name: r.get(0)?, amount: r.get(1)?, count: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    let expenses_total: Money = expenses.iter().map(|e| e.amount).sum();
    let inventory_adjustments = opt_sum(
        conn,
        "SELECT SUM(i.value_change) FROM stock_adjustment_items i JOIN stock_adjustments a ON a.id = i.adjustment_id
         WHERE a.reason <> 'opening_stock' AND a.business_date BETWEEN ?1 AND ?2",
        from,
        to,
    )?;
    let net_sales = sales_total - returns;
    let cogs = cogs_sold - cogs_returned;
    let gross_profit = net_sales - cogs;
    Ok(ProfitLoss {
        from: from.to_string(),
        to: to.to_string(),
        invoices,
        gross_sales,
        discounts,
        sales_total,
        returns,
        net_sales,
        cogs_sold,
        cogs_returned,
        cogs,
        gross_profit,
        gross_margin_bp: margin_bp(gross_profit, net_sales),
        expenses,
        expenses_total,
        inventory_adjustments,
        net_operating_result: gross_profit - expenses_total + inventory_adjustments,
        average_bill: if invoices > 0 { money::prorate(sales_total, 1, invoices) } else { 0 },
    })
}

pub fn profit_loss(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<ProfitLoss> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    profit_loss_raw(conn, &from, &to)
}

// ---------------------------------------------------------- sales summary

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesPeriodRow {
    pub period: String,
    pub invoices: i64,
    pub gross_sales: Money,
    pub discounts: Money,
    pub sales_total: Money,
    pub returns_total: Money,
    pub net_sales: Money,
    pub cogs: Money,
    pub gross_profit: Money,
    pub margin_bp: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalesSummary {
    pub group_by: String,
    pub rows: Vec<SalesPeriodRow>,
    pub totals: SalesPeriodRow,
}

fn period_expr(group_by: &str, col: &str) -> AppResult<String> {
    Ok(match group_by {
        "day" => col.to_string(),
        // Monday of the ISO week.
        "week" => format!("date({col}, '-' || ((CAST(strftime('%w', {col}) AS INTEGER) + 6) % 7) || ' days')"),
        "month" => format!("substr({col}, 1, 7)"),
        _ => return Err(AppError::validation("Group by day, week or month.")),
    })
}

pub fn sales_summary(conn: &Connection, ctx: &Ctx, range: &DateRange, group_by: &str) -> AppResult<SalesSummary> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let sp = period_expr(group_by, "business_date")?;
    let mut rows: BTreeMap<String, SalesPeriodRow> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT {sp}, COUNT(*), SUM(subtotal), SUM(line_discount_total + bill_discount), SUM(total), SUM(cogs_total)
             FROM sales WHERE status = 'completed' AND business_date BETWEEN ?1 AND ?2 GROUP BY 1"
        ))?;
        let mut q = stmt.query(params![from, to])?;
        while let Some(r) = q.next()? {
            let period: String = r.get(0)?;
            let row = rows.entry(period.clone()).or_insert_with(|| SalesPeriodRow { period, ..Default::default() });
            row.invoices = r.get(1)?;
            row.gross_sales = r.get(2)?;
            row.discounts = r.get(3)?;
            row.sales_total = r.get(4)?;
            row.cogs = r.get(5)?;
        }
    }
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT {sp}, SUM(total_refund), SUM(cost_reversed) FROM sale_returns
             WHERE business_date BETWEEN ?1 AND ?2 GROUP BY 1"
        ))?;
        let mut q = stmt.query(params![from, to])?;
        while let Some(r) = q.next()? {
            let period: String = r.get(0)?;
            let row = rows.entry(period.clone()).or_insert_with(|| SalesPeriodRow { period, ..Default::default() });
            row.returns_total = r.get(1)?;
            row.cogs -= r.get::<_, Money>(2)?;
        }
    }
    let mut totals = SalesPeriodRow { period: "Total".into(), ..Default::default() };
    let rows: Vec<SalesPeriodRow> = rows
        .into_values()
        .map(|mut r| {
            r.net_sales = r.sales_total - r.returns_total;
            r.gross_profit = r.net_sales - r.cogs;
            r.margin_bp = margin_bp(r.gross_profit, r.net_sales);
            totals.invoices += r.invoices;
            totals.gross_sales += r.gross_sales;
            totals.discounts += r.discounts;
            totals.sales_total += r.sales_total;
            totals.returns_total += r.returns_total;
            totals.net_sales += r.net_sales;
            totals.cogs += r.cogs;
            totals.gross_profit += r.gross_profit;
            r
        })
        .collect();
    totals.margin_bp = margin_bp(totals.gross_profit, totals.net_sales);
    Ok(SalesSummary { group_by: group_by.to_string(), rows, totals })
}

// ---------------------------------------------------- product / category

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSalesRow {
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub category: String,
    pub brand: String,
    pub qty_sold: Qty,
    pub qty_returned: Qty,
    pub sales_amount: Money,
    pub returns_amount: Money,
    pub net_sales: Money,
    pub cogs: Money,
    pub gross_profit: Money,
    pub margin_bp: i64,
}

fn product_sales_raw(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<ProductSalesRow>> {
    let mut stmt = conn.prepare(
        "WITH sold AS (
            SELECT si.product_id, SUM(si.qty) q, SUM(si.net_amount) amt, SUM(si.cost_amount) cost
            FROM sale_items si JOIN sales s ON s.id = si.sale_id
            WHERE s.status = 'completed' AND s.business_date BETWEEN ?1 AND ?2 GROUP BY si.product_id),
         ret AS (
            SELECT ri.product_id, SUM(ri.qty) q, SUM(ri.refund_amount) amt,
                   SUM(CASE WHEN ri.restock = 1 THEN ri.cost_amount ELSE 0 END) cost
            FROM sale_return_items ri JOIN sale_returns r ON r.id = ri.return_id
            WHERE r.business_date BETWEEN ?1 AND ?2 GROUP BY ri.product_id),
         ids AS (SELECT product_id FROM sold UNION SELECT product_id FROM ret)
         SELECT p.id, p.name, p.size, p.color, p.sku, IFNULL(c.name, 'Uncategorised'), IFNULL(b.name, '-'),
            COALESCE(sold.q, 0), COALESCE(ret.q, 0), COALESCE(sold.amt, 0), COALESCE(ret.amt, 0),
            COALESCE(sold.cost, 0) - COALESCE(ret.cost, 0)
         FROM ids JOIN products p ON p.id = ids.product_id
         LEFT JOIN categories c ON c.id = p.category_id LEFT JOIN brands b ON b.id = p.brand_id
         LEFT JOIN sold ON sold.product_id = ids.product_id LEFT JOIN ret ON ret.product_id = ids.product_id
         ORDER BY COALESCE(sold.amt, 0) - COALESCE(ret.amt, 0) DESC",
    )?;
    let rows = stmt.query_map(params![from, to], |r| {
        let name: String = r.get(1)?;
        let size: Option<String> = r.get(2)?;
        let color: Option<String> = r.get(3)?;
        let sales_amount: Money = r.get(9)?;
        let returns_amount: Money = r.get(10)?;
        let cogs: Money = r.get(11)?;
        let net = sales_amount - returns_amount;
        Ok(ProductSalesRow {
            product_id: r.get(0)?,
            name: products_repo::display_name(&name, size.as_deref(), color.as_deref()),
            sku: r.get(4)?,
            category: r.get(5)?,
            brand: r.get(6)?,
            qty_sold: r.get(7)?,
            qty_returned: r.get(8)?,
            sales_amount,
            returns_amount,
            net_sales: net,
            cogs,
            gross_profit: net - cogs,
            margin_bp: margin_bp(net - cogs, net),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn product_sales(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<Vec<ProductSalesRow>> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    product_sales_raw(conn, &from, &to)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySalesRow {
    pub category: String,
    pub products: i64,
    pub qty_sold: Qty,
    pub net_sales: Money,
    pub cogs: Money,
    pub gross_profit: Money,
    pub margin_bp: i64,
    /// Share of total net sales in basis points.
    pub share_bp: i64,
}

pub fn category_sales(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<Vec<CategorySalesRow>> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let mut by_cat: BTreeMap<String, CategorySalesRow> = BTreeMap::new();
    for p in product_sales_raw(conn, &from, &to)? {
        let row = by_cat.entry(p.category.clone()).or_insert_with(|| CategorySalesRow {
            category: p.category.clone(),
            products: 0,
            qty_sold: 0,
            net_sales: 0,
            cogs: 0,
            gross_profit: 0,
            margin_bp: 0,
            share_bp: 0,
        });
        row.products += 1;
        row.qty_sold += p.qty_sold - p.qty_returned;
        row.net_sales += p.net_sales;
        row.cogs += p.cogs;
        row.gross_profit += p.gross_profit;
    }
    let total: Money = by_cat.values().map(|r| r.net_sales).sum();
    let mut rows: Vec<CategorySalesRow> = by_cat
        .into_values()
        .map(|mut r| {
            r.margin_bp = margin_bp(r.gross_profit, r.net_sales);
            r.share_bp = margin_bp(r.net_sales, total);
            r
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.net_sales));
    Ok(rows)
}

// -------------------------------------------------------------- purchases

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPurchaseRow {
    pub supplier_id: String,
    pub supplier_name: String,
    pub invoices: i64,
    pub purchases: Money,
    pub returns: Money,
    pub payments: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReport {
    pub by_supplier: Vec<SupplierPurchaseRow>,
    pub invoices: i64,
    pub purchases_total: Money,
    pub discounts: Money,
    pub returns_total: Money,
    pub net_purchases: Money,
    pub payments_total: Money,
    pub documents: Vec<PurchaseSummary>,
}

pub fn purchases_report(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<PurchaseReport> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name,
            (SELECT COUNT(*) FROM purchases p WHERE p.supplier_id = s.id AND p.business_date BETWEEN ?1 AND ?2),
            (SELECT COALESCE(SUM(total),0) FROM purchases p WHERE p.supplier_id = s.id AND p.business_date BETWEEN ?1 AND ?2),
            (SELECT COALESCE(SUM(total),0) FROM purchase_returns r WHERE r.supplier_id = s.id AND r.business_date BETWEEN ?1 AND ?2),
            (SELECT COALESCE(SUM(amount),0) FROM supplier_payments x WHERE x.supplier_id = s.id AND x.business_date BETWEEN ?1 AND ?2)
         FROM suppliers s ORDER BY 4 DESC, s.name",
    )?;
    let by_supplier: Vec<SupplierPurchaseRow> = stmt
        .query_map(params![from, to], |r| {
            Ok(SupplierPurchaseRow {
                supplier_id: r.get(0)?,
                supplier_name: r.get(1)?,
                invoices: r.get(2)?,
                purchases: r.get(3)?,
                returns: r.get(4)?,
                payments: r.get(5)?,
            })
        })?
        .filter(|r| r.as_ref().map(|r| r.invoices > 0 || r.returns > 0 || r.payments > 0).unwrap_or(true))
        .collect::<Result<_, _>>()?;
    let discounts =
        opt_sum(conn, "SELECT SUM(discount) FROM purchases WHERE business_date BETWEEN ?1 AND ?2", &from, &to)?;
    let invoices = by_supplier.iter().map(|r| r.invoices).sum();
    let purchases_total: Money = by_supplier.iter().map(|r| r.purchases).sum();
    let returns_total: Money = by_supplier.iter().map(|r| r.returns).sum();
    let payments_total = by_supplier.iter().map(|r| r.payments).sum();
    let documents = purchases::list_purchases(
        conn,
        &DocFilter { date_from: Some(from), date_to: Some(to), limit: Some(1000), ..Default::default() },
    )?
    .items;
    Ok(PurchaseReport {
        by_supplier,
        invoices,
        purchases_total,
        discounts,
        returns_total,
        net_purchases: purchases_total - returns_total,
        payments_total,
        documents,
    })
}

// --------------------------------------------------------------- expenses

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseReport {
    pub by_category: Vec<NamedAmount>,
    pub by_method: Vec<NamedAmount>,
    pub total: Money,
    pub count: i64,
}

pub fn expenses_report(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<ExpenseReport> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let pl = profit_loss_raw(conn, &from, &to)?;
    let mut stmt = conn.prepare(
        "SELECT method, SUM(amount), COUNT(*) FROM expenses WHERE status = 'active' AND business_date BETWEEN ?1 AND ?2
         GROUP BY method ORDER BY 2 DESC",
    )?;
    let by_method = stmt
        .query_map(params![from, to], |r| Ok(NamedAmount { name: r.get(0)?, amount: r.get(1)?, count: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    Ok(ExpenseReport {
        count: pl.expenses.iter().map(|e| e.count).sum(),
        total: pl.expenses_total,
        by_category: pl.expenses,
        by_method,
    })
}

// ---------------------------------------------------------------- returns

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnsReport {
    pub count: i64,
    pub total_refund: Money,
    pub by_method: Vec<NamedAmount>,
    pub top_products: Vec<ProductSalesRow>,
    pub returns: Vec<SaleReturnSummary>,
}

pub fn returns_report(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<ReturnsReport> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let mut stmt = conn.prepare(
        "SELECT refund_method, SUM(total_refund), COUNT(*) FROM sale_returns WHERE business_date BETWEEN ?1 AND ?2
         GROUP BY refund_method ORDER BY 2 DESC",
    )?;
    let by_method: Vec<NamedAmount> = stmt
        .query_map(params![from, to], |r| Ok(NamedAmount { name: r.get(0)?, amount: r.get(1)?, count: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    let mut top_products: Vec<ProductSalesRow> =
        product_sales_raw(conn, &from, &to)?.into_iter().filter(|p| p.qty_returned > 0).collect();
    top_products.sort_by_key(|r| std::cmp::Reverse(r.returns_amount));
    top_products.truncate(20);
    let returns = sales_repo::list_returns(
        conn,
        &ReturnFilter { date_from: Some(from), date_to: Some(to), limit: Some(1000), ..Default::default() },
    )?
    .items;
    Ok(ReturnsReport {
        count: by_method.iter().map(|m| m.count).sum(),
        total_refund: by_method.iter().map(|m| m.amount).sum(),
        by_method,
        top_products,
        returns,
    })
}

// ------------------------------------------------------- payment methods

pub fn payment_methods(conn: &Connection, ctx: &Ctx, range: &DateRange) -> AppResult<Vec<NamedAmount>> {
    ctx.require(Permission::ViewReports)?;
    let (from, to) = range.validated()?;
    let mut stmt = conn.prepare(
        "SELECT p.method, SUM(p.amount), COUNT(DISTINCT p.sale_id) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE s.status = 'completed' AND s.business_date BETWEEN ?1 AND ?2 GROUP BY p.method
         UNION ALL
         SELECT 'credit', SUM(credit_amount), COUNT(*) FROM sales
         WHERE status = 'completed' AND credit_amount > 0 AND business_date BETWEEN ?1 AND ?2 HAVING COUNT(*) > 0
         ORDER BY 2 DESC",
    )?;
    let rows = stmt
        .query_map(params![from, to], |r| Ok(NamedAmount { name: r.get(0)?, amount: r.get(1)?, count: r.get(2)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---------------------------------------------------------------- balances

pub fn supplier_balances(conn: &Connection, ctx: &Ctx) -> AppResult<Vec<Supplier>> {
    ctx.require(Permission::ViewReports)?;
    let mut rows =
        suppliers::list(conn, &SupplierFilter { status: Some("all".into()), limit: Some(1000), ..Default::default() })?
            .items;
    rows.retain(|s| s.balance != 0 || s.is_active);
    rows.sort_by_key(|r| std::cmp::Reverse(r.balance));
    Ok(rows)
}

pub fn customer_balances(conn: &Connection, ctx: &Ctx) -> AppResult<Vec<Customer>> {
    ctx.require(Permission::ViewReports)?;
    let mut rows = customers::list(
        conn,
        &CustomerFilter { status: Some("all".into()), with_balance: true, limit: Some(1000), ..Default::default() },
    )?
    .items;
    rows.sort_by_key(|r| std::cmp::Reverse(r.balance));
    Ok(rows)
}

// ------------------------------------------------------------- inventory

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationRow {
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub category: String,
    pub unit_short: String,
    pub qty: Qty,
    pub unit_cost: Money,
    pub cost_value: Money,
    pub sale_price: Money,
    pub retail_value: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryValuation {
    pub rows: Vec<ValuationRow>,
    pub by_category: Vec<NamedAmount>,
    pub total_cost_value: Money,
    pub total_retail_value: Money,
    pub total_qty: Qty,
    pub products_in_stock: i64,
}

pub fn inventory_valuation_raw(conn: &Connection) -> AppResult<InventoryValuation> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.size, p.color, p.sku, IFNULL(c.name, 'Uncategorised'), u.short_name, p.stock_qty,
            p.cost_price, p.stock_value, p.sale_price
         FROM products p JOIN units u ON u.id = p.unit_id LEFT JOIN categories c ON c.id = p.category_id
         WHERE p.stock_qty <> 0 ORDER BY p.stock_value DESC, p.name COLLATE NOCASE",
    )?;
    let rows: Vec<ValuationRow> = stmt
        .query_map([], |r| {
            let name: String = r.get(1)?;
            let size: Option<String> = r.get(2)?;
            let color: Option<String> = r.get(3)?;
            let qty: Qty = r.get(7)?;
            let price: Money = r.get(10)?;
            Ok(ValuationRow {
                product_id: r.get(0)?,
                name: products_repo::display_name(&name, size.as_deref(), color.as_deref()),
                sku: r.get(4)?,
                category: r.get(5)?,
                unit_short: r.get(6)?,
                qty,
                unit_cost: r.get(8)?,
                cost_value: r.get(9)?,
                sale_price: price,
                retail_value: if qty > 0 { money::mul_qty(price, qty) } else { 0 },
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut cats: BTreeMap<String, NamedAmount> = BTreeMap::new();
    for r in &rows {
        let e = cats.entry(r.category.clone()).or_insert_with(|| NamedAmount {
            name: r.category.clone(),
            amount: 0,
            count: 0,
        });
        e.amount += r.cost_value;
        e.count += 1;
    }
    let mut by_category: Vec<NamedAmount> = cats.into_values().collect();
    by_category.sort_by_key(|r| std::cmp::Reverse(r.amount));
    Ok(InventoryValuation {
        total_cost_value: rows.iter().map(|r| r.cost_value).sum(),
        total_retail_value: rows.iter().map(|r| r.retail_value).sum(),
        total_qty: rows.iter().map(|r| r.qty.max(0)).sum(),
        products_in_stock: rows.iter().filter(|r| r.qty > 0).count() as i64,
        rows,
        by_category,
    })
}

pub fn inventory_valuation(conn: &Connection, ctx: &Ctx) -> AppResult<InventoryValuation> {
    ctx.require(Permission::ViewReports)?;
    ctx.require(Permission::ViewCosts)?;
    inventory_valuation_raw(conn)
}

pub fn low_stock(conn: &Connection, ctx: &Ctx) -> AppResult<Vec<Product>> {
    ctx.require(Permission::ViewCatalog)?;
    let mut rows = products_repo::list(
        conn,
        &ProductFilter {
            stock: Some("attention".into()),
            sort: Some("stock".into()),
            limit: Some(1000),
            ..Default::default()
        },
    )?
    .items;
    if !ctx.can(Permission::ViewCosts) {
        rows.iter_mut().for_each(Product::redact_costs);
    }
    Ok(rows)
}

pub fn expiring(conn: &Connection, ctx: &Ctx, days: i64) -> AppResult<Vec<Batch>> {
    ctx.require(Permission::ViewCatalog)?;
    let mut rows = stock_repo::list_batches(
        conn,
        &BatchFilter { only_available: true, expiring_within_days: Some(days), ..Default::default() },
        &ctx.date(),
    )?;
    if !ctx.can(Permission::ViewCosts) {
        rows.iter_mut().for_each(|b| b.unit_cost = None);
    }
    Ok(rows)
}

// -------------------------------------------------------------- dashboard

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub date: String,
    pub net_sales: Money,
    pub gross_profit: Money,
    pub invoices: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub today: String,
    pub day: ProfitLoss,
    pub month: ProfitLoss,
    pub stock_value: Money,
    pub stock_retail_value: Money,
    pub products_in_stock: i64,
    pub low_stock_count: i64,
    pub out_of_stock_count: i64,
    pub low_stock: Vec<Product>,
    pub recent_sales: Vec<SaleSummary>,
    pub trend: Vec<TrendPoint>,
    pub purchases_month: Money,
    pub payables: Money,
    pub receivables: Money,
    pub expiring_soon: i64,
}

fn trend(conn: &Connection, today: NaiveDate, days: i64) -> AppResult<Vec<TrendPoint>> {
    let start = today - Duration::days(days - 1);
    let (from, to) = (start.format("%Y-%m-%d").to_string(), today.format("%Y-%m-%d").to_string());
    let mut points: BTreeMap<String, TrendPoint> = BTreeMap::new();
    for i in 0..days {
        let d = (start + Duration::days(i)).format("%Y-%m-%d").to_string();
        points.insert(d.clone(), TrendPoint { date: d, net_sales: 0, gross_profit: 0, invoices: 0 });
    }
    let mut stmt = conn.prepare(
        "SELECT business_date, COUNT(*), SUM(total), SUM(total - cogs_total) FROM sales
         WHERE status = 'completed' AND business_date BETWEEN ?1 AND ?2 GROUP BY business_date",
    )?;
    let mut q = stmt.query(params![from, to])?;
    while let Some(r) = q.next()? {
        if let Some(p) = points.get_mut(&r.get::<_, String>(0)?) {
            p.invoices = r.get(1)?;
            p.net_sales = r.get(2)?;
            p.gross_profit = r.get(3)?;
        }
    }
    let mut stmt = conn.prepare(
        "SELECT business_date, SUM(total_refund), SUM(total_refund - cost_reversed) FROM sale_returns
         WHERE business_date BETWEEN ?1 AND ?2 GROUP BY business_date",
    )?;
    let mut q = stmt.query(params![from, to])?;
    while let Some(r) = q.next()? {
        if let Some(p) = points.get_mut(&r.get::<_, String>(0)?) {
            p.net_sales -= r.get::<_, Money>(1)?;
            p.gross_profit -= r.get::<_, Money>(2)?;
        }
    }
    Ok(points.into_values().collect())
}

pub fn dashboard(conn: &Connection, ctx: &Ctx) -> AppResult<Dashboard> {
    ctx.require(Permission::ViewReports)?;
    let today = ctx.date();
    let month_start = ctx.today.with_day(1).unwrap_or(ctx.today).format("%Y-%m-%d").to_string();
    let day = profit_loss_raw(conn, &today, &today)?;
    let month = profit_loss_raw(conn, &month_start, &today)?;
    let valuation = inventory_valuation_raw(conn)?;
    let (low, out): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN stock_qty > 0 AND reorder_level > 0 AND stock_qty <= reorder_level THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN stock_qty <= 0 THEN 1 ELSE 0 END), 0)
         FROM products WHERE is_active = 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let low_stock = products_repo::list(
        conn,
        &ProductFilter {
            stock: Some("attention".into()),
            sort: Some("stock".into()),
            limit: Some(8),
            ..Default::default()
        },
    )?
    .items;
    let recent_sales = sales_repo::list(conn, &SaleFilter { limit: Some(8), ..Default::default() })?.items;
    let purchases_month =
        opt_sum(conn, "SELECT SUM(total) FROM purchases WHERE business_date BETWEEN ?1 AND ?2", &month_start, &today)?;
    let payables: Money =
        suppliers::list(conn, &SupplierFilter { status: Some("all".into()), limit: Some(1000), ..Default::default() })?
            .items
            .iter()
            .map(|s| s.balance.max(0))
            .sum();
    let receivables: Money = customers::list(
        conn,
        &CustomerFilter { status: Some("all".into()), with_balance: true, limit: Some(1000), ..Default::default() },
    )?
    .items
    .iter()
    .map(|c| c.balance.max(0))
    .sum();
    let warn_days = crate::services::settings::load(conn)?.expiry_warning_days;
    let expiring_soon: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stock_batches WHERE qty_remaining > 0 AND expiry_date IS NOT NULL
         AND expiry_date <= date(?1, '+' || ?2 || ' days')",
        params![today, warn_days],
        |r| r.get(0),
    )?;
    Ok(Dashboard {
        trend: trend(conn, ctx.today, 14)?,
        today,
        day,
        month,
        stock_value: valuation.total_cost_value,
        stock_retail_value: valuation.total_retail_value,
        products_in_stock: valuation.products_in_stock,
        low_stock_count: low,
        out_of_stock_count: out,
        low_stock,
        recent_sales,
        purchases_month,
        payables,
        receivables,
        expiring_soon,
    })
}
