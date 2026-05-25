//! Rider session lifecycle: start the game in its sandbox, stop it,
//! and reconcile Bifrost's in-memory view with Sandboxie's actual
//! runtime state.

use tauri::State;

use crate::error::{BifrostError, Result};
use crate::rider::RiderStatus;
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

/// Start a rider's session: provision the box if needed, launch the
/// EVE Frontier exe inside it. We deliberately don't auto-open the
/// browser here — the wallet flow is explicit (user clicks Wallet on
/// the rider card) so they're never surprised by extra windows.
#[tauri::command]
pub async fn start_rider(state: State<'_, AppState>, id: String) -> Result<()> {
    // Pull out everything we need under the lock, then drop it before
    // async work.
    let (cfg, rider) = {
        let riders = state.riders_lock();
        let rider = riders
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        (state.config(), rider)
    };

    let sb_path = cfg
        .sandboxie_path
        .as_deref()
        .ok_or(BifrostError::SandboxieMissing)?;
    let sb = Sandboxie::at(sb_path)?;
    let frontier_exe = cfg
        .frontier_exe
        .as_deref()
        .ok_or_else(|| BifrostError::Config("frontier_exe not set".into()))?;

    state.set_rider_status(&id, RiderStatus::Starting);

    // Drop-revert guard: if `provision_frontier_box` or `launch_in_box`
    // errors below, the rider was previously left wedged in `Starting`
    // until `reconcile_riders` ran (≥30 s later) and flipped it back
    // to `Stopped`. The user saw the error toast but the card was
    // still spinning. This guard catches the error path on Drop and
    // resets to `Stopped` synchronously so the card snaps back the
    // moment the toast appears.
    struct StartGuard<'a> {
        state: &'a AppState,
        id: &'a str,
        committed: bool,
    }
    impl Drop for StartGuard<'_> {
        fn drop(&mut self) {
            if !self.committed {
                self.state.set_rider_status(self.id, RiderStatus::Stopped);
            }
        }
    }
    let mut guard = StartGuard {
        state: &state,
        id: &id,
        committed: false,
    };

    // 1. Make sure the Sandboxie box is provisioned.
    sb.provision_frontier_box(&rider.sandbox).await?;

    // 2. Launch the game into the box.
    sb.launch_in_box(&rider.sandbox, frontier_exe, &[]).await?;

    {
        let mut riders = state.riders_lock();
        if let Some(p) = riders.iter_mut().find(|p| p.id == id) {
            p.status = RiderStatus::Running;
            p.launched_at_least_once = true;
        }
    }
    state.save_riders()?;
    // Mark the guard satisfied so its Drop doesn't revert the
    // happy-path Running status. Anything that errors above (the `?`
    // chains) skips this line and the Drop reverts to Stopped.
    guard.committed = true;
    Ok(())
}

/// Stop a rider's session. Always marks the rider stopped — the
/// user's intent is "stop this rider", and the common failure modes
/// of `Start.exe /terminate` are benign no-ops (box already empty,
/// helper processes already exited, box was never started this
/// session). A surfaced error in those cases would mean the user
/// gets a red toast for the no-op case where they clicked Stop on
/// an already-stopped rider.
///
/// Real failures (Sandboxie isn't installed, the engine is wedged,
/// permission denied) are downgraded to a `tracing::warn!` so they
/// stay visible to anyone reading logs but don't trip the UI's
/// error path. The next `reconcile_riders` tick will catch any
/// genuinely-still-running box and re-flip status.
#[tauri::command]
pub async fn stop_rider(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let riders = state.riders_lock();
        let rider = riders
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        (state.config(), rider.sandbox.clone())
    };

    let terminate_result = if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        match Sandboxie::at(sb_path) {
            Ok(sb) => sb.terminate_box(&sandbox).await,
            Err(e) => Err(e),
        }
    } else {
        Err(BifrostError::SandboxieMissing)
    };

    state.set_rider_status(&id, RiderStatus::Stopped);
    if let Err(e) = &terminate_result {
        tracing::warn!(
            "stop_rider({id}): terminate for box {sandbox} returned non-fatal error: {e}"
        );
    }
    Ok(())
}

/// Reconcile Bifrost's view with Sandboxie's actual runtime state. For
/// each rider we ask Sandboxie whether the box has the game executable
/// running and update status accordingly. This makes Bifrost correct
/// even when sessions started outside of it (legacy .bat workflow,
/// manual SandMan launches, orphaned processes from a stop that
/// errored). Also opportunistically re-fetches on-chain balances since
/// it's hitting the network anyway.
#[tauri::command]
pub async fn reconcile_riders(state: State<'_, AppState>) -> Result<()> {
    let cfg = state.config();
    // Snapshot includes `launched_at_least_once`: a rider that has
    // successfully launched at least once but whose box is now gone is
    // unambiguously broken (the sandbox was nuked externally — usually
    // via Sandboxie's own "Delete Content" menu). A rider that has
    // NEVER launched might just not have been provisioned yet, so we
    // leave those as Stopped — `start_rider` will provision on demand.
    let rider_snapshot: Vec<(String, String, bool)> = {
        let riders = state.riders_lock();
        riders
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
    // want riders to look ONLINE during that window.
    let game_exe_name: String = cfg
        .frontier_exe
        .as_deref()
        .and_then(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "EVE Frontier.exe".to_string());

    let mut new_statuses: Vec<(String, RiderStatus)> = Vec::new();
    if let Some(sb) = sb {
        for (id, sandbox, launched_before) in rider_snapshot {
            let box_exists = crate::ini::box_section_exists(&sandbox);
            // Only probe the game-running state when the box hasn't
            // been declared Missing — `decide_rider_status` short-
            // circuits in that case, and the Start.exe shell-out
            // is the slow part of this loop.
            let game_running = if launched_before && !box_exists {
                false
            } else {
                sb.is_game_running(&sandbox, &game_exe_name).await
            };
            new_statuses.push((
                id,
                decide_rider_status(launched_before, box_exists, game_running),
            ));
        }
    } else {
        // No Sandboxie — best we can do is mark everything stopped.
        for (id, _, _) in rider_snapshot {
            new_statuses.push((id, RiderStatus::Stopped));
        }
    }

    // Track whether anything actually changed so we can skip the
    // disk write when nothing did. Background reconcile ticks every
    // 30 s with no state drift would otherwise rewrite riders.json
    // ~2 880×/day for no reason — wasteful, and on shared/cloud
    // disks contributes to needless I/O quota usage. Mutation
    // counter increments only when we OBSERVED a different value.
    let mut status_changes: u32 = 0;
    {
        let mut riders = state.riders_lock();
        for (id, status) in new_statuses {
            if let Some(p) = riders.iter_mut().find(|p| p.id == id) {
                if p.status != status {
                    p.status = status;
                    status_changes += 1;
                }
            }
        }
    }

    // Refresh on-chain balances at the same cadence — both are
    // best-effort and we're already hitting the network. Returns
    // a "did anything land?" bool so the post-reconcile save can
    // be skipped when nothing on the rider record changed.
    let balances_changed = super::wallet::refresh_balances(&state).await;

    if status_changes > 0 || balances_changed {
        state.save_riders()?;
    }
    Ok(())
}

/// Pure decision function for `reconcile_riders`. Decoupled from the
/// async I/O so the state-machine logic can be unit-tested without
/// stubbing Sandboxie or constructing an `AppState`.
///
/// Inputs:
/// * `launched_before` — has this rider ever successfully launched
///   (i.e. has its Sandboxie box been provisioned by Bifrost at
///   least once)?
/// * `box_exists` — does the rider's `sandbox` name appear as a
///   section in `Sandboxie.ini`?
/// * `game_running` — is the EVE Frontier executable currently
///   running inside the box (per `Sandboxie::is_game_running`)?
///
/// Output: the rider's new status as reconcile sees it.
///
/// Decision table:
///
/// | launched_before | box_exists | game_running | → status   |
/// |-----------------|------------|--------------|------------|
/// | false           | *          | true         | Running    |
/// | false           | *          | false        | Stopped    |
/// | true            | true       | true         | Running    |
/// | true            | true       | false        | Stopped    |
/// | true            | false      | *            | Missing    |
///
/// Rationale: the `Missing` state only fires for riders that have
/// successfully launched before — a never-launched rider whose box
/// hasn't been provisioned yet is in "first-launch will provision"
/// territory, and false-positiving it as Missing would frustrate
/// the new-rider flow. Riders whose box has been deleted *after*
/// they've used it (e.g. via Sandboxie Plus's own Delete Content
/// menu) get the recovery banner in the UI.
pub fn decide_rider_status(
    launched_before: bool,
    box_exists: bool,
    game_running: bool,
) -> RiderStatus {
    if launched_before && !box_exists {
        return RiderStatus::Missing;
    }
    if game_running {
        RiderStatus::Running
    } else {
        RiderStatus::Stopped
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `decide_rider_status` — the load-bearing state
    //! machine for the UI's status badges and Missing-recovery
    //! banner. Every row of the decision table above is exercised
    //! here. The async `reconcile_riders` command is too coupled to
    //! `AppState` / Sandboxie to unit test directly; this pure
    //! helper covers the logic that actually matters.
    use super::*;

    /// A never-launched rider whose box hasn't been provisioned yet
    /// must NOT be flagged Missing — that path is reserved for
    /// riders that have actually run the game once. False-positives
    /// here would put a scary "Sandbox missing" banner on every
    /// fresh rider before they've even clicked Launch.
    #[test]
    fn decide_rider_status_never_launched_no_box_is_stopped_not_missing() {
        let s = decide_rider_status(false, false, false);
        assert_eq!(s, RiderStatus::Stopped);
    }

    /// Once a rider has launched at least once, a vanished box is
    /// unambiguously broken — that's the externally-deleted-box
    /// case the Missing state was designed for.
    #[test]
    fn decide_rider_status_launched_before_no_box_is_missing() {
        let s = decide_rider_status(true, false, false);
        assert_eq!(s, RiderStatus::Missing);
        // Even if game_running is somehow reported true for a
        // missing box (it shouldn't be — the caller short-circuits
        // — but be defensive), Missing wins over Running.
        let s2 = decide_rider_status(true, false, true);
        assert_eq!(
            s2,
            RiderStatus::Missing,
            "Missing takes precedence over Running when the box is gone"
        );
    }

    /// Happy path: box exists + game is running → Running.
    #[test]
    fn decide_rider_status_box_exists_game_running_is_running() {
        assert_eq!(decide_rider_status(true, true, true), RiderStatus::Running);
        assert_eq!(
            decide_rider_status(false, true, true),
            RiderStatus::Running,
            "first-launch transient: the launch flow flips \
             launched_at_least_once AFTER reconcile detects the \
             game running, so this state is reachable briefly"
        );
    }

    /// Box exists, game not running → Stopped, regardless of
    /// launched_before.
    #[test]
    fn decide_rider_status_box_exists_game_not_running_is_stopped() {
        assert_eq!(decide_rider_status(true, true, false), RiderStatus::Stopped);
        assert_eq!(
            decide_rider_status(false, true, false),
            RiderStatus::Stopped
        );
    }

    /// `launched_before` is the gate that keeps the Missing state
    /// from firing on fresh riders. Flipping that gate is the
    /// regression we most worry about — pin it explicitly.
    #[test]
    fn decide_rider_status_missing_gate_requires_launched_before() {
        // Same box-missing condition; only the launched_before flag
        // differs. False → Stopped (don't scare new users), True →
        // Missing (real broken state).
        assert_eq!(
            decide_rider_status(false, false, false),
            RiderStatus::Stopped
        );
        assert_eq!(
            decide_rider_status(true, false, false),
            RiderStatus::Missing
        );
    }
}
