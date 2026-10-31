mod common;

use codexmart_core::domain::payments::PaymentMethod;
use codexmart_core::reports::{self, DateRange};
use codexmart_core::services::cash::{self, CloseInput};
use codexmart_core::services::expenses::{self, ExpenseInput};
use codexmart_core::services::inventory::{self, AdjustMode, AdjustmentInput, AdjustmentLineInput};
use codexmart_core::services::purchasing::{self, PayNow, PurchaseInput, PurchaseLineInput};
use codexmart_core::services::returns::{self, ReturnInput, ReturnLineInput};
use codexmart_core::services::suppliers::{self, SupplierInput};
use common::*;

fn day() -> DateRange {
    DateRange { from: "2026-11-01".into(), to: "2026-11-01".into() }
}

fn expense(shop: &mut Shop, category: &str, amount: i64) -> String {
    let manager = shop.manager.clone();
    expenses::create(
        &mut shop.conn,
        &manager,
        ExpenseInput {
            client_txn_id: txn(),
            category_id: category.into(),
            amount,
            method: PaymentMethod::Cash,
            payee: None,
            note: None,
            business_date: None,
        },
    )
    .unwrap()
    .id
}

/// A full trading day, checked against hand-calculated figures.
#[test]
fn profit_and_loss_uses_recorded_cost_and_counts_nothing_twice() {
    let mut shop = Shop::new();
    let manager = shop.manager.clone();
    let supplier = suppliers::create(
        &mut shop.conn,
        &manager,
        SupplierInput {
            name: "Wholesaler".into(),
            contact_person: None,
            phone: None,
            email: None,
            address: None,
            opening_balance: 0,
            notes: None,
            is_active: None,
        },
    )
    .unwrap()
    .id;
    let cream = shop.product("Cream", rs(500), rs(0), 0);
    // Buy 10 @ 300 and pay 3000 cash: neither is an expense.
    purchasing::create_purchase(
        &mut shop.conn,
        &manager,
        PurchaseInput {
            client_txn_id: txn(),
            supplier_id: supplier,
            supplier_invoice_no: None,
            po_id: None,
            business_date: None,
            discount: 0,
            note: None,
            lines: vec![PurchaseLineInput {
                product_id: cream.id.clone(),
                qty: units(10),
                unit_cost: rs(300),
                batch_no: None,
                expiry_date: None,
                po_item_id: None,
                new_sale_price: None,
            }],
            payment: Some(PayNow { amount: rs(3000), method: PaymentMethod::Cash, reference: None }),
        },
    )
    .unwrap();

    // Sell 4 (2000 revenue, 1200 cost) and 2 (1000 revenue, 600 cost).
    let s1 = shop.sell(&[(&cream.id, 4)]);
    shop.sell(&[(&cream.id, 2)]);
    // Return 1 from the first sale, back on the shelf: -500 revenue, -300 cost.
    returns::create_return(
        &mut shop.conn,
        &manager,
        ReturnInput {
            client_txn_id: txn(),
            sale_id: s1.summary.id.clone(),
            lines: vec![ReturnLineInput { sale_item_id: s1.items[0].id.clone(), qty: units(1), restock: true }],
            refund_method: "cash".into(),
            reason: None,
        },
    )
    .unwrap();
    // A voided sale must not count anywhere.
    let v = shop.sell(&[(&cream.id, 1)]);
    codexmart_core::services::sales::void_sale(&mut shop.conn, &manager, &v.summary.id, "test").unwrap();
    // Expenses: 1000 rent + 250 electricity; a voided 999 does not count.
    expense(&mut shop, RENT, rs(1000));
    expense(&mut shop, ELECTRICITY, rs(250));
    let wrong = expense(&mut shop, RENT, rs(999));
    expenses::void(&mut shop.conn, &manager, &wrong, "duplicate entry").unwrap();
    // One damaged unit written off: -300 inventory loss.
    inventory::adjust(
        &mut shop.conn,
        &manager,
        AdjustmentInput {
            client_txn_id: txn(),
            reason: "damaged".into(),
            note: None,
            lines: vec![AdjustmentLineInput {
                product_id: cream.id.clone(),
                mode: AdjustMode::Remove,
                qty: units(1),
                unit_cost: None,
                batch_id: None,
                batch_no: None,
                expiry_date: None,
            }],
        },
    )
    .unwrap();

    let pl = reports::profit_loss(&shop.conn, &manager, &day()).unwrap();
    assert_eq!(pl.invoices, 2);
    assert_eq!(pl.sales_total, rs(3000));
    assert_eq!(pl.returns, rs(500));
    assert_eq!(pl.net_sales, rs(2500));
    assert_eq!(pl.cogs, rs(1500));
    assert_eq!(pl.gross_profit, rs(1000));
    assert_eq!(pl.expenses_total, rs(1250));
    assert_eq!(pl.inventory_adjustments, -rs(300));
    assert_eq!(pl.net_operating_result, rs(1000 - 1250 - 300));
    assert_eq!(pl.gross_margin_bp, 4000);

    let summary = reports::sales_summary(&shop.conn, &manager, &day(), "day").unwrap();
    assert_eq!(summary.totals.net_sales, pl.net_sales);
    assert_eq!(summary.totals.gross_profit, pl.gross_profit);

    let by_product = reports::product_sales(&shop.conn, &manager, &day()).unwrap();
    assert_eq!(by_product.len(), 1);
    assert_eq!(by_product[0].qty_sold - by_product[0].qty_returned, units(5));
    assert_eq!(by_product[0].gross_profit, rs(1000));
    let by_category = reports::category_sales(&shop.conn, &manager, &day()).unwrap();
    assert_eq!(by_category[0].net_sales, rs(2500));
    assert_eq!(by_category[0].share_bp, 10_000);

    // Stock: 10 bought - 6 sold + 1 returned - 1 written off = 4 @ 300.
    let valuation = reports::inventory_valuation(&shop.conn, &manager).unwrap();
    assert_eq!(valuation.total_cost_value, rs(1200));
    assert_eq!(valuation.total_retail_value, rs(2000));

    // Cash: +3000 sales -500 refund -1250 expenses -3000 supplier = -1750 (+ 5000 float)
    let preview = cash::preview(&shop.conn, &manager, None).unwrap();
    assert_eq!(preview.cash_sales, rs(3000));
    assert_eq!(preview.cash_refunds, rs(500));
    assert_eq!(preview.cash_expenses, rs(1250));
    assert_eq!(preview.cash_supplier_payments, rs(3000));
    assert_eq!(preview.net_cash, -rs(1750));
    let closing = cash::close(
        &mut shop.conn,
        &manager,
        CloseInput { business_date: None, opening_float: rs(5000), counted_cash: rs(3200), note: None },
    )
    .unwrap();
    assert_eq!(closing.expected_cash, rs(3250));
    assert_eq!(closing.difference, -rs(50));
    assert!(cash::close(
        &mut shop.conn,
        &manager,
        CloseInput { business_date: None, opening_float: 0, counted_cash: 0, note: None }
    )
    .is_err());

    let dash = reports::dashboard(&shop.conn, &manager).unwrap();
    assert_eq!(dash.day.net_sales, rs(2500));
    assert_eq!(dash.trend.len(), 14);
    assert_eq!(dash.trend.last().unwrap().net_sales, rs(2500));
    assert_eq!(dash.payables, 0);
    assert!(inventory::verify_ledger(&shop.conn, &manager).unwrap().is_empty());
}

#[test]
fn opening_stock_is_not_a_profit() {
    let mut shop = Shop::new();
    shop.product("Bag", rs(3000), rs(2000), 5);
    let pl = reports::profit_loss(&shop.conn, &shop.manager, &day()).unwrap();
    assert_eq!(pl.inventory_adjustments, 0);
    assert_eq!(pl.net_operating_result, 0);
}

#[test]
fn stock_count_adjustment_sets_quantity() {
    let mut shop = Shop::new();
    let p = shop.product("Lipstick", rs(800), rs(400), 10);
    let manager = shop.manager.clone();
    let adj = inventory::adjust(
        &mut shop.conn,
        &manager,
        AdjustmentInput {
            client_txn_id: txn(),
            reason: "stock_count".into(),
            note: Some("Monthly count".into()),
            lines: vec![AdjustmentLineInput {
                product_id: p.id.clone(),
                mode: AdjustMode::Set,
                qty: units(7),
                unit_cost: None,
                batch_id: None,
                batch_no: None,
                expiry_date: None,
            }],
        },
    )
    .unwrap();
    assert_eq!(adj.lines[0].qty_change, -units(3));
    assert_eq!(adj.value_change, Some(-rs(1200)));
    assert_eq!(shop.stock(&p.id).0, units(7));
    // A reason that only removes stock cannot add it.
    let bad = AdjustmentInput {
        client_txn_id: txn(),
        reason: "damaged".into(),
        note: None,
        lines: vec![AdjustmentLineInput {
            product_id: p.id.clone(),
            mode: AdjustMode::Add,
            qty: units(1),
            unit_cost: None,
            batch_id: None,
            batch_no: None,
            expiry_date: None,
        }],
    };
    assert!(inventory::adjust(&mut shop.conn, &manager, bad).is_err());
}

#[test]
fn reports_require_permission() {
    let shop = Shop::new();
    assert!(reports::profit_loss(&shop.conn, &shop.cashier, &day()).is_err());
    assert!(reports::dashboard(&shop.conn, &shop.cashier).is_err());
    let bad_range = DateRange { from: "2026-11-02".into(), to: "2026-11-01".into() };
    assert!(reports::profit_loss(&shop.conn, &shop.manager, &bad_range).is_err());
}
