//! Pilot CRUD: list / create / archive / restore / delete + the
//! per-pilot accent picker. Lifecycle (start / stop / reconcile) lives
//! in [`super::lifecycle`]; wallet flows in [`super::wallet`].

use std::path::PathBuf;

use tauri::State;

use crate::browser;
use crate::error::{BifrostError, Result};
use crate::pilot::{self, Pilot, PilotStatus};

/// Statuses that allow `delete_pilot` to bypass the "must be archived
/// first" guard. A pilot whose Sandboxie box has been deleted externally
/// is already half-broken â€” forcing the user to archive-then-delete
/// just adds friction. The archive guard exists to protect *running*
/// pilots from being nuked by an accidental click; that concern doesn't
/// apply when the sandbox is already gone.
const ARCHIVE_BYPASS_STATUSES: &[PilotStatus] = &[PilotStatus::Missing];
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

#[tauri::command]
pub fn list_pilots(state: State<'_, AppState>) -> Result<Vec<Pilot>> {
    Ok(state.pilots.lock().unwrap().clone())
}

/// Create a new managed pilot. Eagerly provisions the Sandboxie box so
/// Bifrost's view of the world matches Sandboxie's. Provisioning
/// failure is non-fatal â€” the pilot record is saved either way, and
/// the next Launch click retries.
#[tauri::command]
pub async fn create_pilot(state: State<'_, AppState>, name: String) -> Result<Pilot> {
    let cfg = state.config();

    // Build the pilot record under the lock, then drop it before any
    // async I/O (Sandboxie provisioning).
    let pilot = {
        let mut pilots = state.pilots.lock().unwrap();
        if pilots
            .iter()
            .any(|p| !p.archived && p.name.eq_ignore_ascii_case(&name))
        {
            return Err(BifrostError::PilotExists(name));
        }
        let taken: Vec<String> = pilots.iter().map(|p| p.accent.clone()).collect();
        let taken_refs: Vec<&str> = taken.iter().map(|s| s.as_str()).collect();
        let mut pilot = Pilot::new(name, &taken_refs);
        pilot.sandbox = generate_sandbox_name(&pilots);
        let dir = PathBuf::from(&cfg.pilots_dir).join(&pilot.id);
        pilot.browser_profile_dir = dir.to_string_lossy().into_owned();
        pilots.push(pilot.clone());
        pilot
    };
    state.save_pilots()?;

    // Eager provisioning. Failure is non-fatal â€” log + carry on, the
    // user can retry by clicking Launch.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.provision_frontier_box(&pilot.sandbox).await {
                tracing::warn!(
                    "provisioning {} failed: {e} (pilot saved; retry on launch)",
                    pilot.sandbox
                );
            }
        }
    }

    Ok(pilot)
}

/// Generate a unique Sandboxie box name. Format: `Bifrost<8 hex>` â€”
/// alphanumeric only (Sandboxie's name constraint), short enough to
/// fit in the UI without truncation, and prefixed so a quick glance at
/// Sandboxie-Plus's own UI tells the user which boxes Bifrost owns.
fn generate_sandbox_name(pilots: &[Pilot]) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let taken: std::collections::HashSet<String> = pilots
        .iter()
        .map(|p| p.sandbox.to_ascii_lowercase())
        .collect();

    loop {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let n = ts.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(seq);
        let candidate = format!("Bifrost{:08X}", (n & 0xFFFF_FFFF) as u32);
        if !taken.contains(&candidate.to_ascii_lowercase()) {
            return candidate;
        }
        // Collision is astronomically unlikely; if it happens, loop.
        // The counter increment guarantees progress.
    }
}

/// Move a pilot to the Archived section. Sandbox config is preserved;
/// any running processes in the box are terminated so we don't leave
/// orphans the user can't see from the UI.
#[tauri::command]
pub async fn archive_pilot(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?;
        (state.config(), p.sandbox.clone())
    };

    // Best-effort terminate. If it fails (box doesn't exist, already
    // empty) we still archive â€” the user's intent is to put this
    // pilot aside.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.terminate_box(&sandbox).await {
                tracing::warn!("archive: terminate of {sandbox} failed: {e}");
            }
        }
    }

    {
        let mut pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?;
        p.archived = true;
        p.status = PilotStatus::Stopped;
    }
    state.save_pilots()?;
    Ok(())
}

/// Restore an archived pilot back to the Managed list. Refuses if a
/// managed pilot with the same display name already exists â€” the user
/// has to rename one or the other before restoring.
#[tauri::command]
pub fn restore_pilot(state: State<'_, AppState>, id: String) -> Result<()> {
    {
        let mut pilots = state.pilots.lock().unwrap();
        // Snapshot the name we'd be restoring under, so we can check
        // collisions against other managed pilots without holding a
        // mutable borrow on the same Vec.
        let target_name = pilots
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?
            .name
            .clone();
        if pilots
            .iter()
            .any(|p| !p.archived && p.id != id && p.name.eq_ignore_ascii_case(&target_name))
        {
            return Err(BifrostError::PilotExists(target_name));
        }
        // Re-find with proper error propagation. The unwrap that
        // used to live here was technically unreachable today
        // because we hold the lock from the initial find through
        // this mutation â€” but if anyone ever inserts a yield point
        // (e.g. an async pre-check) between the two finds, a
        // concurrent delete could leave us with `None` and panic
        // the whole runtime. Propagating `PilotNotFound` is cheap
        // insurance.
        let p = pilots
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?;
        p.archived = false;
    }
    state.save_pilots()?;
    Ok(())
}

/// Change a pilot's accent colour. Used by the pen-icon picker on the
/// portrait. Accepts any 6-digit hex string with the `#` prefix. The
/// new colour drives the Bifrost UI immediately and the per-pilot
/// Chromium theme extension on next browser launch.
#[tauri::command]
pub fn set_pilot_accent(state: State<'_, AppState>, id: String, accent: String) -> Result<()> {
    let trimmed = accent.trim();
    if trimmed.len() != 7 || !trimmed.starts_with('#') {
        return Err(BifrostError::Other(
            "Accent must be a 7-character hex code like #F39034.".into(),
        ));
    }
    if !trimmed[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BifrostError::Other(
            "Accent must be a 6-digit hex code after the #.".into(),
        ));
    }
    {
        let mut pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?;
        p.accent = trimmed.to_string();
    }
    state.save_pilots()?;
    Ok(())
}

/// Return the built-in palette of accent colours so the UI's picker
/// can render the swatches without hardcoding them in two places.
#[tauri::command]
pub fn get_accent_palette() -> Vec<String> {
    pilot::PALETTE.iter().map(|s| s.to_string()).collect()
}

/// Permanently delete a pilot record AND clean up everything it owns:
/// the Sandboxie box config + data directory, and Bifrost's per-pilot
/// browser/profile/theme files. Only allowed when the pilot is
/// archived, forcing a two-step removal so accidental clicks can't
/// nuke a running pilot.
#[tauri::command]
pub async fn delete_pilot(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let mut pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::PilotNotFound(id.clone()))?;
        if !p.archived && !ARCHIVE_BYPASS_STATUSES.contains(&p.status) {
            return Err(BifrostError::Other(
                "Archive the pilot before deleting it.".into(),
            ));
        }
        let sandbox = p.sandbox.clone();
        pilots.retain(|p| p.id != id);
        (state.config(), sandbox)
    };
    state.save_pilots()?;

    // Sandboxie box: remove config + wipe data directory.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.delete_box(&sandbox).await {
                tracing::warn!("delete_pilot: clean-up of box {sandbox} failed: {e}");
            }
        }
    }

    // Per-pilot Bifrost files (browser profile, generated theme
    // extension). Brave can hold file handles into the profile dir
    // even after the visible window is closed, so terminate any
    // matching browser process for this profile first then retry the
    // delete a couple of times to give Windows time to release handles.
    //
    // Three escalating delays: the first attempt usually succeeds
    // once `taskkill /F /T` has done its work; the longer waits cover
    // the slow tail of Brave shutdown on hard-pressed machines (heavy
    // antivirus scanners, indexing services). Total worst-case wait
    // is 1.7 s â€” short enough that the delete feels responsive even
    // on the slow path.
    const HANDLE_RELEASE_DELAYS_MS: &[u64] = &[200, 500, 1000];

    let pilot_dir = PathBuf::from(&cfg.pilots_dir).join(&id);
    if pilot_dir.exists() {
        let profile_dir = pilot_dir.join("browser");
        browser::kill_browsers_for_profile(&profile_dir).await;

        let mut last_err: Option<std::io::Error> = None;
        for &delay_ms in HANDLE_RELEASE_DELAYS_MS {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            match std::fs::remove_dir_all(&pilot_dir) {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            tracing::warn!(
                "delete_pilot: could not remove pilot dir {} after retries: {e}",
                pilot_dir.display()
            );
        }
    }

    Ok(())
}
