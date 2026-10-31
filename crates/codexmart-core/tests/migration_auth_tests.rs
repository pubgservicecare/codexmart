mod common;

use chrono::Duration;
use codexmart_core::context::Role;
use codexmart_core::db::{self, migrations};
use codexmart_core::error::AppError;
use codexmart_core::services::auth::{self, SetupInput, UpdateUserInput};
use common::*;

#[test]
fn fresh_database_migrates_to_latest_and_is_idempotent() {
    let mut conn = db::open_in_memory().unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 0);
    let report = migrations::migrate(&mut conn, "t").unwrap();
    assert_eq!(report.from_version, 0);
    assert_eq!(report.to_version, migrations::latest_version());
    let again = migrations::migrate(&mut conn, "t").unwrap();
    assert!(again.applied.is_empty());
    assert!(db::integrity_report(&conn).unwrap().is_empty());
    let units: i64 = conn.query_row("SELECT COUNT(*) FROM units", [], |r| r.get(0)).unwrap();
    assert!(units >= 5);
}

#[test]
fn upgrade_from_older_schema_keeps_existing_data() {
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate_to(&mut conn, 1, "t").unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 1);
    conn.execute(
        "INSERT INTO customers (id, name, opening_balance, is_active, created_at, updated_at)
         VALUES ('c1', 'Existing customer', 500, 1, 't', 't')",
        [],
    )
    .unwrap();
    let report = migrations::migrate(&mut conn, "t").unwrap();
    assert_eq!(report.from_version, 1);
    assert_eq!(report.applied.len() as i64, migrations::latest_version() - 1);
    let name: String = conn.query_row("SELECT name FROM customers WHERE id = 'c1'", [], |r| r.get(0)).unwrap();
    assert_eq!(name, "Existing customer");
}

#[test]
fn newer_or_tampered_schemas_are_refused_without_changes() {
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "t").unwrap();
    conn.execute("INSERT INTO schema_migrations VALUES (999, 'future', 'x', 't')", []).unwrap();
    let err = migrations::migrate(&mut conn, "t").unwrap_err();
    assert!(err.to_string().contains("newer version"), "{err}");

    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "t").unwrap();
    conn.execute("UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 1", []).unwrap();
    assert!(migrations::migrate(&mut conn, "t").is_err());
}

#[test]
fn setup_runs_once() {
    let mut shop = Shop::new();
    let again = auth::setup(
        &mut shop.conn,
        SetupInput {
            store_name: "Other".into(),
            store_phone: None,
            store_address: None,
            currency_symbol: None,
            admin_full_name: "X".into(),
            admin_username: "intruder".into(),
            admin_password: "pass1234".into(),
        },
        now(),
    );
    assert!(matches!(again, Err(AppError::Conflict(_))));
}

#[test]
fn passwords_are_hashed_and_wrong_passwords_lock_the_account() {
    let mut shop = Shop::new();
    let hash: String =
        shop.conn.query_row("SELECT password_hash FROM users WHERE username = 'owner'", [], |r| r.get(0)).unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains("owner-pass"));

    for _ in 0..5 {
        assert!(auth::login(&mut shop.conn, "cashier", "wrong", now()).is_err());
    }
    // Locked even with the right password...
    assert!(matches!(auth::login(&mut shop.conn, "cashier", "pass1234", now()), Err(AppError::Forbidden(_))));
    // ...until the lock expires.
    assert!(auth::login(&mut shop.conn, "cashier", "pass1234", now() + Duration::minutes(6)).is_ok());
    assert!(auth::login(&mut shop.conn, "nobody", "pass1234", now()).is_err());
}

#[test]
fn recovery_key_resets_admin_password() {
    let mut shop = Shop::new();
    let key = shop.recovery_key.clone();
    assert!(auth::recover_admin(&mut shop.conn, "WRONG-KEY", "owner", "new-pass", now()).is_err());
    // Not usable for non-admin accounts.
    assert!(auth::recover_admin(&mut shop.conn, &key, "cashier", "new-pass", now()).is_err());
    auth::recover_admin(&mut shop.conn, &key.to_lowercase(), "owner", "new-pass", now()).unwrap();
    assert!(auth::login(&mut shop.conn, "owner", "new-pass", now()).is_ok());
    assert!(auth::login(&mut shop.conn, "owner", "owner-pass", now()).is_err());
}

#[test]
fn last_admin_cannot_be_removed_and_roles_are_enforced() {
    let mut shop = Shop::new();
    let admin = shop.admin.clone();
    let err = auth::update_user(
        &mut shop.conn,
        &admin,
        &admin.actor.user_id.clone(),
        UpdateUserInput { full_name: "Owner".into(), role: Role::Manager, is_active: true },
    )
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
    let manager = shop.manager.clone();
    assert!(matches!(auth::list_users(&shop.conn, &manager), Err(AppError::Forbidden(_))));
    assert!(Role::Cashier.allows(codexmart_core::Permission::Sell));
    assert!(!Role::Cashier.allows(codexmart_core::Permission::VoidSale));
    assert!(!Role::Manager.allows(codexmart_core::Permission::RestoreBackup));
    assert!(Role::Admin.allows(codexmart_core::Permission::RestoreBackup));
}
