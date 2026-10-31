mod common;

use codexmart_core::domain::payments::PaymentMethod;
use codexmart_core::error::AppError;
use codexmart_core::repo::stock::BatchFilter;
use codexmart_core::services::catalog;
use codexmart_core::services::inventory;
use codexmart_core::services::purchasing::{
    self, PayNow, PoInput, PoLineInput, PurchaseInput, PurchaseLineInput, PurchaseReturnInput, PurchaseReturnLineInput,
    SupplierPaymentInput,
};
use codexmart_core::services::suppliers::{self, SupplierInput};
use common::*;

fn supplier(shop: &mut Shop, opening: i64) -> String {
    suppliers::create(
        &mut shop.conn,
        &shop.manager,
        SupplierInput {
            name: "Lahore Cosmetics Traders".into(),
            contact_person: Some("Imran".into()),
            phone: Some("042-1111111".into()),
            email: None,
            address: None,
            opening_balance: opening,
            notes: None,
            is_active: None,
        },
    )
    .unwrap()
    .id
}

fn line(product_id: &str, qty: i64, cost: i64) -> PurchaseLineInput {
    PurchaseLineInput {
        product_id: product_id.into(),
        qty: units(qty),
        unit_cost: cost,
        batch_no: None,
        expiry_date: None,
        po_item_id: None,
        new_sale_price: None,
    }
}

fn purchase(supplier_id: &str, lines: Vec<PurchaseLineInput>) -> PurchaseInput {
    PurchaseInput {
        client_txn_id: txn(),
        supplier_id: supplier_id.into(),
        supplier_invoice_no: None,
        po_id: None,
        business_date: None,
        discount: 0,
        note: None,
        lines,
        payment: None,
    }
}

#[test]
fn purchase_adds_stock_at_weighted_average_cost() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Sunblock", rs(900), rs(500), 10); // 10 @ 500
    let manager = shop.manager.clone();
    purchasing::create_purchase(&mut shop.conn, &manager, purchase(&s, vec![line(&p.id, 10, rs(600))])).unwrap();
    assert_eq!(shop.stock(&p.id), (units(20), rs(11_000), rs(550)));
    // The next sale is costed at the new average.
    let sale = shop.sell(&[(&p.id, 2)]);
    let cogs: i64 =
        shop.conn.query_row("SELECT cogs_total FROM sales WHERE id = ?1", [&sale.summary.id], |r| r.get(0)).unwrap();
    assert_eq!(cogs, rs(1100));
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn invoice_discount_is_allocated_into_stock_cost() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let a = shop.product("A", rs(200), rs(0), 0);
    let b = shop.product("B", rs(200), rs(0), 0);
    let mut input = purchase(&s, vec![line(&a.id, 10, rs(100)), line(&b.id, 10, rs(300))]);
    input.discount = rs(400); // 10 % of 4000
    let manager = shop.manager.clone();
    let doc = purchasing::create_purchase(&mut shop.conn, &manager, input).unwrap();
    assert_eq!(doc.summary.total, rs(3600));
    assert_eq!(shop.stock(&a.id).1, rs(900));
    assert_eq!(shop.stock(&b.id).1, rs(2700));
    assert_eq!(shop.stock(&a.id).2, rs(90));
}

#[test]
fn supplier_balance_tracks_purchases_payments_and_returns() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, rs(1000));
    let p = shop.product("Shampoo", rs(500), rs(0), 0);
    let manager = shop.manager.clone();
    let mut input = purchase(&s, vec![line(&p.id, 20, rs(250))]); // 5000
    input.payment = Some(PayNow { amount: rs(2000), method: PaymentMethod::Cash, reference: None });
    let doc = purchasing::create_purchase(&mut shop.conn, &manager, input).unwrap();
    assert_eq!(doc.summary.paid_amount, rs(2000));
    assert_eq!(suppliers::get(&shop.conn, &manager, &s).unwrap().balance, rs(1000 + 5000 - 2000));

    purchasing::pay_supplier(
        &mut shop.conn,
        &manager,
        SupplierPaymentInput {
            client_txn_id: txn(),
            supplier_id: s.clone(),
            purchase_id: Some(doc.summary.id.clone()),
            amount: rs(1500),
            method: PaymentMethod::Bank,
            reference: Some("TRX-77".into()),
            note: None,
            business_date: None,
        },
    )
    .unwrap();
    let ret = purchasing::return_to_supplier(
        &mut shop.conn,
        &manager,
        PurchaseReturnInput {
            client_txn_id: txn(),
            purchase_id: doc.summary.id.clone(),
            lines: vec![PurchaseReturnLineInput { purchase_item_id: doc.items[0].id.clone(), qty: units(4) }],
            reason: Some("Leaking bottles".into()),
        },
    )
    .unwrap();
    assert_eq!(ret.total, rs(1000));
    let sup = suppliers::get(&shop.conn, &manager, &s).unwrap();
    assert_eq!(sup.balance, rs(1000 + 5000 - 2000 - 1500 - 1000));
    assert_eq!(shop.stock(&p.id).0, units(16));
    let ledger = suppliers::ledger(&shop.conn, &manager, &s).unwrap();
    assert_eq!(ledger.last().unwrap().balance, sup.balance);
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn cannot_return_more_than_purchased_or_on_hand() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Lotion", rs(500), rs(0), 0);
    let manager = shop.manager.clone();
    let doc =
        purchasing::create_purchase(&mut shop.conn, &manager, purchase(&s, vec![line(&p.id, 5, rs(300))])).unwrap();
    let ret = |qty: i64| PurchaseReturnInput {
        client_txn_id: txn(),
        purchase_id: doc.summary.id.clone(),
        lines: vec![PurchaseReturnLineInput { purchase_item_id: doc.items[0].id.clone(), qty: units(qty) }],
        reason: None,
    };
    assert!(purchasing::return_to_supplier(&mut shop.conn, &manager, ret(6)).is_err());
    shop.sell(&[(&p.id, 4)]);
    let err = purchasing::return_to_supplier(&mut shop.conn, &manager, ret(3)).unwrap_err();
    assert!(matches!(err, AppError::InsufficientStock(_)));
}

#[test]
fn duplicate_supplier_invoice_numbers_are_rejected() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Gel", rs(300), rs(0), 0);
    let manager = shop.manager.clone();
    let mut a = purchase(&s, vec![line(&p.id, 1, rs(100))]);
    a.supplier_invoice_no = Some("INV-9".into());
    purchasing::create_purchase(&mut shop.conn, &manager, a).unwrap();
    let mut b = purchase(&s, vec![line(&p.id, 1, rs(100))]);
    b.supplier_invoice_no = Some("inv-9".into());
    assert!(matches!(purchasing::create_purchase(&mut shop.conn, &manager, b), Err(AppError::Conflict(_))));
    assert_eq!(shop.stock(&p.id).0, units(1));
}

#[test]
fn purchase_is_idempotent() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Mask", rs(300), rs(0), 0);
    let manager = shop.manager.clone();
    let input = purchase(&s, vec![line(&p.id, 3, rs(100))]);
    let a = purchasing::create_purchase(&mut shop.conn, &manager, input.clone()).unwrap();
    let b = purchasing::create_purchase(&mut shop.conn, &manager, input).unwrap();
    assert_eq!(a.summary.id, b.summary.id);
    assert_eq!(shop.stock(&p.id).0, units(3));
}

#[test]
fn purchase_order_receiving_updates_status() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Hair oil", rs(400), rs(0), 0);
    let manager = shop.manager.clone();
    let po = purchasing::create_po(
        &mut shop.conn,
        &manager,
        PoInput {
            supplier_id: s.clone(),
            order_date: None,
            expected_date: Some("2026-11-10".into()),
            note: None,
            status: Some("ordered".into()),
            lines: vec![PoLineInput { product_id: p.id.clone(), qty: units(10), unit_cost: rs(200) }],
        },
    )
    .unwrap();
    assert_eq!(po.po_no, "PO-000001");
    let receive = |qty: i64| {
        let mut l = line(&p.id, qty, rs(200));
        l.po_item_id = Some(po.items[0].id.clone());
        let mut input = purchase(&s, vec![l]);
        input.po_id = Some(po.id.clone());
        input
    };
    purchasing::create_purchase(&mut shop.conn, &manager, receive(4)).unwrap();
    assert_eq!(purchasing::get_po(&shop.conn, &manager, &po.id).unwrap().status, "partial");
    purchasing::create_purchase(&mut shop.conn, &manager, receive(6)).unwrap();
    let done = purchasing::get_po(&shop.conn, &manager, &po.id).unwrap();
    assert_eq!(done.status, "received");
    assert_eq!(done.items[0].qty_received, units(10));
    assert!(purchasing::create_purchase(&mut shop.conn, &manager, receive(1)).is_err());
    assert!(purchasing::set_po_status(&mut shop.conn, &manager, &po.id, "cancelled").is_err());
}

#[test]
fn batch_tracked_products_sell_first_expiry_first() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let mut input = Shop::product_input("Vitamin C Serum", rs(1500));
    input.track_batches = true;
    let manager = shop.manager.clone();
    let p = catalog::create_product(&mut shop.conn, &manager, input).unwrap();
    let mut late = line(&p.id, 5, rs(800));
    late.batch_no = Some("B-LATE".into());
    late.expiry_date = Some("2027-12-31".into());
    let mut early = line(&p.id, 5, rs(800));
    early.batch_no = Some("B-EARLY".into());
    early.expiry_date = Some("2026-12-15".into());
    purchasing::create_purchase(&mut shop.conn, &manager, purchase(&s, vec![late, early])).unwrap();
    shop.sell(&[(&p.id, 6)]);
    let batches = inventory::list_batches(
        &shop.conn,
        &manager,
        &BatchFilter { product_id: Some(p.id.clone()), ..Default::default() },
    )
    .unwrap();
    let early_b = batches.iter().find(|b| b.batch_no.as_deref() == Some("B-EARLY")).unwrap();
    let late_b = batches.iter().find(|b| b.batch_no.as_deref() == Some("B-LATE")).unwrap();
    assert_eq!(early_b.qty_remaining, 0);
    assert_eq!(late_b.qty_remaining, units(4));
    // Within 30 days nothing with stock left expires; within 500 days the late batch does.
    assert!(codexmart_core::reports::expiring(&shop.conn, &manager, 30).unwrap().is_empty());
    let expiring = codexmart_core::reports::expiring(&shop.conn, &manager, 500).unwrap();
    assert_eq!(expiring.len(), 1);
    assert_eq!(expiring[0].batch_no.as_deref(), Some("B-LATE"));
}

#[test]
fn future_dated_purchases_are_rejected() {
    let mut shop = Shop::new();
    let s = supplier(&mut shop, 0);
    let p = shop.product("Comb", rs(100), rs(0), 0);
    let mut input = purchase(&s, vec![line(&p.id, 1, rs(50))]);
    input.business_date = Some("2026-11-02".into());
    let manager = shop.manager.clone();
    assert!(purchasing::create_purchase(&mut shop.conn, &manager, input).is_err());
}
