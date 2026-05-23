use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::Result;

/// One ecosystem app the user can launch into a rider's browser session.
/// "Built-in" sites ship with Bifrost and can't be removed; they can be
/// hidden (`disabled = true`) so they stop appearing in the per-rider
/// Apps row without losing the canonical URL. User-added sites are
/// fully removable and don't surface the disable toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionSite {
    /// Display name, e.g. "EF Map".
    pub name: String,
    /// Single-letter or short monogram for the icon tile (≤2 chars).
    pub icon: String,
    /// Absolute URL the site loads at.
    pub url: String,
    /// True for sites that ship with Bifrost.
    #[serde(default)]
    pub builtin: bool,
    /// True when the user has hidden this site. Hidden sites are kept
    /// in the config (so re-enabling restores the original entry) but
    /// don't render on rider cards.
    #[serde(default)]
    pub disabled: bool,
}

/// Default ecosystem sites bundled with Bifrost. Order matters — this is
/// the order they render in.
pub fn default_companion_sites() -> Vec<CompanionSite> {
    vec![
        CompanionSite {
            name: "EVE Frontier".into(),
            icon: "EF".into(),
            url: "https://evefrontier.com/".into(),
            builtin: true,
            disabled: false,
        },
        CompanionSite {
            name: "EF Map".into(),
            icon: "M".into(),
            url: "https://ef-map.com/".into(),
            builtin: true,
            disabled: false,
        },
        CompanionSite {
            name: "Civilization Control".into(),
            icon: "CC".into(),
            url: "https://civilizationcontrol.com/".into(),
            builtin: true,
            disabled: false,
        },
        CompanionSite {
            name: "Docs".into(),
            icon: "D".into(),
            url: "https://docs.evefrontier.com/".into(),
            builtin: true,
            disabled: false,
        },
        CompanionSite {
            name: "Sui Explorer".into(),
            icon: "SX".into(),
            url: "https://suiscan.xyz/mainnet/home".into(),
            builtin: true,
            disabled: false,
        },
    ]
}

/// Persisted Bifrost settings. Mirrors the TS `BifrostConfig` type.
///
/// Note: pre-v0.0.2 configs may contain a `chromeExe` field that points
/// at a host-installed Chromium browser, and pre-v0.0.3 configs may
/// contain `enableWalletIntegration`. Both are deliberately unused now —
/// Bifrost manages its own portable Chromium, and the EVE Vault
/// integration is automatically active whenever both Brave and the EVE
/// Vault extension are installed (no explicit toggle needed). Serde's
/// default unknown-field-ignore handles old configs transparently.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BifrostConfig {
    pub sandboxie_path: Option<String>,
    pub frontier_exe: Option<String>,
    pub riders_dir: String,
    pub launch_all_on_start: bool,
    /// Ordered list of ecosystem apps the user can launch into a rider's
    /// browser. Bifrost ships with a set of built-ins (see
    /// `default_companion_sites()`); users can append / remove their own.
    #[serde(default = "default_companion_sites")]
    pub companion_sites: Vec<CompanionSite>,
    /// UI text scale factor. Drives the `--text-scale` CSS variable
    /// on `:root` (see `app.css`) so rem-based type sizes scale
    /// together while pixel-based chrome (button hit-targets,
    /// gaps) stays fixed — the desktop-app shape, not the
    /// browser-zoom shape. Settings exposes three presets (0.9
    /// Compact / 1.0 Default / 1.15 Comfortable) but we store the
    /// raw float so persisted custom values from future
    /// keyboard shortcuts (Ctrl + / -) still round-trip correctly.
    /// Clamped to `MIN_UI_ZOOM..=MAX_UI_ZOOM` at the command
    /// boundary so a corrupted config can't push the viewport to
    /// absurd sizes.
    #[serde(default = "default_ui_zoom")]
    pub ui_zoom: f32,
    /// Preferred number of rider cards per row in the roster grid.
    /// `0` means "auto" (the responsive `repeat(auto-fit, minmax(...))`
    /// default — as many cards per row as the window can hold);
    /// `2` and `3` are explicit overrides surfaced in Settings ›
    /// Display so users who multibox at a fixed count can lock the
    /// layout to match.
    #[serde(default = "default_roster_columns")]
    pub roster_columns: u8,
    /// Last user-chosen window width (CSS pixels). Persisted only
    /// while the user is in Auto roster mode — the fixed presets
    /// snap the window to a known width on pick and don't need a
    /// separate memory. Frontend debounces 500 ms after a resize
    /// before calling `set_roster_window_size` so we don't write
    /// to disk on every pixel of a drag.
    #[serde(default)]
    pub roster_window_width: Option<u32>,
    /// Last user-chosen window height (CSS pixels). Saved together
    /// with `roster_window_width` so the roster reopens at the same
    /// vertical extent the user tuned for their rider count.
    #[serde(default)]
    pub roster_window_height: Option<u32>,
}

/// 0.5–2.0 covers every reasonable presbyopia-correction / hi-DPI
/// case. Higher than 2.0 starts overflowing the configured minimum
/// window width (720 px after the roster-layout work — was 960 px
/// before) in inconvenient ways; lower than 0.5 makes the UI
/// unreadable.
pub const MIN_UI_ZOOM: f32 = 0.5;
pub const MAX_UI_ZOOM: f32 = 2.0;

fn default_ui_zoom() -> f32 {
    1.0
}

/// 0 = auto (responsive `auto-fit, minmax(...)` — current behaviour).
/// Stored as a small int rather than an enum so adding more presets
/// later (4? a custom value?) doesn't require a schema migration.
fn default_roster_columns() -> u8 {
    0
}

/// Allowed values for `roster_columns`. `0` = auto (responsive),
/// `2` and `3` = explicit overrides. Validated at the command
/// boundary so a corrupted config can't render the roster invisible
/// or crash the grid layout.
pub const VALID_ROSTER_COLUMNS: &[u8] = &[0, 2, 3];

/// Sanity bounds for persisted window sizes. The lower bound matches
/// `tauri.conf.json`'s minWidth/minHeight; the upper bound is
/// generous enough for ultrawide monitors (5120-wide) without
/// allowing a corrupted/malicious config to ask Windows for a
/// 50 000 px window. Validated in `set_roster_window_size` so the
/// frontend can't accidentally write nonsense.
pub const MIN_ROSTER_WINDOW_WIDTH: u32 = 720;
pub const MAX_ROSTER_WINDOW_WIDTH: u32 = 5120;
pub const MIN_ROSTER_WINDOW_HEIGHT: u32 = 600;
pub const MAX_ROSTER_WINDOW_HEIGHT: u32 = 5120;

/// Compile-time sanity check on the zoom bounds. Catches refactors
/// that accidentally flip `MIN_UI_ZOOM > MAX_UI_ZOOM` or set
/// `MIN_UI_ZOOM <= 0` (which would multiply CSS rem sizes by zero
/// and render the UI invisible).
const _: () = {
    assert!(MIN_UI_ZOOM > 0.0);
    assert!(MAX_UI_ZOOM > MIN_UI_ZOOM);
    assert!(MAX_UI_ZOOM <= 3.0); // above 3× breaks the 960-px min window
};

impl BifrostConfig {
    pub fn defaults() -> Self {
        Self {
            sandboxie_path: detect_sandboxie_path(),
            frontier_exe: detect_frontier_exe(),
            riders_dir: default_riders_dir(),
            launch_all_on_start: false,
            companion_sites: default_companion_sites(),
            ui_zoom: default_ui_zoom(),
            roster_columns: default_roster_columns(),
            roster_window_width: None,
            roster_window_height: None,
        }
    }

    /// Load config from the app's local data dir, falling back to defaults
    /// if the file is missing OR corrupt.
    ///
    /// On a serde parse failure (corrupted JSON, partial write that
    /// somehow slipped past the atomic-write guard, version skew with
    /// an incompatible future schema), we:
    ///
    /// 1. Rename the corrupt file to `<path>.corrupt-<unix>` so the
    ///    user can inspect / restore it manually, and so a subsequent
    ///    save doesn't overwrite the evidence.
    /// 2. Return `defaults()` so the app still launches and the user
    ///    can re-configure rather than staring at an opaque startup
    ///    error they can't diagnose without devtools.
    ///
    /// This mirrors the same "graceful fallback" shape we'd want on
    /// `riders.json` (see `state::load_riders`).
    pub fn load_or_default(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::defaults());
        }
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "config: read failed at {}: {e} - falling back to defaults",
                    path.display()
                );
                return Ok(Self::defaults());
            }
        };
        match serde_json::from_str::<Self>(&raw) {
            Ok(parsed) => Ok(parsed),
            Err(e) => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = path.with_extension(format!("corrupt-{ts}.json"));
                tracing::warn!(
                    "config: parse failed at {}: {e} - moved to {} and reset to defaults",
                    path.display(),
                    backup.display()
                );
                let _ = std::fs::rename(path, &backup);
                Ok(Self::defaults())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        // config.json holds companion sites + persisted Auto-mode
        // window size + ui-zoom + roster columns — corruption here
        // silently resets the user back to defaults on next launch
        // and they have no signal anything went wrong. Atomic write
        // (tmp + rename) makes a crash mid-write a no-op rather
        // than a silent data loss.
        let json = serde_json::to_string_pretty(self)?;
        crate::atomic_write::write_atomic(path, json.as_bytes())
    }
}

fn detect_sandboxie_path() -> Option<String> {
    let candidates = [
        r"C:\Program Files\Sandboxie-Plus",
        r"C:\Program Files (x86)\Sandboxie-Plus",
        r"C:\Program Files\Sandboxie",
    ];
    candidates
        .iter()
        .find(|p| PathBuf::from(p).join("SbieIni.exe").exists())
        .map(|p| (*p).to_string())
}

fn detect_frontier_exe() -> Option<String> {
    let localappdata = std::env::var("LOCALAPPDATA").ok()?;
    let candidate = PathBuf::from(&localappdata)
        .join("eve-frontier")
        .join("EVE Frontier.exe");
    candidate
        .exists()
        .then(|| candidate.to_string_lossy().into_owned())
}

fn default_riders_dir() -> String {
    dirs::data_local_dir()
        .map(|d| d.join("Bifrost").join("riders"))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from(r"C:\BifrostRiders"))
}

#[cfg(test)]
mod tests {
    //! Cascading config tests — they exercise the
    //! `defaults() → save() → load_or_default()` round-trip end to
    //! end so a regression in serde derives, file I/O, or the
    //! companion-site invariants surfaces here instead of when a
    //! user's saved settings stop loading.
    use super::*;
    use tempfile::TempDir;

    /// Every built-in companion site is shipped with the right
    /// invariants set: `builtin = true`, `disabled = false`, non-empty
    /// name + URL + icon. Catches future "I added a new built-in but
    /// forgot the disabled flag" regressions.
    #[test]
    fn default_companion_sites_invariants_hold() {
        let sites = default_companion_sites();
        assert!(!sites.is_empty(), "ship at least one default");
        for s in &sites {
            assert!(s.builtin, "{} should be builtin", s.name);
            assert!(!s.disabled, "{} ships enabled", s.name);
            assert!(!s.name.is_empty());
            assert!(!s.url.is_empty());
            assert!(!s.icon.is_empty());
            assert!(
                s.url.starts_with("http://") || s.url.starts_with("https://"),
                "{} URL must be http/https: {}",
                s.name,
                s.url
            );
        }
    }

    /// The default URLs must be unique so `add_companion_site`'s
    /// duplicate check + the Svelte `{#each ... (site.url)}` keyed
    /// rendering both work correctly.
    #[test]
    fn default_companion_sites_have_unique_urls() {
        let sites = default_companion_sites();
        let mut seen = std::collections::HashSet::new();
        for s in &sites {
            assert!(
                seen.insert(s.url.to_ascii_lowercase()),
                "duplicate URL: {}",
                s.url
            );
        }
    }

    /// Built-in icon monograms fit the 1–2 char tile slot. Three
    /// chars would visually overflow the 28×28 button in the Apps
    /// row.
    #[test]
    fn default_companion_site_icons_fit_tile() {
        for s in default_companion_sites() {
            assert!(
                (1..=2).contains(&s.icon.chars().count()),
                "{} icon must be 1-2 chars (got {:?})",
                s.name,
                s.icon
            );
        }
    }

    /// Happy-path config persistence: write a config, read it back,
    /// every field survives. If any future field is added without a
    /// matching `#[serde(default)]` or with the wrong case, this
    /// test surfaces it before users lose data.
    #[test]
    fn bifrost_config_roundtrips_through_disk() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("config.json");

        let original = BifrostConfig {
            sandboxie_path: Some(r"C:\Program Files\Sandboxie-Plus".into()),
            frontier_exe: Some(r"C:\Games\EVE Frontier\EVE Frontier.exe".into()),
            riders_dir: r"C:\BifrostRiders".into(),
            launch_all_on_start: true,
            companion_sites: default_companion_sites(),
            ui_zoom: 1.15,
            roster_columns: 3,
            roster_window_width: Some(1120),
            roster_window_height: Some(900),
        };
        original.save(&path).expect("save");

        let loaded = BifrostConfig::load_or_default(&path).expect("load");
        // Assert every persisted field — a future contributor adding
        // a new field with the wrong `#[serde(rename)]` or missing
        // `#[serde(default)]` shim would otherwise survive this test.
        assert_eq!(loaded.sandboxie_path, original.sandboxie_path);
        assert_eq!(loaded.frontier_exe, original.frontier_exe);
        assert_eq!(loaded.riders_dir, original.riders_dir);
        assert_eq!(loaded.launch_all_on_start, original.launch_all_on_start);
        assert_eq!(loaded.companion_sites.len(), original.companion_sites.len());
        // The four "newer" fields that the test previously didn't
        // assert. `ui_zoom` is `f32` so we compare with an epsilon to
        // tolerate JSON-roundtrip float imprecision.
        assert!(
            (loaded.ui_zoom - original.ui_zoom).abs() < f32::EPSILON,
            "ui_zoom: expected {} got {}",
            original.ui_zoom,
            loaded.ui_zoom
        );
        assert_eq!(loaded.roster_columns, original.roster_columns);
        assert_eq!(loaded.roster_window_width, original.roster_window_width);
        assert_eq!(loaded.roster_window_height, original.roster_window_height);
    }

    /// Loading from a missing path returns the defaults instead of
    /// erroring out. This is the first-launch path — Bifrost must not
    /// crash on a fresh machine.
    #[test]
    fn load_or_default_returns_defaults_for_missing_file() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("does-not-exist.json");
        let loaded = BifrostConfig::load_or_default(&path).expect("load");
        // We can't compare against `defaults()` directly because that
        // probes the host filesystem, but companion_sites is
        // host-independent.
        assert_eq!(
            loaded.companion_sites.len(),
            default_companion_sites().len()
        );
    }

    /// Pre-v0.1 configs may contain `enableWalletIntegration` or
    /// `chromeExe` keys we no longer read. They must load without
    /// error — serde's default behaviour is to ignore unknown fields,
    /// so this test pins that contract.
    #[test]
    fn load_tolerates_unknown_legacy_keys() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("legacy-config.json");
        let legacy = r#"{
            "sandboxiePath": "C:\\Program Files\\Sandboxie-Plus",
            "frontierExe": null,
            "ridersDir": "C:\\BifrostRiders",
            "launchAllOnStart": false,
            "enableWalletIntegration": true,
            "chromeExe": "C:\\Old\\Chrome\\chrome.exe",
            "companionSites": []
        }"#;
        std::fs::write(&path, legacy).expect("write");

        let loaded = BifrostConfig::load_or_default(&path).expect("legacy load");
        assert_eq!(
            loaded.sandboxie_path.as_deref(),
            Some(r"C:\Program Files\Sandboxie-Plus")
        );
        // Removed field doesn't crash anything; we just don't see it.
    }

    /// Save → load → save must produce stable output (idempotent).
    /// If serde reorders or normalises something, this catches it.
    #[test]
    fn config_save_load_save_is_stable() {
        let tmp = TempDir::new().expect("tempdir");
        let path_a = tmp.path().join("a.json");
        let path_b = tmp.path().join("b.json");

        let cfg = BifrostConfig {
            sandboxie_path: None,
            frontier_exe: None,
            riders_dir: "x".into(),
            launch_all_on_start: false,
            companion_sites: default_companion_sites(),
            ui_zoom: 1.0,
            roster_columns: 0,
            roster_window_width: None,
            roster_window_height: None,
        };
        cfg.save(&path_a).expect("save a");

        let reloaded = BifrostConfig::load_or_default(&path_a).expect("load");
        reloaded.save(&path_b).expect("save b");

        let bytes_a = std::fs::read_to_string(&path_a).expect("read a");
        let bytes_b = std::fs::read_to_string(&path_b).expect("read b");
        assert_eq!(bytes_a, bytes_b);
    }

    /// Pre-v0.0.4 configs don't carry a `uiZoom` field. They must
    /// load at 1.0 (default zoom) instead of erroring out. This pins
    /// the `#[serde(default)]` shim on the field.
    #[test]
    fn load_pre_zoom_config_defaults_to_1_0() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("legacy.json");
        let legacy = r#"{
            "sandboxiePath": null,
            "frontierExe": null,
            "ridersDir": "C:\\BifrostRiders",
            "launchAllOnStart": false,
            "companionSites": []
        }"#;
        std::fs::write(&path, legacy).expect("write");

        let loaded = BifrostConfig::load_or_default(&path).expect("legacy load");
        assert_eq!(loaded.ui_zoom, 1.0);
    }

    /// `defaults()` ships zoom = 1.0. If anyone ever changes that
    /// default it should be a deliberate decision, not an accident —
    /// this test will fail and prompt review.
    #[test]
    fn defaults_ship_zoom_one() {
        let d = BifrostConfig::defaults();
        assert_eq!(d.ui_zoom, 1.0);
    }

    /// Default roster layout is "auto" (responsive). Pre-existing
    /// configs that pre-date this field also load as auto thanks to
    /// `#[serde(default)]`. Pinning the contract here so a refactor
    /// that flips the default to "2" (or anything else) shows up in
    /// review rather than as a silent UX change after upgrade.
    #[test]
    fn defaults_ship_roster_columns_auto() {
        let d = BifrostConfig::defaults();
        assert_eq!(d.roster_columns, 0);
    }

    /// Legacy configs without `rosterColumns` must load with the auto
    /// default. Mirrors the `load_pre_zoom_config_defaults_to_1_0`
    /// test for the same forward-compat reasons.
    #[test]
    fn load_pre_roster_columns_config_defaults_to_auto() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("legacy.json");
        let legacy = r#"{
            "sandboxiePath": null,
            "frontierExe": null,
            "ridersDir": "C:\\BifrostRiders",
            "launchAllOnStart": false,
            "companionSites": [],
            "uiZoom": 1.0
        }"#;
        std::fs::write(&path, legacy).expect("write");
        let loaded = BifrostConfig::load_or_default(&path).expect("legacy load");
        assert_eq!(loaded.roster_columns, 0);
    }

    /// The Settings panel offers three preset zoom values. They must
    /// all fall inside the backend's accepted range — otherwise the
    /// picker would silently render values the `set_ui_zoom` command
    /// then rejects. Compile-time invariants on the bounds themselves
    /// (positive, max > min, etc.) live in the `const _: ()` block
    /// below the struct so clippy doesn't flag the runtime asserts as
    /// const-on-const.
    #[test]
    fn ui_zoom_presets_are_within_bounds() {
        for preset in [0.9_f32, 1.0_f32, 1.15_f32] {
            assert!(
                (MIN_UI_ZOOM..=MAX_UI_ZOOM).contains(&preset),
                "preset {preset} is outside zoom bounds \
                 [{MIN_UI_ZOOM}, {MAX_UI_ZOOM}]"
            );
        }
    }
}
