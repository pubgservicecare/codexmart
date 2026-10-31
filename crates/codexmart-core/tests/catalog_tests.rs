mod common;

use codexmart_core::error::AppError;
use codexmart_core::repo::catalog::LookupKind;
use codexmart_core::repo::products::ProductFilter;
use codexmart_core::services::catalog::{self, OpeningStock};
use codexmart_core::services::inventory;
use common::*;

#[test]
fn creates_product_with_generated_sku_and_opening_stock() {
    let mut shop = Shop::new();
    let p = shop.product("Rose Cream 50ml", rs(450), rs(300), 10);
    assert!(p.sku.starts_with("CXM-"));
    assert_eq!(p.stock_qty, units(10));
    assert_eq!(p.cost_price, Some(rs(300)));
    assert_eq!(p.stock_value, Some(rs(3000)));
    // Opening stock is recorded in the ledger.
    let n: i64 = shop
        .conn
        .query_row(
            "SELECT COUNT(*) FROM stock_movements WHERE product_id = ?1 AND movement_type = 'opening'",
            [&p.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
    assert!(inventory::verify_ledger(&shop.conn, &shop.manager).unwrap().is_empty());
}

#[test]
fn sku_and_barcode_must_be_unique_and_valid() {
    let mut shop = Shop::new();
    let mut a = Shop::product_input("Lipstick Red", rs(800));
    a.sku = Some("lip-red".into());
    a.barcode = Some("4006381333931".into());
    let created = catalog::create_product(&mut shop.conn, &shop.manager, a).unwrap();
    assert_eq!(created.sku, "LIP-RED");

    let mut dup_sku = Shop::product_input("Lipstick Pink", rs(800));
    dup_sku.sku = Some("LIP-RED".into());
    assert!(matches!(catalog::create_product(&mut shop.conn, &shop.manager, dup_sku), Err(AppError::Conflict(_))));

    let mut dup_barcode = Shop::product_input("Lipstick Pink", rs(800));
    dup_barcode.barcode = Some("4006381333931".into());
    assert!(matches!(catalog::create_product(&mut shop.conn, &shop.manager, dup_barcode), Err(AppError::Conflict(_))));

    let mut bad_check = Shop::product_input("Lipstick Pink", rs(800));
    bad_check.barcode = Some("4006381333932".into());
    assert!(matches!(catalog::create_product(&mut shop.conn, &shop.manager, bad_check), Err(AppError::Validation(_))));

    let mut bad_sku = Shop::product_input("Lipstick Pink", rs(800));
    bad_sku.sku = Some("has space".into());
    assert!(catalog::create_product(&mut shop.conn, &shop.manager, bad_sku).is_err());

    // Nothing partial was written by the failed attempts.
    assert_eq!(shop.count("products"), 1);
}

#[test]
fn generated_barcodes_are_valid_unique_ean13() {
    let mut shop = Shop::new();
    let a = catalog::generate_barcode(&mut shop.conn, &shop.manager).unwrap();
    let b = catalog::generate_barcode(&mut shop.conn, &shop.manager).unwrap();
    assert_ne!(a, b);
    assert!(codexmart_core::domain::validation::is_valid_gtin(&a));
    let mut input = Shop::product_input("Tote Bag", rs(1500));
    input.barcode = Some(a.clone());
    let p = catalog::create_product(&mut shop.conn, &shop.manager, input).unwrap();
    let found = catalog::find_by_code(&shop.conn, &shop.cashier, &a).unwrap().unwrap();
    assert_eq!(found.id, p.id);
}

#[test]
fn updating_product_keeps_stock_and_audits_price_change() {
    let mut shop = Shop::new();
    let p = shop.product("Face Wash", rs(350), rs(200), 5);
    let mut input = Shop::product_input("Face Wash 100ml", rs(399));
    input.sku = Some(p.sku.clone());
    let updated = catalog::update_product(&mut shop.conn, &shop.manager, &p.id, input).unwrap();
    assert_eq!(updated.name, "Face Wash 100ml");
    assert_eq!(updated.sale_price, rs(399));
    assert_eq!(updated.stock_qty, units(5));
    let summary: String = shop
        .conn
        .query_row("SELECT summary FROM audit_log WHERE action = 'product.updated' ORDER BY id DESC LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(summary.contains("350.00") && summary.contains("399.00"), "{summary}");
}

#[test]
fn cost_cannot_be_overwritten_while_stock_is_on_hand() {
    let mut shop = Shop::new();
    let p = shop.product("Shampoo", rs(600), rs(400), 3);
    let mut input = Shop::product_input("Shampoo", rs(600));
    input.cost_price = Some(rs(100));
    assert!(matches!(
        catalog::update_product(&mut shop.conn, &shop.manager, &p.id, input),
        Err(AppError::Validation(_))
    ));

    let empty = shop.product("Conditioner", rs(600), rs(400), 0);
    let mut input = Shop::product_input("Conditioner", rs(600));
    input.cost_price = Some(rs(350));
    let updated = catalog::update_product(&mut shop.conn, &shop.manager, &empty.id, input).unwrap();
    assert_eq!(updated.cost_price, Some(rs(350)));
}

#[test]
fn cashiers_cannot_manage_products_or_see_costs() {
    let mut shop = Shop::new();
    shop.product("Body Lotion", rs(700), rs(450), 4);
    let input = Shop::product_input("Hack", rs(1));
    let cashier = shop.cashier.clone();
    assert!(matches!(catalog::create_product(&mut shop.conn, &cashier, input), Err(AppError::Forbidden(_))));
    let list = catalog::list_products(&shop.conn, &shop.cashier, &ProductFilter::default()).unwrap();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].cost_price, None);
    assert_eq!(list.items[0].stock_value, None);
}

#[test]
fn variants_share_a_group_and_search_finds_attributes() {
    let mut shop = Shop::new();
    let mut red = Shop::product_input("Matte Lipstick", rs(900));
    red.color = Some("Red".into());
    let red = catalog::create_product(&mut shop.conn, &shop.manager, red).unwrap();
    let mut nude = Shop::product_input("Matte Lipstick", rs(900));
    nude.color = Some("Nude".into());
    nude.variant_group_id = Some(red.id.clone());
    let nude = catalog::create_product(&mut shop.conn, &shop.manager, nude).unwrap();
    assert_eq!(nude.display_name, "Matte Lipstick — Nude");
    let group = catalog::list_products(
        &shop.conn,
        &shop.manager,
        &ProductFilter { variant_group_id: Some(red.id.clone()), ..Default::default() },
    )
    .unwrap();
    assert_eq!(group.total, 2);
    let hits = catalog::search_products(&shop.conn, &shop.cashier, "lipstick nude", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, nude.id);
}

#[test]
fn decimal_units_accept_fractional_quantities_only_when_allowed() {
    let mut shop = Shop::new();
    let mut loose = Shop::product_input("Loose Rice", rs(320));
    loose.unit_id = KG.into();
    loose.opening_stock =
        Some(OpeningStock { qty: 12_500, unit_cost: Some(rs(250)), batch_no: None, expiry_date: None });
    let p = catalog::create_product(&mut shop.conn, &shop.manager, loose).unwrap();
    assert_eq!(p.stock_qty, 12_500);

    let mut pieces = Shop::product_input("Soap", rs(120));
    pieces.opening_stock =
        Some(OpeningStock { qty: 1_500, unit_cost: Some(rs(80)), batch_no: None, expiry_date: None });
    assert!(catalog::create_product(&mut shop.conn, &shop.manager, pieces).is_err());
}

#[test]
fn lookups_can_be_created_renamed_and_disabled() {
    let mut shop = Shop::new();
    let brand = catalog::create_lookup(&mut shop.conn, &shop.manager, LookupKind::Brand, "Garnier").unwrap();
    assert!(catalog::create_lookup(&mut shop.conn, &shop.manager, LookupKind::Brand, "garnier").is_err());
    let renamed =
        catalog::update_lookup(&mut shop.conn, &shop.manager, LookupKind::Brand, &brand.id, "Garnier PK", false)
            .unwrap();
    assert_eq!(renamed.name, "Garnier PK");
    assert!(!renamed.is_active);
    let active = catalog::list_lookups(&shop.conn, &shop.manager, LookupKind::Brand, false).unwrap();
    assert!(active.is_empty());
}
