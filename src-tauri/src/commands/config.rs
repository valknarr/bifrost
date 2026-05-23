//! Get + set the persisted [`BifrostConfig`]. Most config edits today
//! flow through the more focused [`companion_sites`](super::companion_sites)
//! commands instead; these two are the catch-alls.

use tauri::State;

use crate::config::{
    BifrostConfig, MAX_ROSTER_WINDOW_HEIGHT, MAX_ROSTER_WINDOW_WIDTH, MAX_UI_ZOOM,
    MIN_ROSTER_WINDOW_HEIGHT, MIN_ROSTER_WINDOW_WIDTH, MIN_UI_ZOOM, VALID_ROSTER_COLUMNS,
};
use crate::error::{BifrostError, Result};
use crate::state::AppState;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<BifrostConfig> {
    Ok(state.config())
}

#[tauri::command]
pub fn set_config(state: State<'_, AppState>, config: BifrostConfig) -> Result<()> {
    state.save_config(config)
}

/// Persist a new UI text-scale factor and return the updated config.
/// The frontend mirrors this by setting the `--text-scale` CSS
/// variable on `document.documentElement` immediately (see
/// `src/lib/zoom.ts::applyZoom`) so the change is visible without a
/// restart; storing it here means the next launch reads the same
/// value back from disk and re-applies it before the user sees the
/// UI.
///
/// Clamped to `MIN_UI_ZOOM..=MAX_UI_ZOOM` so a corrupted config or a
/// misbehaving caller can't blow the viewport up to 10× and trap the
/// user without a way to recover.
/// Validate a UI-zoom value. Extracted as a free function so the
/// command-boundary contract can be unit-tested without standing up
/// an `AppState` or a Tauri runtime. `set_ui_zoom` is a thin shell
/// over this helper + state mutation.
pub(crate) fn validate_ui_zoom(zoom: f32) -> Result<()> {
    if !zoom.is_finite() {
        return Err(BifrostError::Other("zoom must be a finite number".into()));
    }
    if !(MIN_UI_ZOOM..=MAX_UI_ZOOM).contains(&zoom) {
        return Err(BifrostError::Other(format!(
            "zoom {zoom} is outside the allowed range {MIN_UI_ZOOM}..{MAX_UI_ZOOM}"
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn set_ui_zoom(state: State<'_, AppState>, zoom: f32) -> Result<BifrostConfig> {
    validate_ui_zoom(zoom)?;
    let mut cfg = state.config();
    cfg.ui_zoom = zoom;
    state.save_config(cfg.clone())?;
    Ok(cfg)
}

/// Persist the user's preferred rider-roster column count and return
/// the updated config. `0` = auto (responsive grid), `2`/`3` = explicit
/// overrides. Anything else is rejected — a corrupted config or a
/// misbehaving frontend can't land on `0` columns (invisible grid) or
/// silly values like `64` (would push the layout off-screen).
///
/// Returns the new config so the frontend can update its local copy
/// in one round-trip without a separate `get_config` call.
/// Validate a roster-column count. Same extraction pattern as
/// `validate_ui_zoom` — testable as a pure function.
pub(crate) fn validate_roster_columns(columns: u8) -> Result<()> {
    if !VALID_ROSTER_COLUMNS.contains(&columns) {
        return Err(BifrostError::Other(format!(
            "roster_columns {columns} is not one of {VALID_ROSTER_COLUMNS:?}"
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn set_roster_columns(state: State<'_, AppState>, columns: u8) -> Result<BifrostConfig> {
    validate_roster_columns(columns)?;
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
/// Validate a roster-window-size pair. Same extraction pattern as
/// the other `validate_*` helpers in this module — testable as a
/// pure function.
pub(crate) fn validate_roster_window_size(width: u32, height: u32) -> Result<()> {
    if !(MIN_ROSTER_WINDOW_WIDTH..=MAX_ROSTER_WINDOW_WIDTH).contains(&width) {
        return Err(BifrostError::Other(format!(
            "window width {width} is outside the allowed range \
             {MIN_ROSTER_WINDOW_WIDTH}..{MAX_ROSTER_WINDOW_WIDTH}"
        )));
    }
    if !(MIN_ROSTER_WINDOW_HEIGHT..=MAX_ROSTER_WINDOW_HEIGHT).contains(&height) {
        return Err(BifrostError::Other(format!(
            "window height {height} is outside the allowed range \
             {MIN_ROSTER_WINDOW_HEIGHT}..{MAX_ROSTER_WINDOW_HEIGHT}"
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn set_roster_window_size(
    state: State<'_, AppState>,
    width: u32,
    height: u32,
) -> Result<BifrostConfig> {
    validate_roster_window_size(width, height)?;
    let mut cfg = state.config();
    cfg.roster_window_width = Some(width);
    cfg.roster_window_height = Some(height);
    state.save_config(cfg.clone())?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    //! Validation-boundary tests for the config setters. Each
    //! command is a thin shell over a pure `validate_*` helper that
    //! we test here. If a contributor accidentally drops the bounds
    //! check (e.g. while refactoring the command body), these tests
    //! catch the regression before it ships to users — the worst
    //! case being a viewport pinned at 100× zoom or a 0×0 window
    //! that traps the user without a way to recover.
    use super::*;

    /// `set_ui_zoom` rejects: non-finite (NaN, ±∞), below MIN,
    /// above MAX. Accepts: MIN, MAX (boundary), and a typical
    /// preset value. The bounds are pinned in
    /// `tests::ui_zoom_presets_are_within_bounds` (config.rs)
    /// against the preset values the UI exposes.
    #[test]
    fn validate_ui_zoom_rejects_invalid_values() {
        assert!(validate_ui_zoom(f32::NAN).is_err());
        assert!(validate_ui_zoom(f32::INFINITY).is_err());
        assert!(validate_ui_zoom(f32::NEG_INFINITY).is_err());
        assert!(validate_ui_zoom(0.0).is_err(), "0.0 is below MIN_UI_ZOOM");
        assert!(validate_ui_zoom(-1.0).is_err());
        assert!(
            validate_ui_zoom(MIN_UI_ZOOM - 0.01).is_err(),
            "just below MIN must be rejected"
        );
        assert!(
            validate_ui_zoom(MAX_UI_ZOOM + 0.01).is_err(),
            "just above MAX must be rejected"
        );
        assert!(
            validate_ui_zoom(100.0).is_err(),
            "100× zoom would trap the user with no Reset button"
        );
    }

    #[test]
    fn validate_ui_zoom_accepts_boundary_and_typical_values() {
        assert!(validate_ui_zoom(MIN_UI_ZOOM).is_ok(), "MIN is inclusive");
        assert!(validate_ui_zoom(MAX_UI_ZOOM).is_ok(), "MAX is inclusive");
        assert!(validate_ui_zoom(1.0).is_ok(), "Default preset");
        assert!(validate_ui_zoom(0.9).is_ok(), "Compact preset");
        assert!(validate_ui_zoom(1.15).is_ok(), "Comfortable preset");
    }

    /// `set_roster_columns` only accepts the explicit values in
    /// `VALID_ROSTER_COLUMNS` (0 = auto, 2, 3). Anything else
    /// risks rendering the roster invisible (0 cols) or off-screen
    /// (huge values).
    #[test]
    fn validate_roster_columns_accepts_only_explicit_values() {
        for &valid in VALID_ROSTER_COLUMNS {
            assert!(
                validate_roster_columns(valid).is_ok(),
                "{valid} must be accepted"
            );
        }
        // Invalid values around the edges: 1 (forgotten), 4 (would
        // need wider layout), 255 (off-screen).
        assert!(validate_roster_columns(1).is_err());
        assert!(validate_roster_columns(4).is_err());
        assert!(validate_roster_columns(64).is_err());
        assert!(validate_roster_columns(u8::MAX).is_err());
    }

    /// `set_roster_window_size` bounds-checks BOTH width AND
    /// height. Test each of the four bounds independently so a
    /// regression that drops one check is caught.
    #[test]
    fn validate_roster_window_size_rejects_out_of_bounds() {
        // Below width minimum.
        assert!(validate_roster_window_size(MIN_ROSTER_WINDOW_WIDTH - 1, 800).is_err());
        assert!(validate_roster_window_size(0, 800).is_err());
        // Above width maximum.
        assert!(validate_roster_window_size(MAX_ROSTER_WINDOW_WIDTH + 1, 800).is_err());
        assert!(validate_roster_window_size(u32::MAX, 800).is_err());
        // Below height minimum.
        assert!(validate_roster_window_size(1024, MIN_ROSTER_WINDOW_HEIGHT - 1).is_err());
        assert!(validate_roster_window_size(1024, 0).is_err());
        // Above height maximum.
        assert!(validate_roster_window_size(1024, MAX_ROSTER_WINDOW_HEIGHT + 1).is_err());
        assert!(validate_roster_window_size(1024, u32::MAX).is_err());
    }

    #[test]
    fn validate_roster_window_size_accepts_boundaries_and_typical_values() {
        // All four boundaries are inclusive.
        assert!(
            validate_roster_window_size(MIN_ROSTER_WINDOW_WIDTH, MIN_ROSTER_WINDOW_HEIGHT).is_ok()
        );
        assert!(
            validate_roster_window_size(MAX_ROSTER_WINDOW_WIDTH, MAX_ROSTER_WINDOW_HEIGHT).is_ok()
        );
        // Typical Auto-mode geometries used in production.
        assert!(validate_roster_window_size(960, 900).is_ok());
        assert!(validate_roster_window_size(800, 600).is_ok());
        assert!(validate_roster_window_size(1120, 1000).is_ok());
    }
}
