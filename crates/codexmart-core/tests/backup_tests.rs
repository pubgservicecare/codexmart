mod common;

use std::fs;
use std::io::Write;

use codexmart_core::backup;
use codexmart_core::db::{self, migrations};
use codexmart_core::services::auth;
use common::*;

fn file_shop(dir: &std::path::Path) -> (Shop, std::path::PathBuf) {
    let path = dir.join("codexmart.db");
    let mut conn = db::open(&path).unwrap();
    migrations::migrate(&mut conn, "2026-11-01T00:00:00.000Z").unwrap();
    db::ensure_app_meta(&conn, "2026-11-01T00:00:00.000Z").unwrap();
    (Shop::with_conn(conn), path)
}

#[test]
fn backup_restores_exact_data() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let dest = dir.path().join("usb");
    let (mut shop, db_path) = file_shop(dir.path());
    let p = shop.product("Cream", rs(450), rs(300), 10);
    shop.sell(&[(&p.id, 3)]);

    let result = backup::create_backup(&shop.conn, &dest, &work, "manual", Some("Test Mart".into()), now()).unwrap();
    assert!(result.file_name.starts_with("CODEXMART_manual_"));
    assert!(result.file_name.ends_with(".cxmbak"));
    assert_eq!(result.manifest.counts["sales"], 1);
    assert_eq!(result.manifest.schema_version, migrations::latest_version());
    let check = backup::validate_file(std::path::Path::new(&result.path), &work);
    assert!(check.valid, "{}", check.message);

    // More business happens after the backup...
    shop.sell(&[(&p.id, 2)]);
    assert_eq!(shop.count("sales"), 2);

    // ...then the backup is restored over the live file.
    let validated = backup::inspect(std::path::Path::new(&result.path), &work, migrations::latest_version()).unwrap();
    drop(shop);
    backup::install(&validated, &db_path).unwrap();
    drop(validated);

    let mut conn = db::open(&db_path).unwrap();
    db::quick_check(&conn).unwrap();
    migrations::migrate(&mut conn, "2026-11-01T00:00:00.000Z").unwrap();
    let sales: i64 = conn.query_row("SELECT COUNT(*) FROM sales", [], |r| r.get(0)).unwrap();
    assert_eq!(sales, 1);
    let stock: i64 = conn.query_row("SELECT stock_qty FROM products WHERE id = ?1", [&p.id], |r| r.get(0)).unwrap();
    assert_eq!(stock, units(7));
    // Users and passwords come back with the data.
    assert!(auth::login(&mut conn, "owner", "owner-pass", now()).is_ok());
    // Temporary restore files are cleaned up.
    assert!(fs::read_dir(&work).unwrap().count() == 0);
}

#[test]
fn corrupted_backups_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let (shop, _) = file_shop(dir.path());
    let result = backup::create_backup(&shop.conn, dir.path(), &work, "manual", None, now()).unwrap();

    // Flip bytes in the middle of the archive.
    let mut bytes = fs::read(&result.path).unwrap();
    let mid = bytes.len() / 2;
    for b in &mut bytes[mid..mid + 64] {
        *b ^= 0xFF;
    }
    let damaged = dir.path().join("damaged.cxmbak");
    fs::write(&damaged, &bytes).unwrap();
    let check = backup::validate_file(&damaged, &work);
    assert!(!check.valid);

    // Truncated file.
    let truncated = dir.path().join("truncated.cxmbak");
    fs::write(&truncated, &bytes[..bytes.len() / 3]).unwrap();
    assert!(!backup::validate_file(&truncated, &work).valid);

    // Random file renamed to .cxmbak.
    let random = dir.path().join("random.cxmbak");
    fs::File::create(&random).unwrap().write_all(b"this is not a backup").unwrap();
    assert!(!backup::validate_file(&random, &work).valid);
}

#[test]
fn foreign_databases_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    // A SQLite database that is not a CODE X MART database, packed like a backup.
    let other = dir.path().join("other.db");
    {
        let c = rusqlite::Connection::open(&other).unwrap();
        c.execute_batch("CREATE TABLE notes (t TEXT); INSERT INTO notes VALUES ('hello');").unwrap();
    }
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "x").unwrap();
    db::ensure_app_meta(&conn, "x").unwrap();
    let real = backup::create_backup(&conn, dir.path(), &work, "manual", None, now()).unwrap();
    assert!(backup::validate_file(std::path::Path::new(&real.path), &work).valid);

    // Re-pack the archive with the foreign database but the original manifest.
    let manifest = {
        let f = fs::File::open(&real.path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("manifest.json").unwrap(), &mut s).unwrap();
        s
    };
    let fake = dir.path().join("fake.cxmbak");
    {
        let mut z = zip::ZipWriter::new(fs::File::create(&fake).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("manifest.json", opts).unwrap();
        z.write_all(manifest.as_bytes()).unwrap();
        z.start_file("codexmart.db", opts).unwrap();
        z.write_all(&fs::read(&other).unwrap()).unwrap();
        z.finish().unwrap();
    }
    let check = backup::validate_file(&fake, &work);
    assert!(!check.valid);
    assert!(check.message.contains("checksum"), "{}", check.message);
}

#[test]
fn backups_from_newer_versions_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "x").unwrap();
    db::ensure_app_meta(&conn, "x").unwrap();
    let real = backup::create_backup(&conn, dir.path(), &work, "manual", None, now()).unwrap();
    let result = backup::inspect(std::path::Path::new(&real.path), &work, migrations::latest_version() - 1);
    assert!(result.is_err());
}

#[test]
fn retention_only_prunes_automatic_backups() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let mut conn = db::open_in_memory().unwrap();
    migrations::migrate(&mut conn, "x").unwrap();
    db::ensure_app_meta(&conn, "x").unwrap();
    for _ in 0..5 {
        backup::create_backup(&conn, dir.path(), &work, "auto", None, now()).unwrap();
    }
    backup::create_backup(&conn, dir.path(), &work, "manual", None, now()).unwrap();
    assert_eq!(backup::list_files(dir.path()).len(), 6);
    let removed = backup::prune_auto(dir.path(), 2);
    assert_eq!(removed.len(), 3);
    let left = backup::list_files(dir.path());
    assert_eq!(left.iter().filter(|f| f.kind == "auto").count(), 2);
    assert_eq!(left.iter().filter(|f| f.kind == "manual").count(), 1);
}

#[test]
fn history_file_survives_and_reports_last_success() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("backup-history.jsonl");
    for (status, at) in [("success", "2026-11-01T08:00:00.000Z"), ("failed", "2026-11-01T09:00:00.000Z")] {
        backup::history_append(
            &file,
            &backup::HistoryEntry {
                at: at.into(),
                action: "backup".into(),
                kind: "auto".into(),
                status: status.into(),
                path: None,
                size_bytes: None,
                message: String::new(),
            },
        )
        .unwrap();
    }
    assert_eq!(backup::history_read(&file, 10).len(), 2);
    assert_eq!(backup::last_success(&file).as_deref(), Some("2026-11-01T08:00:00.000Z"));
}
