//! Get + set the persisted [`BridgeConfig`]. Most config edits today
//! flow through the more focused [`companion_sites`](super::companion_sites)
//! commands instead; these two are the catch-alls.

use tauri::State;

use crate::config::{BridgeConfig, MAX_UI_ZOOM, MIN_UI_ZOOM};
use crate::error::{BridgeError, Result};
use crate::state::AppState;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<BridgeConfig> {
    Ok(state.config())
}

#[tauri::command]
pub fn set_config(state: State<'_, AppState>, config: BridgeConfig) -> Result<()> {
    state.save_config(config)
}

/// Persist a new UI zoom factor and return the updated config. The
/// frontend mirrors this by calling `webview.setZoom(zoom)` immediately
/// so the change is visible without a restart; storing it here means
/// the next launch reads the same value back from disk.
///
/// Clamped to `MIN_UI_ZOOM..=MAX_UI_ZOOM` so a corrupted config or a
/// misbehaving caller can't blow the viewport up to 10× and trap the
/// user without a way to recover.
#[tauri::command]
pub fn set_ui_zoom(state: State<'_, AppState>, zoom: f32) -> Result<BridgeConfig> {
    if !zoom.is_finite() {
        return Err(BridgeError::Other("zoom must be a finite number".into()));
    }
    if !(MIN_UI_ZOOM..=MAX_UI_ZOOM).contains(&zoom) {
        return Err(BridgeError::Other(format!(
            "zoom {zoom} is outside the allowed range {MIN_UI_ZOOM}..{MAX_UI_ZOOM}"
        )));
    }
    let mut cfg = state.config();
    cfg.ui_zoom = zoom;
    state.save_config(cfg.clone())?;
    Ok(cfg)
}
