//! Status + install + uninstall commands for the three managed
//! components: Sandboxie-Plus, the portable Brave browser, and the
//! EVE Vault extension. Status calls accept an optional
//! `force_refresh` boolean which bypasses the 30-min release cache —
//! wired to the "Check for updates" buttons.
//!
//! The heavy lifting (download, verify, extract, run installer) lives
//! in each component's own module ([`crate::chromium`],
//! [`crate::evevault`], [`crate::sandboxie_installer`]); these are
//! thin orchestration wrappers.

use std::path::PathBuf;

use tauri::State;

use crate::config::BifrostConfig;
use crate::error::{BifrostError, Result};
use crate::sandboxie::Sandboxie;
use crate::state::AppState;
use crate::{chromium, evevault, sandboxie_installer};

// ---- EVE Vault extension ------------------------------------------------

/// Combined status for the EVE Vault Chromium extension: installed
/// version, latest on GitHub, etc. `force_refresh` bypasses the 30-min
/// in-process cache and re-fetches from GitHub — wired to the "Check
/// for updates" button so explicit clicks always hit the live API
/// while passive panel mounts stay cache-friendly.
#[tauri::command]
pub async fn get_evevault_status(
    state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<evevault::EveVaultStatus> {
    Ok(evevault::status(&state.app_data_dir, force_refresh.unwrap_or(false)).await)
}

/// Download + verify + extract the latest EVE Vault release into
/// Bifrost's app-data dir. Replaces any existing install in place.
///
/// Routes the release lookup through the shared `release_cache`
/// (same key as `evevault::status`) so an Install click after a
/// rate-limit returns the cached "rate-limit reached" friendly
/// error rather than burning more GitHub quota on every retry.
#[tauri::command]
pub async fn install_evevault(state: State<'_, AppState>) -> Result<()> {
    let release = crate::release_cache::fetch_with_cache(
        "evevault",
        false,
        evevault::fetch_latest_release,
    )
    .await?;
    evevault::install(&state.app_data_dir, &release).await
}

/// Remove the unpacked EVE Vault extension. Per-rider browser
/// profiles retain their extension state under their own
/// `--user-data-dir`, so uninstalling here only removes the *unpacked
/// source*; existing browser windows keep working until they reload.
#[tauri::command]
pub async fn uninstall_evevault(state: State<'_, AppState>) -> Result<()> {
    evevault::uninstall(&state.app_data_dir)
}

// ---- Portable Brave ------------------------------------------------------

/// Combined status for the portable Brave downloader. See
/// [`get_evevault_status`] for `force_refresh` semantics.
#[tauri::command]
pub async fn get_chromium_status(
    state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<chromium::ChromiumStatus> {
    Ok(chromium::status(&state.app_data_dir, force_refresh.unwrap_or(false)).await)
}

/// Download + extract the latest Brave portable build into Bifrost's
/// app-data dir. Replaces any existing install in place. ~180 MB.
///
/// Routes the release lookup through the shared `release_cache` —
/// same rationale as `install_evevault`.
#[tauri::command]
pub async fn install_chromium(state: State<'_, AppState>) -> Result<()> {
    let release = crate::release_cache::fetch_with_cache(
        "chromium",
        false,
        chromium::fetch_latest_release,
    )
    .await?;
    chromium::install(&state.app_data_dir, &release).await
}

/// Remove the portable browser install. Per-rider browser profiles
/// under `<app-data>/riders/*/browser/` are intentionally left
/// untouched — they survive a browser reinstall so wallet sessions
/// aren't blown away. The frontend confirms with the user before
/// invoking this.
#[tauri::command]
pub async fn uninstall_chromium(state: State<'_, AppState>) -> Result<()> {
    chromium::uninstall(&state.app_data_dir)
}

// ---- Sandboxie (Plus + Classic) ----------------------------------------

/// Combined status for the in-app Sandboxie installer. Pairs the
/// existing host detection (`get_status`) with the latest-release
/// lookup and the tag Bifrost last installed, so the UI can show
/// Install / Update / Reinstall actions in parity with the other two
/// components. The variant on disk (Plus vs Classic) is derived from
/// the install path so the UI can label the row accurately.
#[tauri::command]
pub async fn get_sandboxie_installer_status(
    state: State<'_, AppState>,
    force_refresh: Option<bool>,
) -> Result<sandboxie_installer::SandboxieInstallerStatus> {
    let cfg = state.config();
    let detected_variant = cfg.sandboxie_path.as_deref().and_then(|p| {
        if Sandboxie::at(p).is_ok() {
            sandboxie_installer::variant_of_install_path(std::path::Path::new(p))
        } else {
            None
        }
    });
    Ok(sandboxie_installer::status(
        &state.app_data_dir,
        detected_variant,
        force_refresh.unwrap_or(false),
    )
    .await)
}

/// Download + silently install the latest Sandboxie release for the
/// requested variant (Plus or Classic). One UAC prompt is unavoidable
/// — the installer drops a kernel driver (`SbieDrv.sys`) and a service
/// into `Program Files\Sandboxie-Plus\` (or `Sandboxie\` for Classic),
/// both of which Windows hard-requires admin elevation to write. After
/// the installer exits we re-resolve the install path so subsequent
/// `get_status` calls pick up the new bin location automatically and
/// suppress the "Program Compatibility" wizard.
#[tauri::command]
pub async fn install_sandboxie(
    state: State<'_, AppState>,
    variant: sandboxie_installer::SandboxieVariant,
) -> Result<()> {
    let release = sandboxie_installer::fetch_latest_release(variant).await?;
    sandboxie_installer::install(&state.app_data_dir, &release).await?;

    // Re-resolve the sandboxie path now that the installer has placed
    // files on disk. `BifrostConfig::defaults()` re-runs the host probe.
    let defaults = BifrostConfig::defaults();
    if let Some(path) = defaults.sandboxie_path {
        let mut cfg = state.config();
        cfg.sandboxie_path = Some(path.clone());
        state.save_config(cfg)?;

        // Preemptively silence the "Program Compatibility" wizard that
        // would otherwise pop up the first time the engine starts (i.e.
        // when the user creates their first rider). Best-effort; the
        // dialog is annoying-but-harmless if this fails.
        sandboxie_installer::suppress_compat_prompts(std::path::Path::new(&path)).await;
    }
    Ok(())
}

/// Run Sandboxie-Plus's bundled Inno-Setup uninstaller silently. Same
/// UAC requirement as installation — there's no way around the admin
/// elevation for kernel-driver removal. After the uninstaller exits we
/// also clear the cached `sandboxie_path` so the next host probe
/// correctly reports it missing. The frontend confirms with the user
/// (and warns about orphaned riders) before invoking this.
///
/// Refuses to proceed if any sandbox has live processes — the Inno
/// uninstaller can't tear down `SbieDrv.sys` while the driver is held
/// open, and a half-failed uninstall typically leaves an orphaned
/// service and a stuck driver that needs a reboot + manual cleanup. We
/// surface the busy box names so the user knows exactly which app to
/// close before retrying.
#[tauri::command]
pub async fn uninstall_sandboxie(state: State<'_, AppState>) -> Result<()> {
    let cfg = state.config();
    let root: Option<PathBuf> = cfg.sandboxie_path.as_deref().map(PathBuf::from);

    // Pre-flight: if the engine is currently usable, ask it which boxes
    // have live processes. Skipping the check when Sandboxie isn't
    // resolvable is fine — the uninstall is just a marker-scrub at that
    // point anyway (see `sandboxie_installer::uninstall`).
    if let Some(root_path) = root.as_deref() {
        if let Ok(sb) = Sandboxie::at(root_path) {
            let busy = sb.busy_box_names().await;
            if !busy.is_empty() {
                return Err(BifrostError::Other(format!(
                    "Can't uninstall Sandboxie while a sandbox is active. \
                     Close the running app{plural} (e.g. EVE Frontier launcher / game) \
                     and try again. Active sandbox{plural}: {names}.",
                    plural = if busy.len() == 1 { "" } else { "es" },
                    names = busy.join(", "),
                )));
            }
        }
    }

    sandboxie_installer::uninstall(&state.app_data_dir, root.as_deref()).await?;

    // Forget the path so subsequent `get_status` calls correctly
    // report Sandboxie as missing without restarting Bifrost.
    let mut cfg = state.config();
    cfg.sandboxie_path = None;
    state.save_config(cfg)?;
    Ok(())
}
