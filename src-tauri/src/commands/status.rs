//! Host-detection probe shown in the Settings panel "Detection" row +
//! Pilots view "Setup required" banner.

use serde::Serialize;
use tauri::State;

use crate::error::Result;
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

/// Top-level snapshot of what's installed on the host. The frontend
/// renders this into the Settings panel's Detection rows and the
/// Pilots view's "Setup required" banner; both decide what's missing
/// from these booleans.
///
/// The Bridge-tracked installed version (and which Sandboxie variant)
/// lives on the per-installer status structs (`SandboxieInstallerStatus`
/// etc.) — this struct stays focused on "is the binary even there?".
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub sandboxie_installed: bool,
    pub frontier_found: bool,
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<AppStatus> {
    let cfg = state.config();

    let sandboxie_installed = cfg
        .sandboxie_path
        .as_deref()
        .and_then(|p| Sandboxie::at(p).ok())
        .is_some();

    Ok(AppStatus {
        sandboxie_installed,
        frontier_found: cfg.frontier_exe.is_some(),
    })
}
