use std::path::PathBuf;

use chrono::Utc;
use codexmart_core::context::{Ctx, Permission};
use codexmart_core::db::{self, migrations};
use codexmart_core::repo::audit::{AuditEntry, AuditFilter};
use codexmart_core::repo::users::User;
use codexmart_core::repo::Page;
use codexmart_core::services::auth::{self, NewUserInput, SetupInput, UpdateUserInput};
use codexmart_core::services::settings::{self, Settings};
use codexmart_core::services::{audit, inventory};
use codexmart_core::{Actor, AppError, APP_NAME, APP_VERSION};
use serde::Serialize;
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub user: Actor,
    pub permissions: Vec<Permission>,
}

impl From<Actor> for Session {
    fn from(user: Actor) -> Self {
        Session { permissions: user.permissions(), user }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub schema_version: i64,
    pub db_ready: bool,
    pub db_error: Option<String>,
    pub needs_setup: bool,
    pub session: Option<Session>,
    pub data_dir: String,
    pub db_path: String,
    pub backup_dir: String,
    pub log_dir: String,
    pub device_id: Option<String>,
}

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> CmdResult<AppStatus> {
    let db_error = state.db_error();
    let (needs_setup, schema_version, device_id) = if db_error.is_none() {
        state.with_conn(|c| {
            Ok((auth::needs_setup(c)?, migrations::current_version(c)?, db::meta_get(c, "device_id")?))
        })?
    } else {
        (false, 0, None)
    };
    Ok(AppStatus {
        app_name: APP_NAME,
        app_version: APP_VERSION,
        schema_version,
        db_ready: db_error.is_none(),
        db_error,
        needs_setup,
        session: state.session().map(Session::from),
        data_dir: state.paths.root.to_string_lossy().to_string(),
        db_path: state.paths.db_file.to_string_lossy().to_string(),
        backup_dir: state.paths.backup_dir.to_string_lossy().to_string(),
        log_dir: state.paths.log_dir.to_string_lossy().to_string(),
        device_id,
    })
}

/// Retries opening the database after a start-up failure.
#[tauri::command]
pub async fn app_retry_open(state: State<'_, AppState>) -> CmdResult<()> {
    state.retry_open()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResponse {
    pub session: Session,
    pub recovery_key: String,
}

#[tauri::command]
pub async fn app_setup(state: State<'_, AppState>, input: SetupInput) -> CmdResult<SetupResponse> {
    let result = state.with_conn(|c| auth::setup(c, input, Utc::now()))?;
    state.set_session(Some(result.actor.clone()));
    Ok(SetupResponse { session: result.actor.into(), recovery_key: result.recovery_key })
}

#[tauri::command]
pub async fn auth_login(state: State<'_, AppState>, username: String, password: String) -> CmdResult<Session> {
    let actor = state.with_conn(|c| auth::login(c, &username, &password, Utc::now()))?;
    state.set_session(Some(actor.clone()));
    Ok(actor.into())
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(actor) = state.session() {
        let ctx = Ctx::new(actor.clone());
        let _ = state.with_conn(|c| {
            audit::record(
                c,
                &ctx,
                "user.logout",
                "user",
                Some(&actor.user_id),
                format!("{} signed out", actor.username),
                None,
            )
        });
    }
    state.set_session(None);
    Ok(())
}

#[tauri::command]
pub async fn auth_change_password(state: State<'_, AppState>, current: String, new_password: String) -> CmdResult<()> {
    state.run(|c, ctx| auth::change_own_password(c, ctx, &current, &new_password))
}

#[tauri::command]
pub async fn auth_recover(
    state: State<'_, AppState>,
    recovery_key: String,
    username: String,
    new_password: String,
) -> CmdResult<()> {
    state.with_conn(|c| auth::recover_admin(c, &recovery_key, &username, &new_password, Utc::now()))
}

#[tauri::command]
pub async fn auth_regenerate_recovery_key(state: State<'_, AppState>) -> CmdResult<String> {
    state.run(auth::regenerate_recovery_key)
}

#[tauri::command]
pub async fn users_list(state: State<'_, AppState>) -> CmdResult<Vec<User>> {
    state.run(|c, ctx| auth::list_users(c, ctx))
}

#[tauri::command]
pub async fn users_create(state: State<'_, AppState>, input: NewUserInput) -> CmdResult<User> {
    state.run(|c, ctx| auth::create_user(c, ctx, input))
}

#[tauri::command]
pub async fn users_update(state: State<'_, AppState>, id: String, input: UpdateUserInput) -> CmdResult<User> {
    state.run(|c, ctx| auth::update_user(c, ctx, &id, input))
}

#[tauri::command]
pub async fn users_reset_password(state: State<'_, AppState>, id: String, new_password: String) -> CmdResult<()> {
    state.run(|c, ctx| auth::reset_password(c, ctx, &id, &new_password))
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> CmdResult<Settings> {
    state.run(|c, ctx| settings::get(c, ctx))
}

#[tauri::command]
pub async fn settings_save(state: State<'_, AppState>, input: Settings) -> CmdResult<Settings> {
    state.run(|c, ctx| settings::save(c, ctx, input))
}

#[tauri::command]
pub async fn audit_list(state: State<'_, AppState>, filter: AuditFilter) -> CmdResult<Page<AuditEntry>> {
    state.run(|c, ctx| audit::list(c, ctx, &filter))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub integrity_problems: Vec<String>,
    pub stock_mismatches: usize,
    pub schema_version: i64,
    pub db_size_bytes: u64,
}

#[tauri::command]
pub async fn app_health_check(state: State<'_, AppState>) -> CmdResult<HealthReport> {
    let size = std::fs::metadata(&state.paths.db_file).map(|m| m.len()).unwrap_or(0);
    state.run(|c, ctx| {
        ctx.require(Permission::ManageInventory)?;
        Ok(HealthReport {
            integrity_problems: db::integrity_report(c)?,
            stock_mismatches: inventory::verify_ledger(c, ctx)?.len(),
            schema_version: migrations::current_version(c)?,
            db_size_bytes: size,
        })
    })
}

/// Opens one of the app's own folders (or a chosen backup folder) in Explorer.
#[tauri::command]
pub async fn app_open_folder(state: State<'_, AppState>, kind: String, path: Option<String>) -> CmdResult<()> {
    state.session().ok_or(AppError::Unauthenticated)?;
    let dir: PathBuf = match kind.as_str() {
        "data" => state.paths.root.clone(),
        "backups" => state.paths.backup_dir.clone(),
        "logs" => state.paths.log_dir.clone(),
        "custom" => PathBuf::from(path.unwrap_or_default()),
        _ => return Err(AppError::validation("Unknown folder.")),
    };
    if !dir.is_dir() {
        return Err(AppError::validation(format!("The folder {} does not exist.", dir.display())));
    }
    std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

/// Writes a CSV export to a path the user picked in the save dialog.
#[tauri::command]
pub async fn export_text_file(state: State<'_, AppState>, path: String, contents: String) -> CmdResult<()> {
    let actor = state.session().ok_or(AppError::Unauthenticated)?;
    let p = PathBuf::from(&path);
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    if !matches!(ext.as_deref(), Some("csv") | Some("txt")) {
        return Err(AppError::validation("Exports can only be saved as .csv or .txt files."));
    }
    // UTF-8 BOM so Excel shows non-English text correctly.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(contents.as_bytes());
    std::fs::write(&p, bytes)?;
    log::info!("{} exported {}", actor.username, p.display());
    Ok(())
}

#[tauri::command]
pub fn log_client_error(message: String, detail: Option<String>) {
    log::error!("UI error: {message} {}", detail.unwrap_or_default());
}
