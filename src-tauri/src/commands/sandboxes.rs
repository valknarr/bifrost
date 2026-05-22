//! Commands operating on Sandboxie boxes that are NOT (yet) managed
//! by Bridge — discovery, adopt-into-pilot, and "this box is junk,
//! delete it" cleanup. Lifecycle of *managed* pilots lives in
//! [`super::lifecycle`].

use std::path::PathBuf;

use tauri::State;

use crate::config::BridgeConfig;
use crate::error::{BridgeError, Result};
use crate::ini;
use crate::pilot::Pilot;
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

/// Return the path Bridge would auto-detect Sandboxie-Plus at, without
/// modifying any state. Used by the Settings panel to surface the
/// resolved path even before it's persisted into the config.
#[tauri::command]
pub async fn detect_sandboxie() -> Result<Option<String>> {
    Ok(BridgeConfig::defaults().sandboxie_path)
}

/// Enumerate boxes that exist in `Sandboxie.ini` but Bridge does not
/// yet manage. Bridge-managed pilots are filtered out so the frontend
/// can show a "Discovered" section distinct from the Managed list.
///
/// Returns an empty list when Sandboxie isn't usable on this host —
/// the Inno-Setup uninstaller deliberately leaves `Sandboxie.ini`
/// behind so a reinstall keeps user configs, but the boxes inside
/// that orphaned file are not actionable (can't adopt, can't delete)
/// without an engine to drive them. Surfacing them on the Pilots
/// view confuses users who think the previous setup is still live.
#[tauri::command]
pub fn list_sandboxes(state: State<'_, AppState>) -> Result<Vec<ini::DiscoveredBox>> {
    let cfg = state.config();
    if cfg
        .sandboxie_path
        .as_deref()
        .and_then(|p| Sandboxie::at(p).ok())
        .is_none()
    {
        return Ok(Vec::new());
    }

    let all = ini::list_user_boxes()?;
    let pilots = state.pilots.lock().unwrap();
    let managed: std::collections::HashSet<String> = pilots
        .iter()
        .map(|p| p.sandbox.to_ascii_lowercase())
        .collect();
    Ok(all
        .into_iter()
        .filter(|b| !managed.contains(&b.name.to_ascii_lowercase()))
        .collect())
}

/// Promote a discovered Sandboxie box into a Bridge-managed pilot.
/// Does not modify the box itself — only adds a `Pilot` record
/// pointing at it.
#[tauri::command]
pub fn adopt_sandbox(
    state: State<'_, AppState>,
    box_name: String,
    display_name: String,
) -> Result<Pilot> {
    let cfg = state.config();
    let pilot = {
        let mut pilots = state.pilots.lock().unwrap();
        if pilots
            .iter()
            .any(|p| p.sandbox.eq_ignore_ascii_case(&box_name))
        {
            return Err(BridgeError::PilotExists(box_name));
        }
        // Display-name collision: only managed pilots count.
        if pilots
            .iter()
            .any(|p| !p.archived && p.name.eq_ignore_ascii_case(&display_name))
        {
            return Err(BridgeError::PilotExists(display_name));
        }
        let taken: Vec<String> = pilots.iter().map(|p| p.accent.clone()).collect();
        let taken_refs: Vec<&str> = taken.iter().map(|s| s.as_str()).collect();
        let mut pilot = Pilot::new(display_name, &taken_refs);
        pilot.sandbox = box_name;
        let dir = PathBuf::from(&cfg.pilots_dir).join(&pilot.id);
        pilot.browser_profile_dir = dir.to_string_lossy().into_owned();
        pilots.push(pilot.clone());
        pilot
    };
    state.save_pilots()?;
    Ok(pilot)
}

/// Permanently delete an unmanaged ("discovered") Sandboxie box.
/// Terminates any live processes inside the box first, then removes
/// the Sandboxie config section + wipes `C:\Sandbox\<user>\<box>\`.
/// Refuses to delete a box that's associated with a managed pilot —
/// the user must use the pilot's own delete flow so app state and
/// disk stay in sync.
#[tauri::command]
pub async fn delete_sandbox(state: State<'_, AppState>, box_name: String) -> Result<()> {
    {
        let pilots = state.pilots.lock().unwrap();
        if pilots
            .iter()
            .any(|p| p.sandbox.eq_ignore_ascii_case(&box_name))
        {
            return Err(BridgeError::Other(format!(
                "Box {box_name} is in use by a managed pilot — delete the pilot instead."
            )));
        }
    }

    let cfg = state.config();
    let sb_path = cfg
        .sandboxie_path
        .as_deref()
        .ok_or(BridgeError::SandboxieMissing)?;
    let sb = Sandboxie::at(sb_path)?;
    // Terminate first so nothing's holding file handles when we wipe.
    let _ = sb.terminate_box(&box_name).await;
    sb.delete_box(&box_name).await
}
