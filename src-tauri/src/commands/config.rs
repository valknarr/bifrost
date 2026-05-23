//! Get + set the persisted [`BridgeConfig`]. Most config edits today
//! flow through the more focused [`companion_sites`](super::companion_sites)
//! commands instead; these two are the catch-alls.

use tauri::State;

use crate::config::{
    BridgeConfig, MAX_ROSTER_WINDOW_HEIGHT, MAX_ROSTER_WINDOW_WIDTH, MAX_UI_ZOOM,
    MIN_ROSTER_WINDOW_HEIGHT, MIN_ROSTER_WINDOW_WIDTH, MIN_UI_ZOOM, VALID_ROSTER_COLUMNS,
};
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

/// Persist the user's preferred pilot-roster column count and return
/// the updated config. `0` = auto (responsive grid), `2`/`3` = explicit
/// overrides. Anything else is rejected — a corrupted config or a
/// misbehaving frontend can't land on `0` columns (invisible grid) or
/// silly values like `64` (would push the layout off-screen).
///
/// Returns the new config so the frontend can update its local copy
/// in one round-trip without a separate `get_config` call.
#[tauri::command]
pub fn set_roster_columns(state: State<'_, AppState>, columns: u8) -> Result<BridgeConfig> {
    if !VALID_ROSTER_COLUMNS.contains(&columns) {
        return Err(BridgeError::Other(format!(
            "roster_columns {columns} is not one of {VALID_ROSTER_COLUMNS:?}"
        )));
    }
    let mut cfg = state.config();
    cfg.roster_columns = columns;
    state.save_config(cfg.clone())?;
    Ok(cfg)
}

/// Persist the user's last window size while in Auto roster mode.
/// Bounds-checked against `MIN/MAX_ROSTER_WINDOW_*` so a corrupted
/// config can't trap the user with an off-screen or zero-pixel
/// window. Frontend debounces calls ~500 ms after a resize stops
/// (avoids one write per drag pixel) and only invokes this while
/// `roster_columns == 0` (the fixed presets carry their own width).
#[tauri::command]
pub fn set_roster_window_size(
    state: State<'_, AppState>,
    width: u32,
    height: u32,
) -> Result<BridgeConfig> {
    if !(MIN_ROSTER_WINDOW_WIDTH..=MAX_ROSTER_WINDOW_WIDTH).contains(&width) {
        return Err(BridgeError::Other(format!(
            "window width {width} is outside the allowed range \
             {MIN_ROSTER_WINDOW_WIDTH}..{MAX_ROSTER_WINDOW_WIDTH}"
        )));
    }
    if !(MIN_ROSTER_WINDOW_HEIGHT..=MAX_ROSTER_WINDOW_HEIGHT).contains(&height) {
        return Err(BridgeError::Other(format!(
            "window height {height} is outside the allowed range \
             {MIN_ROSTER_WINDOW_HEIGHT}..{MAX_ROSTER_WINDOW_HEIGHT}"
        )));
    }
    let mut cfg = state.config();
    cfg.roster_window_width = Some(width);
    cfg.roster_window_height = Some(height);
    state.save_config(cfg.clone())?;
    Ok(cfg)
}
