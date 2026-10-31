mod common;

use codexmart_core::domain::payments::{PaymentMethod, Tender};
use codexmart_core::domain::pricing::Discount;
use codexmart_core::error::AppError;
use codexmart_core::services::customers::{self, CustomerInput, ReceivePaymentInput};
use codexmart_core::services::inventory;
use codexmart_core::services::returns::{self, ReturnInput, ReturnLineInput};
use codexmart_core::services::sales::{self, Cart, CartLine, CheckoutInput};
use codexmart_core::services::settings;
use common::*;

fn customer(shop: &mut Shop, name: &str, limit: Option<i64>) -> String {
    customers::create(
        &mut shop.conn,
        &shop.manager,
        CustomerInput {
            name: name.into(),
            phone: Some("0311-0000000".into()),
            email: None,
            address: None,
            opening_balance: 0,
            credit_limit: limit,
            notes: None,
            is_active: None,
        },
    )
    .unwrap()
    .id
}

#[test]
fn sale_deducts_stock_records_cost_and_change() {
    let mut shop = Shop::new();
    let cream = shop.product("Cream", rs(450), rs(300), 10);
    let bag = shop.product("Bag", rs(2500), rs(1800), 2);
    let cashier = shop.cashier.clone();
    let sale = shop.sell_with(&cashier, Shop::cart(&[(&cream.id, 3), (&bag.id, 1)]), Shop::cash(rs(5000))).unwrap();
    assert_eq!(sale.summary.total, rs(450 * 3 + 2500));
    assert_eq!(sale.summary.paid_amount, sale.summary.total);
    assert_eq!(sale.change_amount, rs(5000) - sale.summary.total);
    assert_eq!(sale.tendered_cash, rs(5000));
    assert_eq!(sale.items.len(), 2);
    // Cashier cannot see cost, but it is recorded.
    assert_eq!(sale.cogs_total, None);
    let cogs: i64 =
        shop.conn.query_row("SELECT cogs_total FROM sales WHERE id = ?1", [&sale.summary.id], |r| r.get(0)).unwrap();
    assert_eq!(cogs, rs(300 * 3 + 1800));
    assert_eq!(shop.stock(&cream.id).0, units(7));
    assert_eq!(shop.stock(&bag.id).0, units(1));
    assert_eq!(shop.stock(&cream.id).1, rs(300 * 7));
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn invoice_numbers_are_sequential_and_unique() {
    let mut shop = Shop::new();
    let p = shop.product("Soap", rs(100), rs(60), 50);
    let a = shop.sell(&[(&p.id, 1)]);
    let b = shop.sell(&[(&p.id, 1)]);
    let c = shop.sell(&[(&p.id, 1)]);
    assert_eq!(a.summary.invoice_no, "INV-000001");
    assert_eq!(b.summary.invoice_no, "INV-000002");
    assert_eq!(c.summary.invoice_no, "INV-000003");
}

#[test]
fn retrying_a_checkout_never_creates_a_second_sale() {
    let mut shop = Shop::new();
    let p = shop.product("Perfume", rs(1200), rs(700), 5);
    let input = CheckoutInput {
        client_txn_id: txn(),
        cart: Shop::cart(&[(&p.id, 2)]),
        payments: Shop::cash(rs(2400)),
        note: None,
    };
    let cashier = shop.cashier.clone();
    let first = sales::checkout(&mut shop.conn, &cashier, input.clone()).unwrap();
    let second = sales::checkout(&mut shop.conn, &cashier, input).unwrap();
    assert_eq!(first.summary.id, second.summary.id);
    assert_eq!(first.summary.invoice_no, second.summary.invoice_no);
    assert_eq!(shop.count("sales"), 1);
    assert_eq!(shop.stock(&p.id).0, units(3));
}

#[test]
fn failed_sale_rolls_back_everything() {
    let mut shop = Shop::new();
    let a = shop.product("In stock", rs(100), rs(50), 5);
    let b = shop.product("Scarce", rs(100), rs(50), 1);
    let cashier = shop.cashier.clone();
    let err = shop.sell_with(&cashier, Shop::cart(&[(&a.id, 2), (&b.id, 3)]), Shop::cash(rs(500))).unwrap_err();
    assert!(matches!(err, AppError::InsufficientStock(_)), "{err:?}");
    // The first line's stock deduction was rolled back and no number was used.
    assert_eq!(shop.stock(&a.id).0, units(5));
    assert_eq!(shop.count("sales"), 0);
    assert_eq!(shop.count("sale_items"), 0);
    assert_eq!(shop.count("stock_movements"), 2);
    let next = shop.sell(&[(&a.id, 1)]);
    assert_eq!(next.summary.invoice_no, "INV-000001");
}

#[test]
fn same_product_on_two_lines_cannot_oversell() {
    let mut shop = Shop::new();
    let p = shop.product("Lip balm", rs(150), rs(90), 3);
    let cashier = shop.cashier.clone();
    let quote = sales::quote(&shop.conn, &cashier, &Shop::cart(&[(&p.id, 2), (&p.id, 2)])).unwrap();
    assert!(!quote.lines[0].stock_ok);
    assert!(!quote.warnings.is_empty());
    assert!(shop.sell_with(&cashier, Shop::cart(&[(&p.id, 2), (&p.id, 2)]), Shop::cash(rs(600))).is_err());
}

#[test]
fn discounts_are_applied_and_allocated_exactly() {
    let mut shop = Shop::new();
    let a = shop.product("A", rs(1000), rs(600), 10);
    let b = shop.product("B", rs(333), rs(200), 10);
    let cart = Cart {
        lines: vec![
            CartLine { product_id: a.id.clone(), qty: units(2), discount: Some(Discount::Percent(1000)) },
            CartLine { product_id: b.id.clone(), qty: units(1), discount: Some(Discount::Amount(rs(33))) },
        ],
        bill_discount: Some(Discount::Amount(rs(100))),
        ..Default::default()
    };
    let quote = sales::quote(&shop.conn, &shop.cashier, &cart).unwrap();
    // 2000 - 200 = 1800; 333 - 33 = 300; subtotal after line = 2100; bill -100 = 2000
    assert_eq!(quote.totals.subtotal, rs(2333));
    assert_eq!(quote.totals.line_discount_total, rs(233));
    assert_eq!(quote.totals.total, rs(2000));
    let cashier = shop.cashier.clone();
    let sale = shop.sell_with(&cashier, cart, Shop::cash(rs(2000))).unwrap();
    assert_eq!(sale.summary.total, rs(2000));
    assert_eq!(sale.items.iter().map(|i| i.net_amount).sum::<i64>(), rs(2000));
    assert_eq!(sale.items.iter().map(|i| i.bill_discount_share).sum::<i64>(), rs(100));
}

#[test]
fn cashier_discount_limit_is_enforced() {
    let mut shop = Shop::new();
    let p = shop.product("Handbag", rs(5000), rs(3000), 5);
    let mut s = settings::load(&shop.conn).unwrap();
    s.max_cashier_discount_bp = 1000; // 10 %
    settings::save(&mut shop.conn, &shop.admin, s).unwrap();
    let mut cart = Shop::cart(&[(&p.id, 1)]);
    cart.bill_discount = Some(Discount::Percent(2000));
    let cashier = shop.cashier.clone();
    let err = shop.sell_with(&cashier, cart.clone(), Shop::cash(rs(4000))).unwrap_err();
    assert!(matches!(err, AppError::Forbidden(_)));
    let manager = shop.manager.clone();
    assert!(shop.sell_with(&manager, cart, Shop::cash(rs(4000))).is_ok());
}

#[test]
fn underpayment_requires_a_customer_and_respects_credit_limit() {
    let mut shop = Shop::new();
    let p = shop.product("Gift set", rs(3000), rs(2000), 10);
    let cashier = shop.cashier.clone();
    let err = shop.sell_with(&cashier, Shop::cart(&[(&p.id, 1)]), Shop::cash(rs(1000))).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));

    let c = customer(&mut shop, "Ayesha", Some(rs(5000)));
    let mut cart = Shop::cart(&[(&p.id, 1)]);
    cart.customer_id = Some(c.clone());
    let sale = shop.sell_with(&cashier, cart.clone(), Shop::cash(rs(1000))).unwrap();
    assert_eq!(sale.summary.credit_amount, rs(2000));
    let bal = customers::get(&shop.conn, &cashier, &c).unwrap().balance;
    assert_eq!(bal, rs(2000));

    // 2000 owed + 4000 new credit > 5000 limit
    let mut big = Shop::cart(&[(&p.id, 2)]);
    big.customer_id = Some(c.clone());
    assert!(shop.sell_with(&cashier, big, Shop::cash(rs(2000))).is_err());

    customers::receive_payment(
        &mut shop.conn,
        &cashier,
        ReceivePaymentInput {
            client_txn_id: txn(),
            customer_id: c.clone(),
            amount: rs(1500),
            method: PaymentMethod::Cash,
            note: None,
        },
    )
    .unwrap();
    assert_eq!(customers::get(&shop.conn, &cashier, &c).unwrap().balance, rs(500));
    let ledger = customers::ledger(&shop.conn, &cashier, &c).unwrap();
    assert_eq!(ledger.last().unwrap().balance, rs(500));
}

#[test]
fn split_payment_card_cannot_exceed_total() {
    let mut shop = Shop::new();
    let p = shop.product("Watch", rs(4000), rs(2500), 3);
    let cashier = shop.cashier.clone();
    let tenders = vec![
        Tender { method: PaymentMethod::Card, amount: rs(3000), reference: Some("1234".into()) },
        Tender { method: PaymentMethod::Cash, amount: rs(1500), reference: None },
    ];
    let sale = shop.sell_with(&cashier, Shop::cart(&[(&p.id, 1)]), tenders).unwrap();
    assert_eq!(sale.change_amount, rs(500));
    assert_eq!(sale.payments.iter().map(|p| p.amount).sum::<i64>(), rs(4000));
    let over = vec![Tender { method: PaymentMethod::Card, amount: rs(5000), reference: None }];
    assert!(shop.sell_with(&cashier, Shop::cart(&[(&p.id, 1)]), over).is_err());
}

#[test]
fn void_restores_stock_and_requires_permission() {
    let mut shop = Shop::new();
    let p = shop.product("Serum", rs(1500), rs(900), 4);
    let sale = shop.sell(&[(&p.id, 2)]);
    let cashier = shop.cashier.clone();
    assert!(matches!(
        sales::void_sale(&mut shop.conn, &cashier, &sale.summary.id, "mistake"),
        Err(AppError::Forbidden(_))
    ));
    let manager = shop.manager.clone();
    let voided = sales::void_sale(&mut shop.conn, &manager, &sale.summary.id, "Customer changed mind").unwrap();
    assert_eq!(voided.summary.status, "void");
    assert_eq!(shop.stock(&p.id), (units(4), rs(3600), rs(900)));
    assert!(sales::void_sale(&mut shop.conn, &manager, &sale.summary.id, "again").is_err());
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn old_sales_cannot_be_voided() {
    let mut shop = Shop::new();
    let p = shop.product("Toner", rs(800), rs(500), 4);
    let sale = shop.sell(&[(&p.id, 1)]);
    let mut tomorrow = shop.manager.clone();
    tomorrow.today = today().succ_opt().unwrap();
    assert!(matches!(
        sales::void_sale(&mut shop.conn, &tomorrow, &sale.summary.id, "late"),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn sales_records_cannot_be_edited_or_deleted() {
    let mut shop = Shop::new();
    let p = shop.product("Kajal", rs(250), rs(120), 4);
    let sale = shop.sell(&[(&p.id, 1)]);
    let id = &sale.summary.id;
    assert!(shop.conn.execute("UPDATE sales SET total = 1 WHERE id = ?1", [id]).is_err());
    assert!(shop.conn.execute("DELETE FROM sales WHERE id = ?1", [id]).is_err());
    assert!(shop.conn.execute("UPDATE sale_items SET qty = 99000 WHERE sale_id = ?1", [id]).is_err());
    assert!(shop.conn.execute("DELETE FROM sale_payments WHERE sale_id = ?1", [id]).is_err());
    assert!(shop.conn.execute("DELETE FROM stock_movements", []).is_err());
    assert!(shop.conn.execute("UPDATE audit_log SET summary = 'x'", []).is_err());
}

#[test]
fn partial_and_full_returns_refund_exact_amounts() {
    let mut shop = Shop::new();
    let p = shop.product("Nail polish", rs(333), rs(200), 10);
    let mut cart = Shop::cart(&[(&p.id, 3)]);
    cart.bill_discount = Some(Discount::Amount(rs(10)));
    let cashier = shop.cashier.clone();
    let sale = shop.sell_with(&cashier, cart, Shop::cash(rs(989))).unwrap();
    assert_eq!(sale.summary.total, rs(989));
    let item = sale.items[0].id.clone();
    let manager = shop.manager.clone();

    let make = |qty: i64, restock: bool| ReturnInput {
        client_txn_id: txn(),
        sale_id: sale.summary.id.clone(),
        lines: vec![ReturnLineInput { sale_item_id: item.clone(), qty: units(qty), restock }],
        refund_method: "cash".into(),
        reason: Some("Wrong shade".into()),
    };
    let r1 = returns::create_return(&mut shop.conn, &manager, make(1, true)).unwrap();
    assert_eq!(r1.summary.total_refund, 32_967); // 989.00 / 3, rounded
    assert_eq!(shop.stock(&p.id).0, units(8));
    let r2 = returns::create_return(&mut shop.conn, &manager, make(2, false)).unwrap();
    assert_eq!(r1.summary.total_refund + r2.summary.total_refund, rs(989));
    // Damaged goods were not restocked.
    assert_eq!(shop.stock(&p.id).0, units(8));
    // Nothing left to return.
    assert!(returns::create_return(&mut shop.conn, &manager, make(1, true)).is_err());
    let lines = returns::returnable(&shop.conn, &manager, &sale.summary.id).unwrap();
    assert_eq!(lines[0].qty_returnable, 0);
    assert!(sales::void_sale(&mut shop.conn, &manager, &sale.summary.id, "x").is_err());
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn return_to_account_reduces_customer_balance() {
    let mut shop = Shop::new();
    let p = shop.product("Clutch", rs(2000), rs(1200), 5);
    let c = customer(&mut shop, "Sana", None);
    let mut cart = Shop::cart(&[(&p.id, 2)]);
    cart.customer_id = Some(c.clone());
    let cashier = shop.cashier.clone();
    let sale = shop.sell_with(&cashier, cart, vec![]).unwrap();
    assert_eq!(sale.summary.credit_amount, rs(4000));
    let manager = shop.manager.clone();
    returns::create_return(
        &mut shop.conn,
        &manager,
        ReturnInput {
            client_txn_id: txn(),
            sale_id: sale.summary.id.clone(),
            lines: vec![ReturnLineInput { sale_item_id: sale.items[0].id.clone(), qty: units(1), restock: true }],
            refund_method: "account".into(),
            reason: None,
        },
    )
    .unwrap();
    assert_eq!(customers::get(&shop.conn, &manager, &c).unwrap().balance, rs(2000));
}

#[test]
fn returns_are_idempotent() {
    let mut shop = Shop::new();
    let p = shop.product("Brush", rs(500), rs(300), 5);
    let sale = shop.sell(&[(&p.id, 2)]);
    let input = ReturnInput {
        client_txn_id: txn(),
        sale_id: sale.summary.id.clone(),
        lines: vec![ReturnLineInput { sale_item_id: sale.items[0].id.clone(), qty: units(1), restock: true }],
        refund_method: "cash".into(),
        reason: None,
    };
    let manager = shop.manager.clone();
    let a = returns::create_return(&mut shop.conn, &manager, input.clone()).unwrap();
    let b = returns::create_return(&mut shop.conn, &manager, input).unwrap();
    assert_eq!(a.summary.id, b.summary.id);
    assert_eq!(shop.count("sale_returns"), 1);
    assert_eq!(shop.stock(&p.id).0, units(4));
}

#[test]
fn negative_stock_only_when_enabled() {
    let mut shop = Shop::new();
    let p = shop.product("Unlisted item", rs(100), rs(70), 0);
    let cashier = shop.cashier.clone();
    assert!(shop.sell_with(&cashier, Shop::cart(&[(&p.id, 1)]), Shop::cash(rs(100))).is_err());
    let mut s = settings::load(&shop.conn).unwrap();
    s.allow_negative_stock = true;
    settings::save(&mut shop.conn, &shop.admin, s).unwrap();
    let sale = shop.sell_with(&cashier, Shop::cart(&[(&p.id, 2)]), Shop::cash(rs(200))).unwrap();
    assert_eq!(shop.stock(&p.id).0, -units(2));
    let cogs: i64 =
        shop.conn.query_row("SELECT cogs_total FROM sales WHERE id = ?1", [&sale.summary.id], |r| r.get(0)).unwrap();
    assert_eq!(cogs, rs(140));
}
