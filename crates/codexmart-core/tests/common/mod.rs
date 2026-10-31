#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;

use codexmart_core::context::{Ctx, Role};
use codexmart_core::db::{self, migrations};
use codexmart_core::domain::payments::{PaymentMethod, Tender};
use codexmart_core::repo::products::Product;
use codexmart_core::repo::sales::SaleDetail;
use codexmart_core::services::auth::{self, NewUserInput, SetupInput};
use codexmart_core::services::catalog::{self, OpeningStock, ProductInput};
use codexmart_core::services::sales::{self, Cart, CartLine, CheckoutInput};

pub const PIECE: &str = "00000000-0000-7000-8000-000000000101";
pub const KG: &str = "00000000-0000-7000-8000-000000000106";
pub const SKINCARE: &str = "00000000-0000-7000-8000-000000000201";
pub const BAGS: &str = "00000000-0000-7000-8000-000000000203";
pub const RENT: &str = "00000000-0000-7000-8000-000000000301";
pub const ELECTRICITY: &str = "00000000-0000-7000-8000-000000000302";

pub fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 1, 9, 0, 0).unwrap()
}

pub fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
}

pub fn txn() -> String {
    codexmart_core::ids::new_id()
}

/// Rupees to paisa.
pub fn rs(rupees: i64) -> i64 {
    rupees * 100
}

/// Whole units to thousandths.
pub fn units(n: i64) -> i64 {
    n * 1000
}

pub struct Shop {
    pub conn: Connection,
    pub admin: Ctx,
    pub manager: Ctx,
    pub cashier: Ctx,
    pub recovery_key: String,
}

pub fn migrated_db() -> Connection {
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "2026-11-01T00:00:00.000Z").unwrap();
    db::ensure_app_meta(&conn, "2026-11-01T00:00:00.000Z").unwrap();
    conn
}

impl Shop {
    pub fn new() -> Self {
        Self::with_conn(migrated_db())
    }

    pub fn with_conn(mut conn: Connection) -> Self {
        let setup = auth::setup(
            &mut conn,
            SetupInput {
                store_name: "Test Mart".into(),
                store_phone: Some("0300-1234567".into()),
                store_address: None,
                currency_symbol: Some("Rs".into()),
                admin_full_name: "Owner".into(),
                admin_username: "owner".into(),
                admin_password: "owner-pass".into(),
            },
            now(),
        )
        .unwrap();
        let admin = Ctx::at(setup.actor.clone(), now(), today());
        let mut make = |username: &str, role: Role| {
            let u = auth::create_user(
                &mut conn,
                &admin,
                NewUserInput {
                    username: username.into(),
                    full_name: username.to_uppercase(),
                    role,
                    password: "pass1234".into(),
                },
            )
            .unwrap();
            let actor = auth::login(&mut conn, &u.username, "pass1234", now()).unwrap();
            Ctx::at(actor, now(), today())
        };
        let manager = make("manager", Role::Manager);
        let cashier = make("cashier", Role::Cashier);
        Shop { conn, admin, manager, cashier, recovery_key: setup.recovery_key }
    }

    pub fn product_input(name: &str, price: i64) -> ProductInput {
        ProductInput {
            name: name.into(),
            sku: None,
            barcode: None,
            category_id: Some(SKINCARE.into()),
            brand_id: None,
            unit_id: PIECE.into(),
            variant_group_id: None,
            size: None,
            color: None,
            description: None,
            sale_price: price,
            wholesale_price: None,
            cost_price: None,
            reorder_level: 0,
            track_batches: false,
            image_ref: None,
            is_active: None,
            opening_stock: None,
        }
    }

    /// Product with opening stock `qty` units at `cost` each.
    pub fn product(&mut self, name: &str, price: i64, cost: i64, qty: i64) -> Product {
        let mut input = Self::product_input(name, price);
        if qty > 0 {
            input.opening_stock =
                Some(OpeningStock { qty: units(qty), unit_cost: Some(cost), batch_no: None, expiry_date: None });
        } else {
            input.cost_price = Some(cost);
        }
        catalog::create_product(&mut self.conn, &self.manager, input).unwrap()
    }

    pub fn stock(&self, product_id: &str) -> (i64, i64, i64) {
        self.conn
            .query_row("SELECT stock_qty, stock_value, cost_price FROM products WHERE id = ?1", [product_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap()
    }

    pub fn cart(lines: &[(&str, i64)]) -> Cart {
        Cart {
            lines: lines
                .iter()
                .map(|(id, qty)| CartLine { product_id: id.to_string(), qty: units(*qty), discount: None })
                .collect(),
            ..Default::default()
        }
    }

    pub fn cash(amount: i64) -> Vec<Tender> {
        vec![Tender { method: PaymentMethod::Cash, amount, reference: None }]
    }

    pub fn sell_with(&mut self, ctx: &Ctx, cart: Cart, payments: Vec<Tender>) -> codexmart_core::AppResult<SaleDetail> {
        sales::checkout(&mut self.conn, ctx, CheckoutInput { client_txn_id: txn(), cart, payments, note: None })
    }

    /// Cash sale paid exactly.
    pub fn sell(&mut self, lines: &[(&str, i64)]) -> SaleDetail {
        let cashier = self.cashier.clone();
        let quote = sales::quote(&self.conn, &cashier, &Self::cart(lines)).unwrap();
        self.sell_with(&cashier, Self::cart(lines), Self::cash(quote.totals.total)).unwrap()
    }

    pub fn count(&self, table: &str) -> i64 {
        self.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }
}
