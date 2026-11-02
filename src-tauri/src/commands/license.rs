//! Free trial and license activation. Every command here works offline
//! and stays available when the trial or license has ended.

use codexmart_core::services::audit;
use codexmart_core::{AppError, Ctx};
use codexmart_license::OWNER_WHATSAPP_URL;
use serde_json::json;
use tauri::State;

use super::CmdResult;
use crate::licensing::{self, KeyInspection, LicenseStatus, RequestCodeInfo};
use crate::state::AppState;

#[tauri::command]
pub async fn license_status(state: State<'_, AppState>) -> CmdResult<LicenseStatus> {
    Ok(state.licensing.status())
}

#[tauri::command]
pub async fn license_request_code(state: State<'_, AppState>) -> CmdResult<RequestCodeInfo> {
    state.session().ok_or(AppError::Unauthenticated)?;
    Ok(state.licensing.request_code())
}

/// Checks an activation key's format while it is typed (no signature check).
#[tauri::command]
pub fn license_inspect_key(key: String) -> KeyInspection {
    licensing::inspect_key(&key)
}

#[tauri::command]
pub async fn license_activate(state: State<'_, AppState>, key: String) -> CmdResult<LicenseStatus> {
    let actor = state.session().ok_or(AppError::Unauthenticated)?;
    let license = state.with_conn(|c| {
        let license = state.licensing.activate(&key, Some(c))?;
        let summary = format!("{} license {} activated", license.edition.label(), license.serial());
        let details = json!({
            "serial": license.serial(),
            "edition": license.edition.code(),
            "issuedOn": license.issued_on().to_string(),
            "expiresOn": license.expires_on().map(|d| d.to_string()),
        });
        let _ =
            audit::record(c, &Ctx::new(actor.clone()), "license.activated", "license", None, summary, Some(details));
        Ok(license)
    })?;
    log::info!("{} activated license {}", actor.username, license.serial());
    Ok(state.licensing.status())
}

/// Opens a WhatsApp chat with the software owner in the default browser.
/// Without internet the browser shows an error; the number is on screen.
#[tauri::command]
pub async fn license_open_whatsapp() -> CmdResult<()> {
    std::process::Command::new("explorer").arg(OWNER_WHATSAPP_URL).spawn().map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}
