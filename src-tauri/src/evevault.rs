//! EVE Vault Chromium extension downloader & installer.
//!
//! Bifrost can optionally manage a local copy of the [EVE Vault] extension
//! so users don't have to clone & build it themselves. We fetch
//! pre-built release artifacts from the official `evefrontier/evevault`
//! GitHub Releases, verify the SHA-256 against the published
//! `checksums.txt`, and extract the ZIP into a per-installation
//! directory under Bifrost's app-data.
//!
//! [EVE Vault]: https://github.com/evefrontier/evevault

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{BifrostError, Result};
use crate::http;
use crate::release_cache;

const REPO: &str = "evefrontier/evevault";
const ASSET_NAME: &str = "eve-vault-chrome.zip";

/// Latest release as resolved from the GitHub API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag: String,
    pub zip_url: String,
    pub checksum_url: Option<String>,
    pub published_at: Option<String>,
}

/// Combined status returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EveVaultStatus {
    /// Most recent release tag on GitHub. None if the API call failed
    /// (offline, rate-limited, etc.).
    pub latest_version: Option<String>,
    /// Currently installed version (the contents of the version marker
    /// file). None when nothing is installed.
    pub installed_version: Option<String>,
    /// Absolute path to the unpacked extension, if installed.
    pub install_dir: Option<String>,
    /// True when both versions are known and they differ.
    pub update_available: bool,
    /// Human-readable error if the GitHub fetch failed. UI surfaces
    /// this so users know why `latest_version` is null.
    pub latest_error: Option<String>,
}

/// Fetch the metadata for the latest published release. No download yet.
///
/// Raw / uncached path. Callers SHOULD wrap with
/// [`release_cache::fetch_with_cache`] under key `"evevault"` so
/// the rate-limit-backoff cache applies. `status()` below +
/// `commands::installers::install_evevault` both do that. Don't
/// call this directly from anywhere new.
pub async fn fetch_latest_release() -> Result<ReleaseInfo> {
    let body = release_cache::fetch_release_json(REPO).await?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| BifrostError::Other("release JSON has no tag_name".into()))?
        .to_string();
    let assets = body["assets"]
        .as_array()
        .ok_or_else(|| BifrostError::Other("release JSON has no assets array".into()))?;

    let zip_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(ASSET_NAME))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| BifrostError::Other(format!("release has no {ASSET_NAME} asset")))?
        .to_string();
    let checksum_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some("checksums.txt"))
        .and_then(|a| a["browser_download_url"].as_str())
        .map(|s| s.to_string());

    Ok(ReleaseInfo {
        tag,
        zip_url,
        checksum_url,
        published_at: body["published_at"].as_str().map(|s| s.to_string()),
    })
}

/// Where the unpacked extension lives. Stable path so updates replace in-place.
pub fn install_dir(app_data: &Path) -> PathBuf {
    app_data.join("evevault").join("current")
}

/// Tear down the installed extension by removing its directory tree.
/// Safe no-op if nothing's there. The version marker lives inside
/// `install_dir`, so this also clears `installed_version`.
pub fn uninstall(app_data: &Path) -> Result<()> {
    let dir = install_dir(app_data);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        tracing::info!("evevault: uninstalled (removed {})", dir.display());
    }
    Ok(())
}

/// File we write after a successful install so we can read back the version.
fn version_marker(app_data: &Path) -> PathBuf {
    install_dir(app_data).join(".bifrost-version")
}

/// Read the installed version, if any. Best-effort; returns None on any error.
pub fn read_installed_version(app_data: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(version_marker(app_data)).ok()?;
    let trimmed = txt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Combine local installed state + remote latest into the status struct.
pub async fn status(app_data: &Path, force_refresh: bool) -> EveVaultStatus {
    let installed_version = read_installed_version(app_data);
    let install_dir_str = if installed_version.is_some() {
        Some(install_dir(app_data).to_string_lossy().into_owned())
    } else {
        None
    };

    let cached = crate::release_cache::fetch_with_cache::<ReleaseInfo, _, _>(
        "evevault",
        force_refresh,
        fetch_latest_release,
    )
    .await;
    let (latest_version, latest_error) = match cached {
        Ok(r) => (Some(r.tag), None),
        Err(e) => {
            let msg = release_cache::friendly_fetch_error(&e.to_string());
            tracing::warn!("evevault: fetch_latest_release failed: {e}");
            (None, Some(msg))
        }
    };

    let update_available = match (&installed_version, &latest_version) {
        (Some(i), Some(l)) => i != l,
        // No install: an update is implicitly "available" if we know a latest.
        (None, Some(_)) => true,
        _ => false,
    };

    EveVaultStatus {
        latest_version,
        installed_version,
        install_dir: install_dir_str,
        update_available,
        latest_error,
    }
}

/// Download the ZIP, verify SHA-256 against checksums.txt (when provided),
/// and extract over the install directory. Existing install is wiped first
/// so there are no stale files from older versions.
/// Read the installed extension's `manifest.json` and return the popup
/// HTML filename declared under `action.default_popup` (MV3) or
/// `browser_action.default_popup` (MV2 fallback). Returns `None` when
/// the extension isn't installed or the manifest doesn't declare a
/// popup. The caller combines this with the extension's Chromium-
/// assigned ID (see `browser::read_extension_id`) to build a working
/// `chrome-extension://<id>/<popup>` URL — we can't compute the ID
/// reliably ourselves because Chromium's path → ID hashing differs from
/// any reproducible Rust implementation we've tried.
pub fn popup_filename(app_data: &Path) -> Option<String> {
    let manifest_path = install_dir(app_data).join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&raw).ok()?;
    manifest
        .pointer("/action/default_popup")
        .or_else(|| manifest.pointer("/browser_action/default_popup"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Download the extension ZIP, verify SHA-256 against
/// `checksums.txt` when published, and extract into [`install_dir`]
/// (wiping any previous install). Writes a `.bifrost-version` marker
/// on success.
pub async fn install(app_data: &Path, release: &ReleaseInfo) -> Result<()> {
    // EVE Vault is a few-MB download — the shared client's 120 s
    // default timeout fits, no override needed.
    let client = http::client();

    // 1. Fetch expected SHA-256 if a checksums file is published.
    let expected_hash: Option<String> = if let Some(url) = &release.checksum_url {
        let txt = client
            .get(url)
            .send()
            .await
            .map_err(|e| BifrostError::Other(format!("checksums.txt download failed: {e}")))?
            .text()
            .await
            .map_err(|e| BifrostError::Other(format!("checksums.txt body read failed: {e}")))?;
        txt.lines()
            .find(|line| line.contains(ASSET_NAME))
            .and_then(|line| line.split_whitespace().next())
            .map(|s| s.to_lowercase())
    } else {
        None
    };

    // 2. Download the ZIP with a 50 MiB cap — EVE Vault releases
    // are small (~5 MB). 50 MiB gives 10× growth headroom while
    // rejecting an attacker-controlled or compromised upstream
    // that tries to feed us unbounded data.
    const EVEVAULT_ZIP_CAP: u64 = 50 * 1024 * 1024;
    let resp = client
        .get(&release.zip_url)
        .send()
        .await
        .map_err(|e| BifrostError::Other(format!("zip download failed: {e}")))?;
    let zip_bytes = crate::http::download_capped(resp, EVEVAULT_ZIP_CAP)
        .await
        .map_err(|e| BifrostError::Other(format!("zip body read failed: {e}")))?;

    // 3. Verify hash if we have one.
    if let Some(expected) = &expected_hash {
        let actual = hex_encode(&Sha256::digest(&zip_bytes));
        if &actual != expected {
            return Err(BifrostError::Other(format!(
                "SHA-256 mismatch for {ASSET_NAME}: expected {expected}, got {actual}"
            )));
        }
        tracing::info!("evevault: SHA-256 verified ({})", &actual[..16]);
    } else {
        tracing::warn!("evevault: no checksums.txt available; skipping hash verification");
    }

    // 4. Wipe + recreate the target dir, then extract.
    let target = install_dir(app_data);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::create_dir_all(&target)?;

    let mut archive = zip::ZipArchive::new(Cursor::new(&zip_bytes[..]))
        .map_err(|e| BifrostError::Other(format!("zip parse failed: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| BifrostError::Other(format!("zip entry {i}: {e}")))?;
        let Some(name) = entry.enclosed_name() else {
            // Reject path traversal / absolute paths.
            continue;
        };
        let out_path = target.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Bound the per-entry allocation + read. `entry.size()` is
        // the declared-uncompressed size from the zip header —
        // attacker / upstream controlled. A malicious or corrupted
        // zip could declare a multi-GB size and force a giant
        // pre-allocation; a high-compression-ratio zip could also
        // expand past the download cap during streaming.
        // EVE Vault's largest legitimate file is ~250 KB; capping
        // each entry at 10 MiB gives 40× headroom and bounds the
        // worst case. Both `min(declared, cap)` for the Vec capacity
        // AND `take(cap)` on the reader so a lying header can't
        // bypass the cap.
        const MAX_ENTRY_BYTES: u64 = 10 * 1024 * 1024;
        let declared = entry.size();
        let cap_hint = std::cmp::min(declared, MAX_ENTRY_BYTES) as usize;
        let mut buf = Vec::with_capacity(cap_hint);
        let mut limited = std::io::Read::take(&mut entry, MAX_ENTRY_BYTES + 1);
        std::io::Read::read_to_end(&mut limited, &mut buf)
            .map_err(|e| BifrostError::Other(format!("zip entry read: {e}")))?;
        if buf.len() as u64 > MAX_ENTRY_BYTES {
            return Err(BifrostError::Other(format!(
                "zip entry exceeded {MAX_ENTRY_BYTES} bytes — refusing to extract"
            )));
        }
        std::fs::write(&out_path, &buf)?;
    }

    // 5. Drop the version marker.
    std::fs::write(version_marker(app_data), &release.tag)?;
    tracing::info!(
        "evevault: installed {} into {}",
        release.tag,
        target.display()
    );
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    //! Version marker + popup-filename + hex-encoding round-trips.
    //! Same cascading pattern as `chromium.rs::tests` — a fresh
    //! app-data dir → simulate install → read back → uninstall →
    //! reads as gone.
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_installed_version_returns_none_on_empty_app_data() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(read_installed_version(tmp.path()).is_none());
    }

    #[test]
    fn version_marker_roundtrips() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "v0.0.9").expect("marker");

        assert_eq!(
            read_installed_version(tmp.path()).as_deref(),
            Some("v0.0.9")
        );
    }

    #[test]
    fn empty_marker_file_reads_as_none() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "").expect("empty marker");
        assert!(read_installed_version(tmp.path()).is_none());
    }

    /// Uninstall removes the entire install dir (extension, marker,
    /// manifest). The Settings row flips to "Not installed" on the
    /// next status() call.
    #[test]
    fn uninstall_clears_install_dir() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "v0.0.9").expect("marker");
        std::fs::write(
            install_dir(tmp.path()).join("manifest.json"),
            r#"{"name":"EVE Vault"}"#,
        )
        .expect("manifest");

        uninstall(tmp.path()).expect("uninstall");
        assert!(!install_dir(tmp.path()).exists());
        assert!(read_installed_version(tmp.path()).is_none());
    }

    /// `popup_filename` parses the extension's `manifest.json` to find
    /// the popup HTML the wallet button should land on. This pins the
    /// MV3 (`action.default_popup`) shape — if upstream EVE Vault
    /// switches manifests, this test surfaces it.
    #[test]
    fn popup_filename_reads_mv3_action_default_popup() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        let manifest = r#"{
            "manifest_version": 3,
            "name": "EVE Vault",
            "version": "0.0.9",
            "action": { "default_popup": "popup.html" }
        }"#;
        std::fs::write(install_dir(tmp.path()).join("manifest.json"), manifest).expect("manifest");

        assert_eq!(popup_filename(tmp.path()).as_deref(), Some("popup.html"));
    }

    /// MV2 fallback: older extensions use `browser_action.default_popup`.
    /// EVE Vault is MV3 today but we accept both for forward-compat.
    #[test]
    fn popup_filename_falls_back_to_mv2_browser_action() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        let manifest = r#"{
            "manifest_version": 2,
            "name": "EVE Vault",
            "browser_action": { "default_popup": "old.html" }
        }"#;
        std::fs::write(install_dir(tmp.path()).join("manifest.json"), manifest).expect("manifest");

        assert_eq!(popup_filename(tmp.path()).as_deref(), Some("old.html"));
    }

    /// No manifest = no popup, no panic. The wallet button then opens
    /// a regular new-tab page on first launch — that's the documented
    /// fallback in `commands::browser`.
    #[test]
    fn popup_filename_returns_none_without_manifest() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(popup_filename(tmp.path()).is_none());
    }

    /// `hex_encode` is used to render the downloaded ZIP's SHA-256
    /// for comparison against `checksums.txt`. Lower-case hex,
    /// fixed-width per byte. Bugs here mean every verified install
    /// silently fails — high-leverage to test.
    #[test]
    fn hex_encode_matches_known_vectors() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty_hash = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(
            hex_encode(&empty_hash),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hex_encode_pads_single_digit_bytes() {
        assert_eq!(hex_encode(&[0x00, 0x01, 0x0f, 0xff]), "00010fff");
    }

    /// Live: EVE Vault's most recent release MUST still contain
    /// the `eve-vault-chrome.zip` asset we hard-code. If upstream
    /// renames the artifact (e.g. `eve-vault-chrome-mv3.zip` or
    /// `evevault-chrome.zip` — both plausible refactors), Bifrost's
    /// install path silently breaks. Catches the drift before a
    /// user reports it.
    ///
    /// Skips on transport / rate-limit failure (same shape as the
    /// Brave contract test). Hard-fails if the asset is genuinely
    /// missing from the latest release.
    #[tokio::test]
    async fn live_evevault_latest_release_has_expected_asset() {
        if std::env::var("BIFROST_SKIP_UPSTREAM_TESTS").is_ok() {
            eprintln!("upstream-contract test: skipped (BIFROST_SKIP_UPSTREAM_TESTS set)");
            return;
        }
        match release_cache::fetch_release_json(REPO).await {
            Ok(body) => {
                let tag = body["tag_name"].as_str().unwrap_or("");
                assert!(
                    tag.starts_with('v'),
                    "EVE Vault tag should start with 'v', got {tag:?}"
                );
                let assets = body["assets"]
                    .as_array()
                    .expect("release JSON has assets array");
                let zip_found = assets
                    .iter()
                    .any(|a| a["name"].as_str() == Some(ASSET_NAME));
                assert!(
                    zip_found,
                    "EVE Vault matcher contract broken — latest release {tag} of \
                     {REPO} has no asset named {ASSET_NAME:?}. Upstream may have \
                     renamed the artifact. Listed asset names: {:?}",
                    assets
                        .iter()
                        .map(|a| a["name"].as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                );
                eprintln!("upstream-contract: EVE Vault OK — tag={tag} asset={ASSET_NAME}");
            }
            Err(e) => {
                let msg = e.to_string();
                if crate::release_cache::looks_like_transport_failure(&msg) {
                    eprintln!(
                        "upstream-contract test: skipped (transport / rate-limit): {msg}"
                    );
                    return;
                }
                panic!("EVE Vault latest-release fetch failed unexpectedly: {msg}");
            }
        }
    }
}
