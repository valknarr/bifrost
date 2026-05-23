//! Per-rider browser launches. These commands orchestrate three lower
//! layers: [`crate::chromium`] (find the portable Brave binary on
//! disk), [`crate::evevault`] (find the unpacked extension dir +
//! popup file), and [`crate::browser::BrowserLauncher`] (spawn the
//! Brave process with the right flags + extensions).
//!
//! Two entry points:
//! - [`open_rider_browser`] — the plain "Wallet" button on each rider
//!   card. Opens the EVE Vault popup URL directly if Bifrost already
//!   knows the extension's Chromium-assigned ID; otherwise a blank
//!   new tab.
//! - [`open_rider_app`] — the per-rider Apps row. Opens a specific
//!   URL in standalone `--app=…` mode (no Chrome chrome) so each
//!   ecosystem site feels like a desktop app.

use std::path::PathBuf;

use tauri::State;

use crate::browser::{self, BrowserLauncher};
use crate::error::{BifrostError, Result};
use crate::state::AppState;
use crate::{chromium, evevault};

/// Open this rider's browser to the EVE Vault popup (or a blank new
/// tab on first ever launch, before Chromium has resolved the
/// extension ID).
#[tauri::command]
pub async fn open_rider_browser(state: State<'_, AppState>, id: String) -> Result<()> {
    open_rider_browser_impl(state, id, None).await
}

/// Open a specific URL in this rider's browser as a standalone
/// Chromium "app" window (`--app=URL` — no URL bar / tabs / menu).
/// Same per-rider profile + EVE Vault extension as the regular
/// Wallet button, so the site receives this rider's identity via the
/// Sui Wallet Standard.
///
/// URL is checked against the configured companion-site whitelist
/// before launch. If the FE ever passes a URL that isn't in the
/// user's `companion_sites` list (UI state corruption, future
/// component bug, malicious FE injection), the launch is refused
/// rather than opening the page with the rider's wallet-bearing
/// session. Defence in depth — the FE only renders buttons for
/// configured sites today, but the backend shouldn't trust that.
#[tauri::command]
pub async fn open_rider_app(state: State<'_, AppState>, id: String, url: String) -> Result<()> {
    let cfg = state.config();
    let allowed = cfg
        .companion_sites
        .iter()
        .any(|s| s.url.eq_ignore_ascii_case(&url));
    if !allowed {
        return Err(BifrostError::Other(format!(
            "Refusing to open {url} — not in the companion-site list. \
             Add it under Settings › Companion sites first."
        )));
    }
    open_rider_browser_impl(state, id, Some(url)).await
}

/// Shared core for opening a per-rider browser window. `url = Some(…)`
/// launches in `--app=` mode (no Chrome chrome — looks like a
/// standalone desktop app). `url = None` opens a regular window.
///
/// The wallet integration is implicitly active whenever both Brave
/// and the EVE Vault extension are installed; this function errors
/// out with actionable messages if either is missing, so no explicit
/// gating flag is needed.
async fn open_rider_browser_impl(
    state: State<'_, AppState>,
    id: String,
    url: Option<String>,
) -> Result<()> {
    let cfg = state.config();
    // Bifrost owns its Chromium. No host-browser fallback — keeps the
    // experience identical across all users' machines and avoids ever
    // touching their day-to-day browser profile.
    let browser_exe_path = chromium::chrome_exe_path(&state.app_data_dir).ok_or_else(|| {
        BifrostError::Other(
            "Portable Chromium isn't installed yet. Open Settings → \
             EVE Vault Integration → Portable browser → Install."
                .into(),
        )
    })?;
    let browser_exe = browser_exe_path.as_path();

    let extension_dir = evevault::install_dir(&state.app_data_dir);
    if evevault::read_installed_version(&state.app_data_dir).is_none() {
        return Err(BifrostError::Other(
            "EVE Vault is not installed yet. Install it from Settings first.".into(),
        ));
    }

    let (profile_dir, rider_name, rider_accent, rider_root) = {
        let riders = state.riders_lock();
        let p = riders
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        // Per-rider browser profile lives alongside the rider's other
        // local state. Distinct from the Sandboxie game-side profile.
        let root = PathBuf::from(&cfg.riders_dir).join(&p.id);
        (root.join("browser"), p.name.clone(), p.accent.clone(), root)
    };

    // Generate / refresh the per-rider theme extension so the
    // Chromium window frame colour matches this rider's accent.
    // Best-effort — failures only mean Chromium uses its default
    // colours, the rest of the launch is unaffected.
    let theme_dir = browser::ensure_rider_theme(&rider_root, &rider_name, &rider_accent).ok();

    // If the EVE Vault extension has been loaded into this profile
    // before, Chromium will have written its assigned ID into the
    // Preferences file. Use that to (a) ensure the extension is
    // pinned on the next launch, and (b) build a chrome-extension://
    // popup URL for the plain Wallet button so the user lands inside
    // the wallet UI directly.
    let evevault_id = browser::read_extension_id(&profile_dir, "evevault");
    if let Some(ref id) = evevault_id {
        let _ = browser::ensure_extension_pinned(&profile_dir, id);
    }

    // Resolve URL precedence:
    //   1. Explicit URL passed in (Apps buttons)        → --app=URL
    //   2. Plain Wallet click + we know the EVE Vault
    //      ID + manifest declares a popup file         → regular tab,
    //                                                    chrome-extension:// URL
    //                                                    (--app= mode refuses
    //                                                    non-web schemes)
    //   3. First-ever launch of this profile           → no URL, blank
    //                                                    new tab page; the
    //                                                    extension auto-loads
    //                                                    and will be in Prefs
    //                                                    for next time.
    let resolved_url: Option<String> = match url {
        Some(u) => Some(u),
        None => evevault_id.as_deref().and_then(|id| {
            evevault::popup_filename(&state.app_data_dir)
                .map(|popup| format!("chrome-extension://{id}/{popup}"))
        }),
    };

    let app_mode = matches!(
        resolved_url.as_deref(),
        Some(u) if u.starts_with("http://") || u.starts_with("https://")
    );

    // Compose the extensions list. EVE Vault first (so it's the
    // "primary" one our read_extension_id hint resolves to), theme
    // extension second (purely visual).
    let mut extensions = vec![extension_dir];
    if let Some(theme) = theme_dir {
        extensions.push(theme);
    }

    let launcher = BrowserLauncher::new(browser_exe);
    launcher
        .launch(&profile_dir, &extensions, resolved_url.as_deref(), app_mode)
        .await
}
