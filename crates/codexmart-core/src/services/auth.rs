//! First-run setup, sign-in, users and account recovery.
//!
//! Passwords are hashed with Argon2id (salted, memory-hard). Five wrong
//! passwords lock an account for five minutes. A one-time recovery key
//! (shown once at setup and stored only as a hash) lets the owner reset an
//! administrator password without losing data.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{DateTime, Duration, Utc};
use rand_core::{OsRng, RngCore};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::settings::{self, Settings};
use super::{audit, begin};
use crate::context::{timestamp, Actor, Ctx, Permission, Role};
use crate::db;
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::repo::users::{self as repo, User};

const MAX_ATTEMPTS: i64 = 5;
const LOCK_MINUTES: i64 = 5;
const RECOVERY_KEY_META: &str = "recovery_key_hash";

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Could not secure the password: {e}")))
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

fn validate_password(password: &str) -> AppResult<()> {
    let len = password.chars().count();
    if len < 4 {
        return Err(AppError::validation("Password must be at least 4 characters."));
    }
    if len > 128 {
        return Err(AppError::validation("Password is too long."));
    }
    Ok(())
}

fn validate_username(username: &str) -> AppResult<String> {
    let u = username.trim().to_lowercase();
    if u.len() < 3 || u.len() > 32 || !u.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return Err(AppError::validation(
            "Username must be 3-32 characters: letters, numbers, dot, dash or underscore.",
        ));
    }
    Ok(u)
}

fn actor_of(u: &User) -> Actor {
    Actor { user_id: u.id.clone(), username: u.username.clone(), full_name: u.full_name.clone(), role: u.role }
}

fn new_recovery_key() -> String {
    // Unambiguous alphabet (no 0/O, 1/I/L).
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 20];
    OsRng.fill_bytes(&mut bytes);
    let chars: String = bytes.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect();
    chars.as_bytes().chunks(5).map(|c| std::str::from_utf8(c).unwrap_or_default()).collect::<Vec<_>>().join("-")
}

fn normalise_key(key: &str) -> String {
    key.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}

// ------------------------------------------------------------- setup

pub fn needs_setup(conn: &Connection) -> AppResult<bool> {
    Ok(repo::count(conn)? == 0)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupInput {
    pub store_name: String,
    #[serde(default)]
    pub store_phone: Option<String>,
    #[serde(default)]
    pub store_address: Option<String>,
    #[serde(default)]
    pub currency_symbol: Option<String>,
    pub admin_full_name: String,
    pub admin_username: String,
    pub admin_password: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub actor: Actor,
    /// Shown to the owner once; only its hash is stored.
    pub recovery_key: String,
}

pub fn setup(conn: &mut Connection, input: SetupInput, now: DateTime<Utc>) -> AppResult<SetupResult> {
    let ts = timestamp(now);
    let tx = begin(conn)?;
    if repo::count(&tx)? > 0 {
        return Err(AppError::Conflict("CODE X MART has already been set up. Please sign in.".into()));
    }
    let username = validate_username(&input.admin_username)?;
    let full_name = v::required("Full name", &input.admin_full_name, 80)?;
    validate_password(&input.admin_password)?;

    let mut s = Settings { store_name: input.store_name.clone(), ..Settings::default() };
    if let Some(p) = &input.store_phone {
        s.store_phone = p.clone();
    }
    if let Some(a) = &input.store_address {
        s.store_address = a.clone();
    }
    if let Some(c) = input.currency_symbol.as_ref().filter(|c| !c.trim().is_empty()) {
        s.currency_symbol = c.clone();
    }
    settings::save_initial(&tx, &s, &ts)?;

    let id = new_id();
    repo::insert(&tx, &id, &username, &full_name, Role::Admin, &hash_password(&input.admin_password)?, &ts)?;
    let key = new_recovery_key();
    db::meta_set(&tx, RECOVERY_KEY_META, &hash_password(&normalise_key(&key))?)?;

    let actor = Actor { user_id: id.clone(), username: username.clone(), full_name, role: Role::Admin };
    let ctx = Ctx::at(actor.clone(), now, chrono::Local::now().date_naive());
    audit::record(
        &tx,
        &ctx,
        "setup.completed",
        "user",
        Some(&id),
        format!("Store set up; administrator '{username}' created"),
        None,
    )?;
    tx.commit()?;
    Ok(SetupResult { actor, recovery_key: key })
}

// ------------------------------------------------------------- sign-in

pub fn login(conn: &mut Connection, username: &str, password: &str, now: DateTime<Utc>) -> AppResult<Actor> {
    let ts = timestamp(now);
    let bad = || AppError::validation("Incorrect username or password.");
    let auth = match repo::find_auth_by_username(conn, username)? {
        Some(a) => a,
        None => {
            // Spend comparable time so usernames cannot be probed by timing.
            let _ = verify_password("$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$ZmFrZWhhc2hmYWtlaGFzaA", password);
            return Err(bad());
        }
    };
    if !auth.user.is_active {
        return Err(AppError::Forbidden("This account is disabled. Ask an administrator.".into()));
    }
    if let Some(until) = auth.locked_until.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok()) {
        if until.with_timezone(&Utc) > now {
            let mins = (until.with_timezone(&Utc) - now).num_minutes() + 1;
            return Err(AppError::Forbidden(format!("Too many wrong passwords. Try again in {mins} minute(s).")));
        }
    }
    if !verify_password(&auth.password_hash, password) {
        let attempts = auth.failed_attempts + 1;
        let lock = (attempts >= MAX_ATTEMPTS).then(|| timestamp(now + Duration::minutes(LOCK_MINUTES)));
        repo::record_failure(conn, &auth.user.id, if lock.is_some() { 0 } else { attempts }, lock.as_deref())?;
        return Err(bad());
    }
    repo::record_login(conn, &auth.user.id, &ts)?;
    let actor = actor_of(&auth.user);
    let ctx = Ctx::at(actor.clone(), now, chrono::Local::now().date_naive());
    audit::record(
        conn,
        &ctx,
        "user.login",
        "user",
        Some(&auth.user.id),
        format!("{} signed in", auth.user.username),
        None,
    )?;
    Ok(actor)
}

pub fn change_own_password(conn: &mut Connection, ctx: &Ctx, current: &str, new_password: &str) -> AppResult<()> {
    let id = super::user_id(ctx)?.to_string();
    validate_password(new_password)?;
    let tx = begin(conn)?;
    let auth = repo::get_auth(&tx, &id)?;
    if !verify_password(&auth.password_hash, current) {
        return Err(AppError::validation("Your current password is incorrect."));
    }
    repo::set_password(&tx, &id, &hash_password(new_password)?, &ctx.ts())?;
    audit::record(&tx, ctx, "user.password_changed", "user", Some(&id), "Changed own password", None)?;
    tx.commit()?;
    Ok(())
}

/// Resets an administrator's password with the recovery key.
pub fn recover_admin(
    conn: &mut Connection,
    recovery_key: &str,
    username: &str,
    new_password: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    validate_password(new_password)?;
    let tx = begin(conn)?;
    let stored = db::meta_get(&tx, RECOVERY_KEY_META)?
        .ok_or_else(|| AppError::validation("No recovery key is set up for this database."))?;
    if !verify_password(&stored, &normalise_key(recovery_key)) {
        return Err(AppError::validation("The recovery key is not correct."));
    }
    let auth = repo::find_auth_by_username(&tx, username)?
        .filter(|a| a.user.role == Role::Admin)
        .ok_or_else(|| AppError::validation("No administrator account has that username."))?;
    let ts = timestamp(now);
    repo::set_password(&tx, &auth.user.id, &hash_password(new_password)?, &ts)?;
    repo::update_profile(&tx, &auth.user.id, &auth.user.full_name, Role::Admin, true, &ts)?;
    let ctx = Ctx::at(actor_of(&auth.user), now, chrono::Local::now().date_naive());
    audit::record(
        &tx,
        &ctx,
        "user.recovered",
        "user",
        Some(&auth.user.id),
        "Administrator password reset with recovery key",
        None,
    )?;
    tx.commit()?;
    Ok(())
}

pub fn regenerate_recovery_key(conn: &mut Connection, ctx: &Ctx) -> AppResult<String> {
    ctx.require(Permission::ManageUsers)?;
    let tx = begin(conn)?;
    let key = new_recovery_key();
    db::meta_set(&tx, RECOVERY_KEY_META, &hash_password(&normalise_key(&key))?)?;
    audit::record(&tx, ctx, "security.recovery_key_regenerated", "security", None, "Recovery key regenerated", None)?;
    tx.commit()?;
    Ok(key)
}

// --------------------------------------------------------------- users

pub fn list_users(conn: &Connection, ctx: &Ctx) -> AppResult<Vec<User>> {
    ctx.require(Permission::ManageUsers)?;
    repo::list(conn)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUserInput {
    pub username: String,
    pub full_name: String,
    pub role: Role,
    pub password: String,
}

pub fn create_user(conn: &mut Connection, ctx: &Ctx, input: NewUserInput) -> AppResult<User> {
    ctx.require(Permission::ManageUsers)?;
    let username = validate_username(&input.username)?;
    let full_name = v::required("Full name", &input.full_name, 80)?;
    validate_password(&input.password)?;
    let tx = begin(conn)?;
    if repo::find_auth_by_username(&tx, &username)?.is_some() {
        return Err(AppError::Conflict("This username is already taken.".into()));
    }
    let id = new_id();
    repo::insert(&tx, &id, &username, &full_name, input.role, &hash_password(&input.password)?, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "user.created",
        "user",
        Some(&id),
        format!("User '{username}' created as {}", input.role.label()),
        Some(json!({ "role": input.role })),
    )?;
    tx.commit()?;
    repo::get(conn, &id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserInput {
    pub full_name: String,
    pub role: Role,
    pub is_active: bool,
}

pub fn update_user(conn: &mut Connection, ctx: &Ctx, id: &str, input: UpdateUserInput) -> AppResult<User> {
    ctx.require(Permission::ManageUsers)?;
    let full_name = v::required("Full name", &input.full_name, 80)?;
    let tx = begin(conn)?;
    let existing = repo::get(&tx, id)?;
    let removes_admin =
        existing.role == Role::Admin && existing.is_active && (input.role != Role::Admin || !input.is_active);
    if removes_admin && repo::count_active_admins(&tx)? <= 1 {
        return Err(AppError::validation("There must always be at least one active administrator."));
    }
    if ctx.user_id() == Some(id) && !input.is_active {
        return Err(AppError::validation("You cannot disable your own account."));
    }
    repo::update_profile(&tx, id, &full_name, input.role, input.is_active, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "user.updated",
        "user",
        Some(id),
        format!("User '{}' updated", existing.username),
        Some(json!({ "role": input.role, "active": input.is_active })),
    )?;
    tx.commit()?;
    repo::get(conn, id)
}

pub fn reset_password(conn: &mut Connection, ctx: &Ctx, id: &str, new_password: &str) -> AppResult<()> {
    ctx.require(Permission::ManageUsers)?;
    validate_password(new_password)?;
    let tx = begin(conn)?;
    let user = repo::get(&tx, id)?;
    repo::set_password(&tx, id, &hash_password(new_password)?, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "user.password_reset",
        "user",
        Some(id),
        format!("Password reset for '{}'", user.username),
        None,
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hashes_verify_and_are_salted() {
        let a = hash_password("secret1").unwrap();
        let b = hash_password("secret1").unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("$argon2id$"));
        assert!(verify_password(&a, "secret1"));
        assert!(!verify_password(&a, "secret2"));
        assert!(!verify_password("not-a-hash", "secret1"));
    }

    #[test]
    fn recovery_keys_are_formatted() {
        let k = new_recovery_key();
        assert_eq!(k.len(), 23);
        assert_eq!(normalise_key(&k.to_lowercase()).len(), 20);
    }
}
