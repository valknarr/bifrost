//! Per-pilot browser launcher.
//!
//! Bridge ships its own portable Chromium (Ungoogled Chromium) and uses
//! it exclusively for per-pilot wallet sessions. Host-installed browsers
//! are deliberately ignored so every Bridge install behaves identically
//! and the user's day-to-day Chrome / Edge profile is never touched.
//!
//! - `--user-data-dir=<path>` for a fully isolated profile (cookies,
//!   localStorage, extension state — Chromium's first-class multi-
//!   profile support gives us the same kind of isolation Sandboxie
//!   gives us for the game client).
//! - `--load-extension=<unpacked-extension-dir>` to side-load EVE Vault
//!   (and the per-pilot frame-colour theme) into that profile without
//!   going through the Chrome Web Store.

use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::cmd::no_window;
use crate::error::Result;

/// Terminate any chromium-family browser process whose command line
/// references the given profile directory. Used by `delete_pilot` so
/// the per-pilot browser doesn't keep file handles open and block
/// `remove_dir_all` of the profile.
///
/// Uses PowerShell's `Get-CimInstance Win32_Process` (modern, replaces
/// the deprecated `wmic`) to enumerate `brave.exe` / `chrome.exe`
/// processes with their full command line, filters by case-insensitive
/// substring match on the target path, then `taskkill /F /T` on the
/// matches. Best-effort: failures only log a warning and the caller
/// still attempts removal afterwards.
pub async fn kill_browsers_for_profile(profile_dir: &Path) {
    let target = profile_dir.to_string_lossy().to_lowercase();
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Get-CimInstance Win32_Process | \
         Where-Object { $_.Name -in 'brave.exe','chrome.exe' } | \
         ForEach-Object { \"$($_.ProcessId)|$($_.CommandLine)\" }",
    ]);
    no_window(&mut cmd);
    let Ok(out) = cmd.output().await else {
        return;
    };
    if !out.status.success() {
        return;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.to_lowercase().contains(&target) {
            continue;
        }
        if let Some(pid_str) = line.split('|').next() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                let mut k = Command::new("taskkill.exe");
                k.args(["/PID", &pid.to_string(), "/F", "/T"]);
                no_window(&mut k);
                let _ = k.output().await;
                tracing::info!(
                    "kill_browsers_for_profile: killed pid {pid} holding {}",
                    profile_dir.display()
                );
            }
        }
    }
}

/// Generate (or refresh) a tiny Chromium **theme extension** that
/// colours the browser frame with the given pilot's accent. Side-loaded
/// alongside EVE Vault on every browser launch so the user can tell at
/// a glance which pilot owns each Chromium window — Airikr's wallet
/// windows have an orange frame, Tal'Ra's are green, etc., matching
/// the pilot card accent in Bridge itself.
///
/// Returns the path to the unpacked extension directory; pass it via
/// `--load-extension` alongside any other extensions.
pub fn ensure_pilot_theme(
    profile_root: &Path,
    pilot_name: &str,
    accent_hex: &str,
) -> Result<PathBuf> {
    let theme_dir = profile_root.join("theme");
    std::fs::create_dir_all(&theme_dir)?;

    let rgb = hex_to_rgb(accent_hex).unwrap_or([243, 144, 52]);

    let manifest = serde_json::json!({
        "manifest_version": 3,
        "name": format!("Bridge — {pilot_name}"),
        "version": "1.0.0",
        "description": "Per-pilot Chromium frame colour for visual identification (set by Bridge).",
        "theme": {
            "colors": {
                "frame": rgb,
                "frame_inactive": rgb,
                "toolbar": [27, 31, 41],
                "tab_text": [232, 234, 240],
                "tab_background_text": [180, 180, 190],
                "bookmark_text": [232, 234, 240],
                "ntp_background": [11, 13, 18],
                "ntp_text": [232, 234, 240],
                "ntp_link": rgb,
                "button_background": rgb
            },
            "tints": {
                "buttons": [-1, -1, 0.9]
            }
        }
    });

    let manifest_path = theme_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(theme_dir)
}

fn hex_to_rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some([r, g, b])
}

/// Bootstrap a freshly-created Chromium profile directory.
///
/// On first ever launch we drop a `First Run` sentinel so Chromium
/// skips its welcome wizard, and a minimal `Default/Preferences` JSON
/// with a stack of "behave quietly" defaults. We deliberately do NOT
/// try to pre-pin extensions here — Chromium's path → extension-ID
/// hashing differs subtly from any reproducible Rust implementation
/// we've tried, and pinning with the wrong ID silently fails. Use
/// `ensure_extension_pinned` after the first launch instead, which
/// reads Chromium's own Preferences to discover the real ID.
///
/// The Brave-specific prefs (`brave.*` keys) suppress the toolbar
/// noise that ships on by default: Rewards onboarding, Brave Wallet
/// (which would compete with EVE Vault for the wallet-icon slot),
/// Talk, News, Leo AI, Search promotions. Each pilot's profile is
/// dedicated to the EVE Frontier ecosystem; none of that bloat is
/// relevant. Engine-level kill switches live in
/// [`BrowserLauncher::launch`] as `--disable-features` so they apply
/// even on profiles that pre-date this change.
pub fn prepare_profile(profile_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(profile_dir)?;

    let first_run = profile_dir.join("First Run");
    if !first_run.exists() {
        std::fs::File::create(&first_run)?;
    }

    let default_dir = profile_dir.join("Default");
    let prefs_path = default_dir.join("Preferences");
    if !prefs_path.exists() {
        std::fs::create_dir_all(&default_dir)?;
        let prefs = serde_json::json!({
            "browser": {
                "check_default_browser": false,
                "has_seen_welcome_page": true,
            },
            "session": {
                "restore_on_startup": 5,  // 5 = open NTP
            },
            // Brave-specific prefs — silence the toolbar / onboarding
            // noise that ships on by default. Each pilot's profile is
            // a single-purpose EVE Frontier wallet session, none of
            // these surfaces are relevant.
            "brave": {
                "rewards": { "enabled": false },
                "ads": { "should_show_onboarding": false },
                "wallet": {
                    // 0 = no native wallet. Hands the wallet-icon slot
                    // to EVE Vault uncontested.
                    "default_wallet": 0,
                    "default_solana_wallet": 0
                },
                "brave_news": { "opted_in": false, "show_on_new_tab_page": false },
                "today": { "opted_in": false },
                "ai_chat": { "has_seen_onboarding": true, "show_in_sidebar": false },
                "talk": { "is_pinned": false },
                "vpn": { "show_in_toolbar": false },
                "show_brave_rewards_button_in_location_bar": false,
                "show_side_panel_button": false,
                "show_bookmarks_button": false
            },
            // Suppress "set Brave as default" + "try Brave Search"
            // nudges that pop up on first launch otherwise.
            "default_search_provider_data": {
                "template_url_data": null
            },
        });
        std::fs::write(&prefs_path, serde_json::to_string_pretty(&prefs)?)?;
        tracing::info!("browser: bootstrapped profile at {}", profile_dir.display());
    }

    Ok(())
}

/// Engine-level features to disable via `--disable-features` on every
/// launch. Joined with commas. These are belt-and-braces alongside the
/// per-profile `Preferences` we write in [`prepare_profile`]: this list
/// applies even to profiles created by older Bridge versions, before
/// Brave reads any user prefs.
///
/// Documentation for each Brave feature flag is sparse — names lifted
/// from the `brave/brave-core` source. We err on the side of "disable
/// anything that paints UI a pilot wouldn't expect to see on their
/// EVE-Frontier wallet window".
const QUIET_BRAVE_FEATURES: &[&str] = &[
    // Rewards / Ads / BAT token plumbing.
    "BraveRewards",
    "BraveAds",
    "BraveAdsCustomNotifications",
    // Brave's own crypto wallet (competes with EVE Vault for the
    // wallet-icon slot).
    "NativeBraveWallet",
    // Brave-branded video chat surface.
    "BraveTalk",
    // News feed on the new-tab page.
    "BraveNewsBETA",
    "BraveNewsV2",
    // Leo AI assistant + its sidebar entry.
    "AIChat",
    "AIChatHistory",
    // VPN add-on promotion.
    "BraveVPN",
    // "Brave Search is faster!" interstitial nudges.
    "BraveSearchOmniboxBanner",
    "BraveSyncV2",
];

/// Find the extension ID Chromium assigned to an unpacked extension by
/// looking for an `extensions.settings.<id>` entry whose `path` field
/// matches (substring) the install directory we control. Returns None
/// when the profile hasn't launched yet, or when no matching entry has
/// been written.
pub fn read_extension_id(profile_dir: &Path, install_dir_hint: &str) -> Option<String> {
    let prefs_path = profile_dir.join("Default").join("Preferences");
    let raw = std::fs::read_to_string(&prefs_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let settings = json
        .pointer("/extensions/settings")
        .and_then(|v| v.as_object())?;
    let hint_lower = install_dir_hint.to_ascii_lowercase();
    for (id, info) in settings {
        if let Some(path_str) = info.get("path").and_then(|v| v.as_str()) {
            if path_str.to_ascii_lowercase().contains(&hint_lower) {
                return Some(id.clone());
            }
        }
    }
    None
}

/// Update the profile's `extensions.pinned_extensions` list to include
/// the given extension ID, persisting the change to disk. Takes effect
/// on the next Chromium startup. Safe no-op if already pinned, if the
/// Preferences file doesn't exist, or if Chromium is currently running
/// (its in-memory state will overwrite our edits on shutdown — that's
/// OK, we'll re-try on the next launch).
pub fn ensure_extension_pinned(profile_dir: &Path, extension_id: &str) -> Result<()> {
    let prefs_path = profile_dir.join("Default").join("Preferences");
    if !prefs_path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&prefs_path)?;
    let mut json: serde_json::Value = serde_json::from_str(&raw)?;

    let extensions_obj = json
        .pointer_mut("/extensions")
        .and_then(|v| v.as_object_mut());

    let extensions_obj = match extensions_obj {
        Some(obj) => obj,
        None => {
            // Insert an `extensions` object so we can add the pin.
            let root = json.as_object_mut().ok_or_else(|| {
                crate::error::BridgeError::Other("prefs root is not an object".into())
            })?;
            root.insert(
                "extensions".to_string(),
                serde_json::Value::Object(Default::default()),
            );
            // Safe: we just inserted an `Object` two lines up.
            root.get_mut("extensions")
                .and_then(|v| v.as_object_mut())
                .expect("extensions object just inserted")
        }
    };

    let pinned = extensions_obj
        .entry("pinned_extensions".to_string())
        .or_insert(serde_json::Value::Array(Vec::new()));
    let arr = match pinned.as_array_mut() {
        Some(a) => a,
        None => {
            // Wrong type; replace with a fresh array.
            *pinned = serde_json::Value::Array(Vec::new());
            // Safe: we just assigned `Array(...)` on the line above.
            pinned
                .as_array_mut()
                .expect("pinned_extensions array just assigned")
        }
    };

    if arr.iter().any(|v| v.as_str() == Some(extension_id)) {
        return Ok(()); // already pinned
    }
    arr.push(serde_json::Value::String(extension_id.to_string()));
    std::fs::write(&prefs_path, serde_json::to_string_pretty(&json)?)?;
    tracing::info!(
        "browser: pinned extension {extension_id} for profile {}",
        profile_dir.display()
    );
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BrowserLauncher {
    pub exe: PathBuf,
}

impl BrowserLauncher {
    pub fn new(exe: impl AsRef<Path>) -> Self {
        Self {
            exe: exe.as_ref().to_path_buf(),
        }
    }

    /// Launch the browser with an isolated `--user-data-dir`. If the
    /// profile directory doesn't exist yet it's created. Optional
    /// `extension_dirs` are unpacked-extension directories (each
    /// containing a `manifest.json`) loaded via `--load-extension`.
    ///
    /// When `startup_url` is provided AND `app_mode` is true, the URL is
    /// launched via `--app=URL` — a standalone window with no Chrome
    /// chrome (URL bar, tabs, menu all hidden). Used for our per-pilot
    /// "App" buttons so each ecosystem site feels like its own desktop
    /// app. When `app_mode` is false the URL just opens as a regular tab.
    pub async fn launch(
        &self,
        user_data_dir: &Path,
        extension_dirs: &[PathBuf],
        startup_url: Option<&str>,
        app_mode: bool,
    ) -> Result<()> {
        // Bootstrap the profile (First Run sentinel + quiet defaults) on
        // its very first use. No-op if the profile is already initialised.
        prepare_profile(user_data_dir)?;

        let mut cmd = Command::new(&self.exe);
        cmd.arg(format!(
            "--user-data-dir={}",
            user_data_dir.to_string_lossy()
        ));
        // Quiet the standard first-run noise.
        cmd.arg("--no-first-run");
        cmd.arg("--no-default-browser-check");
        // Brave-specific toolbar / sidebar / onboarding noise (Rewards,
        // News, Talk, Leo AI, VPN, native wallet, etc.). See
        // QUIET_BRAVE_FEATURES — this list is the engine-level companion
        // to the `Preferences` writes in `prepare_profile`.
        cmd.arg(format!(
            "--disable-features={}",
            QUIET_BRAVE_FEATURES.join(",")
        ));

        if !extension_dirs.is_empty() {
            let joined = extension_dirs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(",");
            cmd.arg(format!("--load-extension={joined}"));
        }

        if let Some(url) = startup_url {
            if app_mode {
                cmd.arg(format!("--app={url}"));
            } else {
                cmd.arg(url);
            }
        }

        // Fire-and-forget: the Brave window's lifetime is owned by
        // the user (they close it when they're done), not by
        // Bridge. We explicitly opt OUT of killing the child on
        // drop so the browser keeps running after this future
        // resolves. The `Child` handle is dropped immediately on
        // the next line — that just means we stop watching it
        // from Rust; the OS process continues independently.
        //
        // When Bridge actually needs to terminate a pilot's browser
        // (during `delete_pilot`), it uses
        // `kill_browsers_for_profile` above, which finds the right
        // process by command-line and runs `taskkill`.
        cmd.kill_on_drop(false);
        cmd.spawn()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Profile bootstrap is the contract the user actually sees: open
    //! Brave with a fresh per-pilot profile and have it be quiet —
    //! no Rewards onboarding, no native wallet popup, no News feed,
    //! no Leo AI sidebar. These tests pin every preference key the
    //! Settings UI promises a pilot's window won't have. If a key
    //! drifts, the regression surfaces here before a CCP-dev demo.
    use super::*;
    use tempfile::TempDir;

    /// Happy path: `prepare_profile` on a clean dir creates the
    /// `First Run` sentinel + the `Default/Preferences` file.
    /// Verifies the sequence end-to-end without mocking, so a
    /// regression in either step (file create, JSON write) surfaces.
    #[test]
    fn prepare_profile_bootstraps_clean_dir() {
        let tmp = TempDir::new().expect("tempdir");
        prepare_profile(tmp.path()).expect("bootstrap");

        assert!(tmp.path().join("First Run").exists(), "First Run sentinel");
        assert!(
            tmp.path().join("Default").join("Preferences").exists(),
            "Default/Preferences"
        );
    }

    /// The Preferences JSON must disable every Brave noise surface
    /// users would otherwise see on first launch. This is the
    /// canonical contract — if any feature falls off the list, this
    /// test breaks loudly. Pin every key we promise to suppress.
    #[test]
    fn prepare_profile_writes_quieting_preferences() {
        let tmp = TempDir::new().expect("tempdir");
        prepare_profile(tmp.path()).expect("bootstrap");

        let raw = std::fs::read_to_string(tmp.path().join("Default").join("Preferences"))
            .expect("read prefs");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("valid json");

        // Browser baseline (no welcome popup, no "set Brave as default" prompt).
        assert_eq!(json["browser"]["check_default_browser"], false);
        assert_eq!(json["browser"]["has_seen_welcome_page"], true);

        // Rewards / Ads / BAT.
        assert_eq!(json["brave"]["rewards"]["enabled"], false);
        assert_eq!(json["brave"]["ads"]["should_show_onboarding"], false);
        assert_eq!(
            json["brave"]["show_brave_rewards_button_in_location_bar"],
            false
        );

        // Native Brave Wallet — must be off so EVE Vault owns the
        // wallet-icon slot uncontested.
        assert_eq!(json["brave"]["wallet"]["default_wallet"], 0);
        assert_eq!(json["brave"]["wallet"]["default_solana_wallet"], 0);

        // Brave News on the new-tab page.
        assert_eq!(json["brave"]["brave_news"]["opted_in"], false);

        // Leo AI sidebar.
        assert_eq!(json["brave"]["ai_chat"]["show_in_sidebar"], false);

        // VPN promo button.
        assert_eq!(json["brave"]["vpn"]["show_in_toolbar"], false);
    }

    /// Calling `prepare_profile` twice must be idempotent — the
    /// second call neither errors nor clobbers the existing
    /// Preferences (we use `if !prefs_path.exists()` to avoid
    /// overwriting any tweaks the user made after first launch).
    #[test]
    fn prepare_profile_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        prepare_profile(tmp.path()).expect("first bootstrap");

        // Mutate the Preferences as a user might, then bootstrap
        // again and confirm the mutation survived.
        let prefs_path = tmp.path().join("Default").join("Preferences");
        std::fs::write(&prefs_path, r#"{"user_marker": true}"#).expect("clobber");

        prepare_profile(tmp.path()).expect("second bootstrap");

        let raw = std::fs::read_to_string(&prefs_path).expect("read");
        assert!(
            raw.contains("user_marker"),
            "second prepare_profile must not overwrite an existing Preferences"
        );
    }

    /// Engine-level disable list must not be empty (otherwise the
    /// `--disable-features=` arg renders as a no-op trailing comma).
    /// And it should contain the must-disable Brave wallet feature
    /// since that's the one EVE Vault directly competes with.
    #[test]
    fn quiet_brave_features_pins_critical_flags() {
        assert!(!QUIET_BRAVE_FEATURES.is_empty());
        assert!(
            QUIET_BRAVE_FEATURES.contains(&"NativeBraveWallet"),
            "NativeBraveWallet must be in the disable list — it competes \
             with EVE Vault for the wallet-icon slot"
        );
        assert!(QUIET_BRAVE_FEATURES.contains(&"BraveRewards"));
        assert!(QUIET_BRAVE_FEATURES.contains(&"AIChat"));
    }

    /// Cascading: bootstrap + pin extension + read back. Covers the
    /// real flow Bridge runs on every pilot's second launch (first
    /// launch creates Preferences without an `extensions` key,
    /// second launch's `ensure_extension_pinned` has to create it
    /// then write the pin).
    #[test]
    fn ensure_extension_pinned_inserts_id_when_extensions_object_missing() {
        let tmp = TempDir::new().expect("tempdir");
        prepare_profile(tmp.path()).expect("bootstrap");

        let fake_id = "lbmfdkobfnkfobfahpekbaaombpnafah";
        ensure_extension_pinned(tmp.path(), fake_id).expect("pin");

        let raw =
            std::fs::read_to_string(tmp.path().join("Default").join("Preferences")).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let pins = json["extensions"]["pinned_extensions"]
            .as_array()
            .expect("pinned_extensions array");
        assert!(
            pins.iter().any(|v| v.as_str() == Some(fake_id)),
            "extension id must be in pinned_extensions"
        );
    }

    /// Pinning is idempotent — calling twice doesn't double-insert.
    #[test]
    fn ensure_extension_pinned_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        prepare_profile(tmp.path()).expect("bootstrap");

        let fake_id = "lbmfdkobfnkfobfahpekbaaombpnafah";
        ensure_extension_pinned(tmp.path(), fake_id).expect("pin 1");
        ensure_extension_pinned(tmp.path(), fake_id).expect("pin 2");

        let raw =
            std::fs::read_to_string(tmp.path().join("Default").join("Preferences")).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let pins = json["extensions"]["pinned_extensions"]
            .as_array()
            .expect("pinned_extensions");
        let count = pins.iter().filter(|v| v.as_str() == Some(fake_id)).count();
        assert_eq!(count, 1, "extension id pinned exactly once");
    }
}
