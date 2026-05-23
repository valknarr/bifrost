//! Rider CRUD: list / create / archive / restore / delete + the
//! per-rider accent picker. Lifecycle (start / stop / reconcile) lives
//! in [`super::lifecycle`]; wallet flows in [`super::wallet`].

use std::path::PathBuf;

use tauri::State;

use crate::browser;
use crate::error::{BifrostError, Result};
use crate::rider::{self, Rider, RiderStatus};
use crate::sandboxie::Sandboxie;
use crate::state::AppState;

/// Statuses that allow `delete_rider` to bypass the "must be archived
/// first" guard. A rider whose Sandboxie box has been deleted externally
/// is already half-broken — forcing the user to archive-then-delete
/// just adds friction. The archive guard exists to protect *running*
/// riders from being nuked by an accidental click; that concern doesn't
/// apply when the sandbox is already gone.
const ARCHIVE_BYPASS_STATUSES: &[RiderStatus] = &[RiderStatus::Missing];

#[tauri::command]
pub fn list_riders(state: State<'_, AppState>) -> Result<Vec<Rider>> {
    Ok(state.riders_lock().clone())
}

/// Create a new managed rider. Eagerly provisions the Sandboxie box so
/// Bifrost's view of the world matches Sandboxie's. Provisioning
/// failure is non-fatal — the rider record is saved either way, and
/// the next Launch click retries.
#[tauri::command]
pub async fn create_rider(state: State<'_, AppState>, name: String) -> Result<Rider> {
    let cfg = state.config();

    // Validation step 1: non-empty trimmed name. Without this an
    // empty / whitespace-only `name` propagates to `id = ""` and
    // `<riders_dir>/<id>` collapses to the parent — so
    // `delete_rider`'s `remove_dir_all` would wipe every rider's
    // browser profile in a single click. The FE has its own guard;
    // this is defence-in-depth at the command boundary.
    let trimmed_name = name.trim().to_string();
    if trimmed_name.is_empty() {
        return Err(BifrostError::Other("Rider name cannot be empty.".into()));
    }
    // Validation step 2: slug must contain at least one alphanumeric
    // character. Catches names like "!!!" / "---" that pass the
    // non-empty check but slugify to "".
    let slug = rider::slugify(&trimmed_name);
    if slug.is_empty() {
        return Err(BifrostError::Other(
            "Rider name must contain at least one letter or number.".into(),
        ));
    }

    // Build the rider record under the lock, then drop it before any
    // async I/O (Sandboxie provisioning).
    let rider = {
        let mut riders = state.riders_lock();
        if riders
            .iter()
            .any(|p| !p.archived && p.name.eq_ignore_ascii_case(&trimmed_name))
        {
            return Err(BifrostError::RiderExists(trimmed_name));
        }
        // Validation step 3: id uniqueness across BOTH archived and
        // managed riders. Previously a fresh "Airikr" and an
        // archived "Airikr" both got id = "airikr" and shared a
        // browser profile dir on disk — `delete_rider` on one
        // would nuke the other's data. `unique_rider_id` appends a
        // hex suffix on collision so the ids are always disjoint.
        let existing_ids: Vec<String> = riders.iter().map(|p| p.id.clone()).collect();
        let id = rider::unique_rider_id(&slug, existing_ids.iter().map(|s| s.as_str()))
            .ok_or_else(|| {
                BifrostError::Other(
                    "Rider name must contain at least one letter or number.".into(),
                )
            })?;
        let taken: Vec<String> = riders.iter().map(|p| p.accent.clone()).collect();
        let taken_refs: Vec<&str> = taken.iter().map(|s| s.as_str()).collect();
        let mut rider = Rider::new(trimmed_name, &taken_refs);
        rider.id = id;
        rider.sandbox = generate_sandbox_name(&riders);
        let dir = PathBuf::from(&cfg.riders_dir).join(&rider.id);
        rider.browser_profile_dir = dir.to_string_lossy().into_owned();
        riders.push(rider.clone());
        rider
    };
    state.save_riders()?;

    // Eager provisioning. Failure is non-fatal — log + carry on, the
    // user can retry by clicking Launch.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.provision_frontier_box(&rider.sandbox).await {
                tracing::warn!(
                    "provisioning {} failed: {e} (rider saved; retry on launch)",
                    rider.sandbox
                );
            }
        }
    }

    Ok(rider)
}

/// Generate a unique Sandboxie box name. Format: `Bifrost<8 hex>` —
/// alphanumeric only (Sandboxie's name constraint), short enough to
/// fit in the UI without truncation, and prefixed so a quick glance at
/// Sandboxie-Plus's own UI tells the user which boxes Bifrost owns.
fn generate_sandbox_name(riders: &[Rider]) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let taken: std::collections::HashSet<String> = riders
        .iter()
        .map(|p| p.sandbox.to_ascii_lowercase())
        .collect();

    // Bounded retry: collision is astronomically unlikely
    // (32-bit namespace ≈ 4 × 10⁹), but a malformed clock could
    // theoretically spin forever. 1000 iterations is well past any
    // plausible legitimate collision rate; the deterministic
    // ordinal fallback below guarantees we always return.
    for _ in 0..1000 {
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
    }
    // Deterministic last-resort ordinal. Never observed in practice;
    // present so the function signature is total rather than
    // `Result<String, ...>`. `BifrostNNNNNNNN` is still a valid
    // Sandboxie box name (alphanumeric only).
    let mut n: u32 = 0;
    loop {
        let candidate = format!("Bifrost{n:08}");
        if !taken.contains(&candidate.to_ascii_lowercase()) {
            return candidate;
        }
        n = n.wrapping_add(1);
    }
}

/// Move a rider to the Archived section. Sandbox config is preserved;
/// any running processes in the box are terminated so we don't leave
/// orphans the user can't see from the UI.
#[tauri::command]
pub async fn archive_rider(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let riders = state.riders_lock();
        let p = riders
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        (state.config(), p.sandbox.clone())
    };

    // Best-effort terminate. If it fails (box doesn't exist, already
    // empty) we still archive — the user's intent is to put this
    // rider aside.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.terminate_box(&sandbox).await {
                tracing::warn!("archive: terminate of {sandbox} failed: {e}");
            }
        }
    }

    {
        let mut riders = state.riders_lock();
        let p = riders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        p.archived = true;
        p.status = RiderStatus::Stopped;
    }
    state.save_riders()?;
    Ok(())
}

/// Restore an archived rider back to the Managed list. Refuses if a
/// managed rider with the same display name already exists — the user
/// has to rename one or the other before restoring.
#[tauri::command]
pub fn restore_rider(state: State<'_, AppState>, id: String) -> Result<()> {
    {
        let mut riders = state.riders_lock();
        // Snapshot the name we'd be restoring under, so we can check
        // collisions against other managed riders without holding a
        // mutable borrow on the same Vec.
        let target_name = riders
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?
            .name
            .clone();
        if riders
            .iter()
            .any(|p| !p.archived && p.id != id && p.name.eq_ignore_ascii_case(&target_name))
        {
            return Err(BifrostError::RiderExists(target_name));
        }
        // Re-find with proper error propagation. The unwrap that
        // used to live here was technically unreachable today
        // because we hold the lock from the initial find through
        // this mutation — but if anyone ever inserts a yield point
        // (e.g. an async pre-check) between the two finds, a
        // concurrent delete could leave us with `None` and panic
        // the whole runtime. Propagating `RiderNotFound` is cheap
        // insurance.
        let p = riders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        p.archived = false;
    }
    state.save_riders()?;
    Ok(())
}

/// Change a rider's accent colour. Used by the pen-icon picker on the
/// portrait. Accepts any 6-digit hex string with the `#` prefix. The
/// new colour drives the Bifrost UI immediately and the per-rider
/// Chromium theme extension on next browser launch.
#[tauri::command]
pub fn set_rider_accent(state: State<'_, AppState>, id: String, accent: String) -> Result<()> {
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
        let mut riders = state.riders_lock();
        let p = riders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        p.accent = trimmed.to_string();
    }
    state.save_riders()?;
    Ok(())
}

/// Return the built-in palette of accent colours so the UI's picker
/// can render the swatches without hardcoding them in two places.
#[tauri::command]
pub fn get_accent_palette() -> Vec<String> {
    rider::PALETTE.iter().map(|s| s.to_string()).collect()
}

/// Permanently delete a rider record AND clean up everything it owns:
/// the Sandboxie box config + data directory, and Bifrost's per-rider
/// browser/profile/theme files. Only allowed when the rider is
/// archived, forcing a two-step removal so accidental clicks can't
/// nuke a running rider.
#[tauri::command]
pub async fn delete_rider(state: State<'_, AppState>, id: String) -> Result<()> {
    let (cfg, sandbox) = {
        let mut riders = state.riders_lock();
        let p = riders
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        if !p.archived && !ARCHIVE_BYPASS_STATUSES.contains(&p.status) {
            return Err(BifrostError::Other(
                "Archive the rider before deleting it.".into(),
            ));
        }
        let sandbox = p.sandbox.clone();
        riders.retain(|p| p.id != id);
        (state.config(), sandbox)
    };
    state.save_riders()?;

    // Sandboxie box: remove config + wipe data directory.
    if let Some(sb_path) = cfg.sandboxie_path.as_deref() {
        if let Ok(sb) = Sandboxie::at(sb_path) {
            if let Err(e) = sb.delete_box(&sandbox).await {
                tracing::warn!("delete_rider: clean-up of box {sandbox} failed: {e}");
            }
        }
    }

    // Per-rider Bifrost files (browser profile, generated theme
    // extension). Brave can hold file handles into the profile dir
    // even after the visible window is closed, so terminate any
    // matching browser process for this profile first then retry the
    // delete a couple of times to give Windows time to release handles.
    //
    // Three escalating delays: the first attempt usually succeeds
    // once `taskkill /F /T` has done its work; the longer waits cover
    // the slow tail of Brave shutdown on hard-pressed machines (heavy
    // antivirus scanners, indexing services). Total worst-case wait
    // is 1.7 s — short enough that the delete feels responsive even
    // on the slow path.
    const HANDLE_RELEASE_DELAYS_MS: &[u64] = &[200, 500, 1000];

    let rider_dir = PathBuf::from(&cfg.riders_dir).join(&id);
    if rider_dir.exists() {
        let profile_dir = rider_dir.join("browser");
        browser::kill_browsers_for_profile(&profile_dir).await;

        let mut last_err: Option<std::io::Error> = None;
        for &delay_ms in HANDLE_RELEASE_DELAYS_MS {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            match std::fs::remove_dir_all(&rider_dir) {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            tracing::warn!(
                "delete_rider: could not remove rider dir {} after retries: {e}",
                rider_dir.display()
            );
        }
    }

    Ok(())
}
