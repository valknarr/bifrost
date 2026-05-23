//! Pilot session lifecycle: start the game in its sandbox, stop it,
//! and reconcile Bifrost's in-memory view with Sandboxie's actual
//! runtime state.

use tauri::State;

use crate::error::{BridgeError, Result};
use crate::pilot::PilotStatus;
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

/// Start a pilot's session: provision the box if needed, launch the
/// EVE Frontier exe inside it. We deliberately don't auto-open the
/// browser here — the wallet flow is explicit (user clicks Wallet on
/// the pilot card) so they're never surprised by extra windows.
#[tauri::command]
pub async fn start_pilot(state: State<'_, AppState>, id: String) -> Result<()> {
    // Pull out everything we need under the lock, then drop it before
    // async work.
    let (cfg, pilot) = {
        let pilots = state.pilots.lock().unwrap();
        let pilot = pilots
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| BridgeError::PilotNotFound(id.clone()))?;
        (state.config(), pilot)
    };

    let sb_path = cfg
        .sandboxie_path
        .as_deref()
        .ok_or(BridgeError::SandboxieMissing)?;
    let sb = Sandboxie::at(sb_path)?;
    let frontier_exe = cfg
        .frontier_exe
        .as_deref()
        .ok_or_else(|| BridgeError::Config("frontier_exe not set".into()))?;

    state.set_pilot_status(&id, PilotStatus::Starting);

    // 1. Make sure the Sandboxie box is provisioned.
    sb.provision_frontier_box(&pilot.sandbox).await?;

    // 2. Launch the game into the box.
    sb.launch_in_box(&pilot.sandbox, frontier_exe, &[]).await?;

    {
        let mut pilots = state.pilots.lock().unwrap();
        if let Some(p) = pilots.iter_mut().find(|p| p.id == id) {
            p.status = PilotStatus::Running;
            p.launched_at_least_once = true;
        }
    }
    state.save_pilots()?;
    Ok(())
}

/// Stop a pilot's session. Always marks the pilot stopped — the
/// user's intent is "stop this pilot", and the common failure modes
/// of `Start.exe /terminate` are benign no-ops (box already empty,
/// helper processes already exited, box was never started this
/// session). A surfaced error in those cases would mean the user
/// gets a red toast for the no-op case where they clicked Stop on
/// an already-stopped pilot.
///
/// Real failures (Sandboxie isn't installed, the engine is wedged,
/// permission denied) are downgraded to a `tracing::warn!` so they
/// stay visible to anyone reading logs but don't trip the UI's
/// error path. The next `reconcile_pilots` tick will catch any
/// genuinely-still-running box and re-flip status.
#[tauri::command]
pub async fn stop_pilot(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let pilots = state.pilots.lock().unwrap();
        let pilot = pilots
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BridgeError::PilotNotFound(id.clone()))?;
        (state.config(), pilot.sandbox.clone())
    };

    let terminate_result = if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        match Sandboxie::at(sb_path) {
            Ok(sb) => sb.terminate_box(&sandbox).await,
            Err(e) => Err(e),
        }
    } else {
        Err(BridgeError::SandboxieMissing)
    };

    state.set_pilot_status(&id, PilotStatus::Stopped);
    if let Err(e) = &terminate_result {
        tracing::warn!(
            "stop_pilot({id}): terminate for box {sandbox} returned non-fatal error: {e}"
        );
    }
    Ok(())
}

/// Reconcile Bifrost's view with Sandboxie's actual runtime state. For
/// each pilot we ask Sandboxie whether the box has the game executable
/// running and update status accordingly. This makes Bifrost correct
/// even when sessions started outside of it (legacy .bat workflow,
/// manual SandMan launches, orphaned processes from a stop that
/// errored). Also opportunistically re-fetches on-chain balances since
/// it's hitting the network anyway.
#[tauri::command]
pub async fn reconcile_pilots(state: State<'_, AppState>) -> Result<()> {
    let cfg = state.config();
    // Snapshot includes `launched_at_least_once`: a pilot that has
    // successfully launched at least once but whose box is now gone is
    // unambiguously broken (the sandbox was nuked externally — usually
    // via Sandboxie's own "Delete Content" menu). A pilot that has
    // NEVER launched might just not have been provisioned yet, so we
    // leave those as Stopped — `start_pilot` will provision on demand.
    let pilot_snapshot: Vec<(String, String, bool)> = {
        let pilots = state.pilots.lock().unwrap();
        pilots
            .iter()
            .map(|p| (p.id.clone(), p.sandbox.clone(), p.launched_at_least_once))
            .collect()
    };

    // Query Sandboxie outside the lock — every check shells out.
    let sb = match cfg.sandboxie_path.as_deref() {
        Some(p) => Sandboxie::at(p).ok(),
        None => None,
    };

    // Identify "the game is running" by the actual game executable,
    // not just by "the box has any sandboxed process" — Sandboxie
    // keeps helper processes (SandboxieRpcSs, SandboxieDcomLaunch, …)
    // alive for a linger window after the user app exits, and we don't
    // want pilots to look ONLINE during that window.
    let game_exe_name: String = cfg
        .frontier_exe
        .as_deref()
        .and_then(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "EVE Frontier.exe".to_string());

    let mut new_statuses: Vec<(String, PilotStatus)> = Vec::new();
    if let Some(sb) = sb {
        for (id, sandbox, launched_before) in pilot_snapshot {
            // Pre-flight: if the pilot has launched before but the box
            // is now gone from Sandboxie.ini, surface as Missing so the
            // UI can offer to clean it up. Pilots that have never
            // launched yet are still in "first-launch will provision"
            // territory — don't false-positive them as missing.
            if launched_before && !crate::ini::box_section_exists(&sandbox) {
                new_statuses.push((id, PilotStatus::Missing));
                continue;
            }
            let running = sb.is_game_running(&sandbox, &game_exe_name).await;
            new_statuses.push((
                id,
                if running {
                    PilotStatus::Running
                } else {
                    PilotStatus::Stopped
                },
            ));
        }
    } else {
        // No Sandboxie — best we can do is mark everything stopped.
        for (id, _, _) in pilot_snapshot {
            new_statuses.push((id, PilotStatus::Stopped));
        }
    }

    {
        let mut pilots = state.pilots.lock().unwrap();
        for (id, status) in new_statuses {
            if let Some(p) = pilots.iter_mut().find(|p| p.id == id) {
                p.status = status;
            }
        }
    }

    // Refresh on-chain balances at the same cadence — both are
    // best-effort and we're already hitting the network.
    super::wallet::refresh_balances(&state).await;

    state.save_pilots()?;
    Ok(())
}
