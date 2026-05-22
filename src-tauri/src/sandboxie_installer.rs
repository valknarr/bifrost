//! Sandboxie installer (Plus + Classic).
//!
//! Sandboxie is the only Bridge dependency that **can't** be made
//! portable: it ships a kernel driver (`SbieDrv.sys`) and a service
//! (`SbieSvc.exe`), both of which must live under
//! `C:\Program Files\Sandboxie-Plus\` or `C:\Program Files\Sandboxie\`.
//! So unlike [`crate::chromium`] (drop a zip into `<app-data>`) we run
//! the official Inno-Setup installer in silent mode with one
//! unavoidable UAC prompt.
//!
//! Both variants ship from the same GitHub repo
//! (`sandboxie-plus/Sandboxie`) and share the same kernel driver + CLI
//! surface (`SbieIni.exe`, `Start.exe`). They differ only in UI: Plus
//! is the modern Qt frontend (recommended default), Classic is the
//! legacy MFC UI on long-term support. Only one variant can be
//! installed at a time because they share the driver.
//!
//! The flow mirrors `chromium.rs` everywhere else:
//!   1. Query GitHub Releases (`sandboxie-plus/Sandboxie`) for the
//!      latest tag and pick the per-variant installer asset
//!      (`Sandboxie-Plus-x64-vX.Y.Z.exe` or
//!      `Sandboxie-Classic-vX.Y.Z.exe`).
//!   2. Download to `<app-data>/sandboxie-installer/downloads/`.
//!   3. Run it via PowerShell `Start-Process -Verb RunAs -Wait` with
//!      `/verysilent /suppressmsgboxes /norestart` so the install is
//!      headless once the user clicks "Yes" on the UAC prompt.
//!   4. After the installer exits, drop a `.bridge-installed-version`
//!      marker (JSON: `{"version": "...", "variant": "plus|classic"}`)
//!      so Settings can show "v1.16.7 · up to date" / "update
//!      available" and remembers which variant Bridge installed.
//!
//! We deliberately don't attempt SHA-256 verification because
//! Sandboxie releases don't publish per-asset checksums consistently.
//! Authenticode signature verification by the installer itself
//! (Windows checks the digital signature on every EXE run with Smart
//! App Control / SmartScreen) is our integrity guarantee.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cmd::run_elevated_silent;
use crate::error::{BridgeError, Result};
use crate::http;
use crate::release_cache;
use crate::sandboxie::Sandboxie;

const REPO: &str = "sandboxie-plus/Sandboxie";

/// Inno-Setup silent-install flags. Shared by `install` and `uninstall`
/// — same flags, same expected behaviour (no GUI, no message boxes, no
/// reboot prompt).
const INNO_SILENT_ARGS: &[&str] = &["/verysilent", "/suppressmsgboxes", "/norestart"];

/// Which Sandboxie variant Bridge is operating on. Both share the
/// same kernel driver + CLI surface, but ship under different names
/// and install paths.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxieVariant {
    /// Modern Qt UI. The recommended default. Install path:
    /// `C:\Program Files\Sandboxie-Plus\`.
    Plus,
    /// Legacy MFC UI on long-term support. Install path:
    /// `C:\Program Files\Sandboxie\`.
    Classic,
}

impl SandboxieVariant {
    /// Human-readable label for the UI ("Plus" / "Classic").
    pub fn label(self) -> &'static str {
        match self {
            SandboxieVariant::Plus => "Plus",
            SandboxieVariant::Classic => "Classic",
        }
    }
}

/// Asset matcher per variant. Real-world asset names seen on the
/// `sandboxie-plus/Sandboxie` releases page:
///   - `Sandboxie-Plus-x64-v1.17.6.exe`         (current Plus)
///   - `Sandboxie-Plus-ARM64-v1.17.6.exe`       (Plus ARM — rejected)
///   - `Sandboxie-Classic-x64-v5.72.6.exe`      (current Classic)
///   - `Sandboxie-Classic-v5.69.6.exe`          (older Classic, no `x64`)
///
/// Both Plus and Classic ship from the same release tag (Plus's
/// version owns the tag, Classic's version is embedded only in its
/// filename — see [`extract_version_from_asset_name`]).
fn matches_variant_asset(name: &str, variant: SandboxieVariant) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".exe") {
        return false;
    }
    match variant {
        SandboxieVariant::Plus => {
            lower.starts_with("sandboxie-plus-x64-v")
                && !lower.contains("arm")
                && !lower.contains("classic")
        }
        SandboxieVariant::Classic => {
            // Accept both the old `Sandboxie-Classic-v…` and the new
            // `Sandboxie-Classic-x64-v…` shape. Reject any future ARM
            // variant defensively.
            lower.starts_with("sandboxie-classic-") && !lower.contains("arm")
        }
    }
}

/// Pull the version string out of the asset filename. Required for
/// Classic because GitHub's release `tag_name` only reflects Plus's
/// version (Classic + Plus share a release tag). For Plus we could
/// use `tag_name` directly, but parsing both from the filename keeps
/// the two paths symmetric and protects us if upstream ever decouples
/// the tags.
///
/// Looks for the first `-v<digit>` boundary, then takes everything up
/// to `.exe`. Returns `None` if the pattern isn't there.
fn extract_version_from_asset_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    // Find `-v` immediately followed by a digit, in case the
    // filename contains other `-v` substrings (e.g. some hypothetical
    // future "preview" tag).
    let mut idx = 0;
    while let Some(p) = lower[idx..].find("-v") {
        let absolute = idx + p;
        let after = &lower[absolute + 2..];
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            let rest = &name[absolute + 2..];
            let ext = rest.to_ascii_lowercase().rfind(".exe")?;
            return Some(rest[..ext].to_string());
        }
        idx = absolute + 2;
    }
    None
}

/// Which Sandboxie variant lives at this install path, if any. Looks
/// at the path string as the cheap heuristic — Plus paths contain
/// "sandboxie-plus", Classic paths contain "sandboxie" but not the
/// "-plus" suffix.
pub fn variant_of_install_path(path: &Path) -> Option<SandboxieVariant> {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.contains("sandboxie-plus") {
        Some(SandboxieVariant::Plus)
    } else if lower.contains("sandboxie") {
        Some(SandboxieVariant::Classic)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Plus asset matcher ----------------------------------------

    #[test]
    fn matcher_accepts_plus_canonical_release() {
        assert!(matches_variant_asset(
            "Sandboxie-Plus-x64-v1.16.7.exe",
            SandboxieVariant::Plus
        ));
    }

    #[test]
    fn matcher_plus_is_case_insensitive() {
        assert!(matches_variant_asset(
            "sandboxie-plus-x64-v1.16.7.exe",
            SandboxieVariant::Plus
        ));
        assert!(matches_variant_asset(
            "SANDBOXIE-PLUS-X64-V1.16.7.EXE",
            SandboxieVariant::Plus
        ));
    }

    #[test]
    fn matcher_plus_rejects_arm64() {
        assert!(!matches_variant_asset(
            "Sandboxie-Plus-ARM64-v1.16.7.exe",
            SandboxieVariant::Plus
        ));
    }

    #[test]
    fn matcher_plus_rejects_classic_variant() {
        assert!(!matches_variant_asset(
            "Sandboxie-Classic-v5.69.exe",
            SandboxieVariant::Plus
        ));
    }

    #[test]
    fn matcher_plus_rejects_non_exe() {
        assert!(!matches_variant_asset(
            "Sandboxie-Plus-x64-v1.16.7.zip",
            SandboxieVariant::Plus
        ));
    }

    #[test]
    fn matcher_plus_rejects_pre_1x_naming() {
        // Pre-1.x releases used `Sandboxie-Plus-Setup-*` — we want the
        // new layout only.
        assert!(!matches_variant_asset(
            "Sandboxie-Plus-Setup-x64-v0.99.exe",
            SandboxieVariant::Plus
        ));
    }

    // ---- Classic asset matcher -------------------------------------

    /// As of 2025 the real Classic asset name has the `x64` infix:
    /// `Sandboxie-Classic-x64-v5.72.6.exe`. The older shape without
    /// the infix (`Sandboxie-Classic-v5.69.6.exe`) appears in
    /// historical releases — accept both.
    #[test]
    fn matcher_accepts_classic_current_x64_layout() {
        assert!(matches_variant_asset(
            "Sandboxie-Classic-x64-v5.72.6.exe",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_accepts_classic_legacy_no_arch_layout() {
        assert!(matches_variant_asset(
            "Sandboxie-Classic-v5.69.6.exe",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_classic_is_case_insensitive() {
        assert!(matches_variant_asset(
            "sandboxie-classic-x64-v5.72.6.exe",
            SandboxieVariant::Classic
        ));
        assert!(matches_variant_asset(
            "SANDBOXIE-CLASSIC-X64-V5.72.6.EXE",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_classic_rejects_plus_variant() {
        assert!(!matches_variant_asset(
            "Sandboxie-Plus-x64-v1.17.6.exe",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_classic_rejects_non_exe() {
        assert!(!matches_variant_asset(
            "Sandboxie-Classic-x64-v5.72.6.zip",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_classic_rejects_future_arm() {
        // Defensive: if upstream ever ships an ARM Classic build,
        // make sure we don't accidentally install it on x64.
        assert!(!matches_variant_asset(
            "Sandboxie-Classic-ARM64-v5.72.6.exe",
            SandboxieVariant::Classic
        ));
    }

    #[test]
    fn matcher_classic_rejects_pre_1x_naming() {
        // Old pre-Plus-era Classic builds used `SandboxieInstall.exe`
        // — outside the new "Sandboxie-Classic-*" layout we expect.
        assert!(!matches_variant_asset(
            "SandboxieInstall.exe",
            SandboxieVariant::Classic
        ));
    }

    // ---- extract_version_from_asset_name ---------------------------

    #[test]
    fn extract_version_from_plus_filename() {
        assert_eq!(
            extract_version_from_asset_name("Sandboxie-Plus-x64-v1.17.6.exe").as_deref(),
            Some("1.17.6")
        );
    }

    #[test]
    fn extract_version_from_classic_current_filename() {
        assert_eq!(
            extract_version_from_asset_name("Sandboxie-Classic-x64-v5.72.6.exe").as_deref(),
            Some("5.72.6")
        );
    }

    #[test]
    fn extract_version_from_classic_legacy_filename() {
        assert_eq!(
            extract_version_from_asset_name("Sandboxie-Classic-v5.69.6.exe").as_deref(),
            Some("5.69.6")
        );
    }

    #[test]
    fn extract_version_skips_non_digit_dash_v() {
        // "-vault" would naively match `-v` but the char after must
        // be a digit. This guards against false-positive matches if
        // a future filename contains words beginning with "v".
        assert_eq!(
            extract_version_from_asset_name("Sandboxie-Vault-x64-v1.0.exe").as_deref(),
            Some("1.0")
        );
    }

    #[test]
    fn extract_version_returns_none_when_pattern_absent() {
        assert!(extract_version_from_asset_name("checksums.txt").is_none());
        assert!(extract_version_from_asset_name("Sandboxie-Plus.exe").is_none());
    }

    // ---- variant_of_install_path -----------------------------------

    #[test]
    fn variant_of_install_path_recognises_plus() {
        assert_eq!(
            variant_of_install_path(Path::new(r"C:\Program Files\Sandboxie-Plus")),
            Some(SandboxieVariant::Plus)
        );
    }

    #[test]
    fn variant_of_install_path_recognises_classic() {
        assert_eq!(
            variant_of_install_path(Path::new(r"C:\Program Files\Sandboxie")),
            Some(SandboxieVariant::Classic)
        );
    }

    #[test]
    fn variant_of_install_path_is_case_insensitive() {
        assert_eq!(
            variant_of_install_path(Path::new(r"c:\program files\sandboxie-plus")),
            Some(SandboxieVariant::Plus)
        );
    }

    #[test]
    fn variant_of_install_path_unknown_path_returns_none() {
        assert_eq!(
            variant_of_install_path(Path::new(r"C:\Program Files\SomethingElse")),
            None
        );
    }

    // ---- version-marker round-trip ----------------------------------
    //
    // Sandboxie is special — Bridge tracks the version *we installed*
    // separately from "is it detected on disk?", because users can
    // install Sandboxie themselves and Bridge then has no marker. The
    // `status()` function combines both. These tests pin the marker
    // half of that contract.

    use tempfile::TempDir;

    #[test]
    fn read_installed_marker_returns_none_on_empty_app_data() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(read_installed_marker(tmp.path()).is_none());
    }

    #[test]
    fn version_marker_roundtrips_plus_json() {
        let tmp = TempDir::new().expect("tempdir");
        write_marker(tmp.path(), "1.16.7", SandboxieVariant::Plus).expect("write marker");
        let (v, variant) = read_installed_marker(tmp.path()).expect("read marker");
        assert_eq!(v, "1.16.7");
        assert_eq!(variant, SandboxieVariant::Plus);
    }

    #[test]
    fn version_marker_roundtrips_classic_json() {
        let tmp = TempDir::new().expect("tempdir");
        write_marker(tmp.path(), "5.69.6", SandboxieVariant::Classic).expect("write marker");
        let (v, variant) = read_installed_marker(tmp.path()).expect("read marker");
        assert_eq!(v, "5.69.6");
        assert_eq!(variant, SandboxieVariant::Classic);
    }

    /// Legacy markers (Bridge pre-Classic-support) were plain text
    /// holding just the version tag. They must still load, defaulting
    /// to variant=Plus since that was the only thing Bridge could
    /// install back then.
    #[test]
    fn version_marker_reads_legacy_plain_text_as_plus() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(installer_dir(tmp.path())).expect("dir");
        std::fs::write(
            installer_dir(tmp.path()).join(".bridge-installed-version"),
            "1.16.7",
        )
        .expect("legacy marker");
        let (v, variant) = read_installed_marker(tmp.path()).expect("read legacy");
        assert_eq!(v, "1.16.7");
        assert_eq!(
            variant,
            SandboxieVariant::Plus,
            "pre-Classic-support markers must default to Plus"
        );
    }

    #[test]
    fn empty_marker_file_reads_as_none() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(installer_dir(tmp.path())).expect("dir");
        std::fs::write(
            installer_dir(tmp.path()).join(".bridge-installed-version"),
            "",
        )
        .expect("empty marker");
        assert!(read_installed_marker(tmp.path()).is_none());
    }

    /// Cascading: `status()` combines on-disk detection (the
    /// `detected_variant` arg from the host probe) with the
    /// Bridge-written marker. The "external install" case — detected
    /// but no marker — must surface as `installed_version = None`
    /// rather than claiming Bridge installed something it didn't.
    /// Also covers `update_available = false` for the external case
    /// (we can't say one way or the other if we don't know what's
    /// installed).
    #[tokio::test]
    async fn status_with_detected_but_no_marker_reports_external_install() {
        let tmp = TempDir::new().expect("tempdir");
        let s = status(tmp.path(), Some(SandboxieVariant::Plus), false).await;

        assert!(s.detected, "host probe said yes");
        assert_eq!(s.installed_variant, Some(SandboxieVariant::Plus));
        assert!(
            s.installed_version.is_none(),
            "no marker → no Bridge-tracked version"
        );
        assert!(
            !s.update_available,
            "we don't know what version is installed — never claim 'update available'"
        );
    }

    /// And: not-detected → marker is meaningless even if present.
    /// A stale marker from a previous install the user removed via
    /// Windows Settings shouldn't claim Sandboxie is still around.
    #[tokio::test]
    async fn status_with_undetected_ignores_stale_marker() {
        let tmp = TempDir::new().expect("tempdir");
        write_marker(tmp.path(), "1.16.7", SandboxieVariant::Plus).expect("marker");

        let s = status(tmp.path(), None, false).await;

        assert!(!s.detected);
        assert!(s.installed_variant.is_none());
        assert!(
            s.installed_version.is_none(),
            "marker without on-disk binaries must NOT report a version"
        );
    }
}

/// Resolved metadata for the latest Sandboxie installer release on
/// GitHub. Populated by [`fetch_latest_release`]. The `variant` field
/// pins which family this release is for so cached entries are
/// self-describing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxieRelease {
    pub tag: String,
    pub exe_url: String,
    pub exe_name: String,
    pub size_bytes: u64,
    pub published_at: Option<String>,
    pub variant: SandboxieVariant,
}

/// Live status the Settings panel renders for the Sandboxie row.
/// Combines on-disk detection (`detected` + `installed_variant`) with
/// the Bridge-tracked version (`installed_version`, only set when
/// Bridge ran the installer itself) and the latest-known upstream
/// tags for *both* variants — the UI shows the install button for
/// whichever variant the user picks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxieInstallerStatus {
    /// Variant currently detected on disk, if any. Only one can be
    /// installed at a time (they share the kernel driver).
    pub installed_variant: Option<SandboxieVariant>,
    /// Tag Bridge last installed via this module. `None` if Bridge
    /// didn't install it (external install) or nothing is installed.
    pub installed_version: Option<String>,
    /// True when *something* is detected on disk (host probe).
    pub detected: bool,
    /// Latest Plus release tag on GitHub.
    pub latest_plus_version: Option<String>,
    /// Latest Classic release tag on GitHub.
    pub latest_classic_version: Option<String>,
    /// True when the installed variant has a newer version available
    /// on GitHub.
    pub update_available: bool,
    /// Approximate download size of the currently-installed variant's
    /// latest release (for the UI to display a "~12 MB" hint). When
    /// nothing is installed yet, this is the Plus download size (the
    /// default install target).
    pub latest_size_bytes: Option<u64>,
    /// Human-readable error if the GitHub fetch failed.
    pub latest_error: Option<String>,
}

/// Query the GitHub Releases API for the latest Sandboxie release and
/// pick the per-variant installer EXE asset. The raw release JSON is
/// cached under `"sandboxie_releases_json"` so a single GitHub fetch
/// serves both Plus and Classic lookups within the cache TTL.
pub async fn fetch_latest_release(variant: SandboxieVariant) -> Result<SandboxieRelease> {
    let body = fetch_release_json_cached(false).await?;
    parse_release_for_variant(&body, variant)
}

/// Parse a cached `/releases/latest` JSON blob and extract the asset
/// for the requested variant. Each variant's version is parsed from
/// its own asset filename rather than the release `tag_name` — the
/// repo bundles Plus + Classic into one release tagged with Plus's
/// version, so Classic's actual version (e.g. `5.72.6`) only appears
/// in its filename (`Sandboxie-Classic-x64-v5.72.6.exe`).
fn parse_release_for_variant(
    body: &serde_json::Value,
    variant: SandboxieVariant,
) -> Result<SandboxieRelease> {
    let assets = body["assets"]
        .as_array()
        .ok_or_else(|| BridgeError::Other("release JSON has no assets array".into()))?;

    let asset = assets
        .iter()
        .find(|a| matches_variant_asset(a["name"].as_str().unwrap_or_default(), variant))
        .ok_or_else(|| {
            BridgeError::Other(format!(
                "release has no Sandboxie {} installer asset",
                variant.label()
            ))
        })?;

    let exe_name = asset["name"]
        .as_str()
        .ok_or_else(|| BridgeError::Other("asset has no name".into()))?
        .to_string();

    // Each variant's true version lives in its own filename, not in
    // the shared release tag.
    let tag = extract_version_from_asset_name(&exe_name).ok_or_else(|| {
        BridgeError::Other(format!(
            "couldn't parse a version from asset name {exe_name}"
        ))
    })?;

    Ok(SandboxieRelease {
        tag,
        exe_url: asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| BridgeError::Other("asset has no browser_download_url".into()))?
            .to_string(),
        exe_name,
        size_bytes: asset["size"].as_u64().unwrap_or(0),
        published_at: body["published_at"].as_str().map(|s| s.to_string()),
        variant,
    })
}

/// Cache the raw GitHub `/releases/latest` JSON under a single key so
/// fetching Plus + Classic info doesn't hit GitHub twice. Force_refresh
/// bypasses the cache for one call (used by "Check for updates").
async fn fetch_release_json_cached(force_refresh: bool) -> Result<serde_json::Value> {
    release_cache::fetch_with_cache::<serde_json::Value, _, _>(
        "sandboxie_releases_json",
        force_refresh,
        || async { release_cache::fetch_release_json(REPO).await },
    )
    .await
}

fn installer_dir(app_data: &Path) -> PathBuf {
    app_data.join("sandboxie-installer")
}

fn downloads_dir(app_data: &Path) -> PathBuf {
    installer_dir(app_data).join("downloads")
}

fn version_marker(app_data: &Path) -> PathBuf {
    installer_dir(app_data).join(".bridge-installed-version")
}

/// On-disk marker payload. Stored as JSON so we can extend without
/// further migrations; pre-Classic-support Bridge wrote plain text
/// instead, which [`read_installed_marker`] still tolerates.
#[derive(Debug, Serialize, Deserialize)]
struct MarkerPayload {
    version: String,
    variant: SandboxieVariant,
}

fn write_marker(app_data: &Path, version: &str, variant: SandboxieVariant) -> Result<()> {
    std::fs::create_dir_all(installer_dir(app_data))?;
    let payload = MarkerPayload {
        version: version.to_string(),
        variant,
    };
    let json = serde_json::to_string(&payload)?;
    std::fs::write(version_marker(app_data), json)?;
    Ok(())
}

/// Read the tag + variant Bridge last installed (the contents of the
/// `.bridge-installed-version` marker). Returns `None` when the user
/// installed Sandboxie out-of-band, or never installed at all.
///
/// Backward-compat: if the marker is plain text (pre-Classic-support
/// markers Bridge wrote before this module learned about variants),
/// it's interpreted as a version string with variant=Plus.
pub fn read_installed_marker(app_data: &Path) -> Option<(String, SandboxieVariant)> {
    let txt = std::fs::read_to_string(version_marker(app_data)).ok()?;
    let trimmed = txt.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(payload) = serde_json::from_str::<MarkerPayload>(trimmed) {
        return Some((payload.version, payload.variant));
    }

    // Legacy plain-text marker — Bridge only knew about Plus then.
    Some((trimmed.to_string(), SandboxieVariant::Plus))
}

/// Combined live status for the Settings panel. `detected_variant`
/// should be passed in from the caller (which already resolved the
/// install path + ran `detect_sandboxie`) so we don't duplicate the
/// probe logic. `force_refresh = true` bypasses the [`release_cache`]
/// for one call (used by the "Check for updates" button).
pub async fn status(
    app_data: &Path,
    detected_variant: Option<SandboxieVariant>,
    force_refresh: bool,
) -> SandboxieInstallerStatus {
    let detected = detected_variant.is_some();

    // Pull the marker if (and only if) something is actually on disk.
    // A stale marker without binaries is misleading — treat as fresh.
    let marker = if detected {
        read_installed_marker(app_data)
    } else {
        None
    };
    let installed_version = marker.as_ref().map(|(v, _)| v.clone());
    // Prefer the host-detected variant (the source of truth — that's
    // what's actually on disk). Fall back to the marker variant for
    // the case where the install path didn't classify cleanly.
    let installed_variant = detected_variant.or_else(|| marker.as_ref().map(|(_, v)| *v));

    // One GitHub fetch, then parse twice — once per variant. Failures
    // are surfaced as `latest_error`; per-asset misses surface as
    // `None` for that variant's tag (e.g. a release that only ships
    // Plus assets reports a Classic version of None).
    let raw_json = fetch_release_json_cached(force_refresh).await;
    let (latest_plus, latest_classic, latest_error) = match raw_json {
        Ok(body) => {
            let plus = parse_release_for_variant(&body, SandboxieVariant::Plus).ok();
            let classic = parse_release_for_variant(&body, SandboxieVariant::Classic).ok();
            (plus, classic, None)
        }
        Err(e) => {
            let msg = release_cache::friendly_fetch_error(&e.to_string());
            tracing::warn!("sandboxie-installer: fetch_latest_release failed: {e}");
            (None, None, Some(msg))
        }
    };

    let latest_plus_version = latest_plus.as_ref().map(|r| r.tag.clone());
    let latest_classic_version = latest_classic.as_ref().map(|r| r.tag.clone());

    // The "currently relevant" release is whatever variant is on
    // disk; if nothing's installed yet the default install target is
    // Plus, so we surface that variant's download size.
    let active_variant = installed_variant.unwrap_or(SandboxieVariant::Plus);
    let active_release = match active_variant {
        SandboxieVariant::Plus => latest_plus.as_ref(),
        SandboxieVariant::Classic => latest_classic.as_ref(),
    };
    let latest_version_for_active = active_release.map(|r| r.tag.clone());
    let latest_size_bytes = active_release.map(|r| r.size_bytes);

    let update_available = match (&installed_version, &latest_version_for_active) {
        (Some(i), Some(l)) => i != l,
        // Detected but no marker → manual install, we can't tell. Don't
        // claim an update is available (would be misleading).
        (None, Some(_)) if detected => false,
        // Not installed at all → "update" semantically means "install".
        (None, Some(_)) => true,
        _ => false,
    };

    SandboxieInstallerStatus {
        installed_variant,
        installed_version,
        detected,
        latest_plus_version,
        latest_classic_version,
        update_available,
        latest_size_bytes,
        latest_error,
    }
}

/// Download + run the Sandboxie installer with silent flags. The
/// installer itself triggers Windows' UAC prompt; once accepted, the
/// install runs hands-off. After the installer process exits, we drop a
/// version+variant marker so subsequent calls to
/// [`read_installed_marker`] know which tag is on disk.
pub async fn install(app_data: &Path, release: &SandboxieRelease) -> Result<()> {
    let downloads = downloads_dir(app_data);
    std::fs::create_dir_all(&downloads)?;
    let installer_path = downloads.join(&release.exe_name);

    // Re-download if we don't already have the exact file. We keep the
    // .exe around between runs so re-running "Reinstall" is fast.
    if !installer_path.exists() {
        tracing::info!(
            "sandboxie-installer: downloading {} ({} MB)",
            release.exe_name,
            release.size_bytes / 1_048_576
        );
        // Sandboxie installer is ~12 MB. 300 s timeout is overkill
        // but matches the previous behaviour; shared client reuses
        // the GitHub CDN connection from the latest-release fetch.
        let exe_bytes = http::client()
            .get(&release.exe_url)
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| BridgeError::Other(format!("sandboxie installer download failed: {e}")))?
            .bytes()
            .await
            .map_err(|e| {
                BridgeError::Other(format!("sandboxie installer body read failed: {e}"))
            })?;

        std::fs::write(&installer_path, &exe_bytes)?;
    } else {
        tracing::info!(
            "sandboxie-installer: reusing cached {}",
            installer_path.display()
        );
    }

    // Run the installer elevated and silently. `run_elevated_silent`
    // wraps `Start-Process -Verb RunAs -Wait` in PowerShell, suppresses
    // the console flash, and maps a declined UAC prompt to a friendly
    // error message.
    tracing::info!(
        "sandboxie-installer: launching {} (UAC prompt will appear)",
        installer_path.display()
    );
    run_elevated_silent(&installer_path, INNO_SILENT_ARGS, "Sandboxie installer").await?;

    // Persist version+variant marker only on success so we don't lie
    // about what's installed.
    write_marker(app_data, &release.tag, release.variant)?;
    tracing::info!(
        "sandboxie-installer: installed Sandboxie {} v{} (marker at {})",
        release.variant.label(),
        release.tag,
        version_marker(app_data).display()
    );
    Ok(())
}

/// Run the Inno-Setup uninstaller that ships with Sandboxie to remove
/// the program, its kernel driver, and the service. Like [`install`],
/// this requires elevation and triggers exactly one UAC prompt.
/// `sandboxie_root` is the program directory we resolved earlier (e.g.
/// `C:\Program Files\Sandboxie-Plus` or `C:\Program Files\Sandboxie`);
/// we look for `unins000.exe` (the standard Inno-Setup uninstaller
/// name) and invoke it with the same silent flags as the installer.
/// Both Plus and Classic ship this same uninstaller, so one code path
/// covers both.
///
/// Best-effort: if no uninstaller is found we still scrub Bridge's own
/// version marker so the UI doesn't claim a Bridge-installed version
/// when the user removes Sandboxie out-of-band.
pub async fn uninstall(app_data: &Path, sandboxie_root: Option<&Path>) -> Result<()> {
    if let Some(root) = sandboxie_root {
        let uninstaller = root.join("unins000.exe");
        if uninstaller.exists() {
            tracing::info!(
                "sandboxie-installer: running uninstaller {} (UAC prompt will appear)",
                uninstaller.display()
            );
            run_elevated_silent(&uninstaller, INNO_SILENT_ARGS, "Sandboxie uninstaller").await?;
        } else {
            tracing::warn!(
                "sandboxie-installer: no uninstaller at {} — scrubbing Bridge marker only",
                uninstaller.display()
            );
        }
    }

    // Always clear our own version marker — keeps the UI honest even if
    // the user removed Sandboxie via Windows Settings while Bridge was
    // closed.
    let marker = version_marker(app_data);
    if marker.exists() {
        let _ = std::fs::remove_file(&marker);
    }
    Ok(())
}

/// Silence Sandboxie's "Program Compatibility" wizard.
///
/// Without this, the first time SbieSvc spins up after install — which
/// for Bridge users is right when they click "Add pilot" — Sandboxie
/// scans the host for installed apps with known compat templates
/// (Office licensing, Edge, Windows Live, etc.) and prompts the user
/// to apply them globally. The dialog is harmless but it shatters
/// Bridge's "you never have to look at Sandboxie's UI" promise.
///
/// We set `AutoRunSoftCompat=n` in Sandboxie's global config, which is
/// the engine-level switch the wizard checks before running. Same
/// effect as ticking the "Don't check for program compatibility in
/// the future" checkbox in the dialog, done preemptively. Best-effort:
/// if SbieIni isn't where we expected or the call fails we only log a
/// warning — the wizard appearing is a paper-cut, not a blocker.
/// Applies to both Plus and Classic (the global config option lives
/// in the shared engine, not the variant-specific UI).
pub async fn suppress_compat_prompts(sandboxie_root: &Path) {
    let sb = match Sandboxie::at(sandboxie_root) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                "suppress_compat_prompts: can't resolve Sandboxie at {}: {e}",
                sandboxie_root.display()
            );
            return;
        }
    };

    if let Err(e) = sb.set("GlobalSettings", "AutoRunSoftCompat", "n").await {
        tracing::warn!("suppress_compat_prompts: failed to set AutoRunSoftCompat=n: {e}");
        return;
    }
    tracing::info!("sandboxie-installer: AutoRunSoftCompat=n applied — compat wizard silenced");
}
