//! Portable browser downloader & installer.
//!
//! Mirrors the [`evevault`](crate::evevault) module â€” fetches the latest
//! Windows x64 portable ZIP of [Brave Browser] from GitHub Releases,
//! extracts it to `<app-data>/chromium/current/`, and exposes the
//! resulting browser executable path via [`chrome_exe_path`].
//!
//! We've cycled through portable Chromium distros to find one that
//! handles EVE Vault's OAuth flow:
//!   - Ungoogled Chromium â†’ strips Google identity plumbing,
//!     `chromiumapp.org` redirect broke FusionAuth.
//!   - Thorium AVX2 â†’ process crashed on launch (CPU compatibility or
//!     fork-specific issue), file-lock errors on retry.
//!   - **Brave** â†’ real, signed Chromium fork with full identity plumbing
//!     intact. Bigger than UCG (220 MB) but actually works.
//!
//! [Brave Browser]: https://github.com/brave/brave-browser

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{BifrostError, Result};
use crate::http;
use crate::release_cache;

const REPO: &str = "brave/brave-browser";

/// Asset matcher â€” Brave publishes both portable binaries and installers
/// in the same release; we want only the Windows x64 portable ZIP and
/// must reject the symbols ZIP (1.4 GB of pdb files) plus the
/// installer EXEs.
fn is_portable_zip(name: &str) -> bool {
    name.starts_with("brave-v") && name.ends_with("-win32-x64.zip") && !name.contains("symbols")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_zip_matcher_accepts_canonical_release() {
        assert!(is_portable_zip("brave-v1.90.124-win32-x64.zip"));
    }

    #[test]
    fn portable_zip_matcher_rejects_symbols_zip() {
        assert!(!is_portable_zip("brave-v1.90.124-win32-x64-symbols.zip"));
    }

    #[test]
    fn portable_zip_matcher_rejects_installer_exe() {
        assert!(!is_portable_zip("BraveBrowserStandaloneSetup.exe"));
    }

    #[test]
    fn portable_zip_matcher_rejects_arm64_and_x86() {
        assert!(!is_portable_zip("brave-v1.90.124-win32-ia32.zip"));
        assert!(!is_portable_zip("brave-v1.90.124-win32-arm64.zip"));
    }

    #[test]
    fn portable_zip_matcher_rejects_macos_and_linux() {
        assert!(!is_portable_zip("brave-v1.90.124-darwin-x64.zip"));
        assert!(!is_portable_zip("brave-v1.90.124-linux-x64.zip"));
    }

    // ---- version-marker round-trip ----------------------------------
    //
    // Cascading scenario: install completes â†’ marker is written â†’
    // `read_installed_version` reports the same tag â†’ uninstall â†’
    // `read_installed_version` reports None. Each of these tests
    // exercises a real on-disk path so a regression in `install_dir`,
    // `version_marker` (the private helper), or the `uninstall`
    // cleanup surfaces here before the Settings panel starts lying
    // about what's installed.

    use tempfile::TempDir;

    /// Fresh app-data dir: nothing installed, `read_installed_version`
    /// returns `None`. This is the first-launch contract â€” the
    /// Settings row shows "â—‹ Not installed" only if we trust this.
    #[test]
    fn read_installed_version_returns_none_on_empty_app_data() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(read_installed_version(tmp.path()).is_none());
    }

    /// Simulate a successful install: create the install dir, write a
    /// version marker, verify read_installed_version reports the tag.
    #[test]
    fn version_marker_roundtrips() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "v1.90.124")
            .expect("marker");

        assert_eq!(
            read_installed_version(tmp.path()).as_deref(),
            Some("v1.90.124")
        );
    }

    /// Edge case: marker file exists but is empty (e.g. a half-written
    /// install crashed). Treat as "no version" so the Settings UI
    /// offers Install rather than reporting a phantom version.
    #[test]
    fn empty_marker_file_reads_as_none() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "").expect("empty marker");
        assert!(read_installed_version(tmp.path()).is_none());
    }

    /// Marker leading/trailing whitespace must be trimmed â€” text
    /// editors love to add a trailing newline.
    #[test]
    fn marker_whitespace_is_trimmed() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "  v1.0\n")
            .expect("marker");
        assert_eq!(read_installed_version(tmp.path()).as_deref(), Some("v1.0"));
    }

    /// Uninstall removes the install dir (and therefore the marker
    /// inside it). The Settings UI relies on this â€” uninstall must
    /// cause `read_installed_version` to flip to None on the very
    /// next call, no app restart needed.
    #[test]
    fn uninstall_flips_marker_to_none() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(install_dir(tmp.path())).expect("install dir");
        std::fs::write(install_dir(tmp.path()).join(".bifrost-version"), "v1.0").expect("marker");
        assert_eq!(read_installed_version(tmp.path()).as_deref(), Some("v1.0"));

        uninstall(tmp.path()).expect("uninstall");
        assert!(read_installed_version(tmp.path()).is_none());
        assert!(!install_dir(tmp.path()).exists());
    }

    /// Uninstall on an empty dir is a no-op (safe to call from the
    /// Settings UI even when nothing's installed).
    #[test]
    fn uninstall_is_safe_when_nothing_installed() {
        let tmp = TempDir::new().expect("tempdir");
        uninstall(tmp.path()).expect("uninstall is no-op");
    }
}

/// Resolved metadata for the latest Brave Windows-x64 portable release.
/// Populated from the GitHub Releases API by [`fetch_latest_release`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromiumRelease {
    pub tag: String,
    pub zip_url: String,
    pub published_at: Option<String>,
    pub size_bytes: u64,
}

/// Live status the Settings panel renders for the portable browser
/// row â€” combines on-disk install detection with the latest-known
/// upstream release so the UI can offer Install / Update / Reinstall.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromiumStatus {
    pub latest_version: Option<String>,
    pub installed_version: Option<String>,
    pub install_dir: Option<String>,
    pub chrome_exe: Option<String>,
    pub update_available: bool,
    /// Approximate download size for the latest release, in bytes. Helpful
    /// so the UI can warn users about the ~180 MB transfer.
    pub latest_size_bytes: Option<u64>,
    /// Human-readable error if the GitHub fetch failed (rate-limit,
    /// network, etc.). UI surfaces this so users understand why the
    /// `latest_version` field is null.
    pub latest_error: Option<String>,
}

/// Query the GitHub Releases API for the latest Brave release and
/// pick the Windows-x64 portable ZIP asset. Returns the metadata only;
/// no download yet. Cached upstream via [`release_cache::fetch_with_cache`]
/// so passive Settings refreshes don't burn through GitHub's rate limit.
pub async fn fetch_latest_release() -> Result<ChromiumRelease> {
    let body = release_cache::fetch_release_json(REPO).await?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| BifrostError::Other("release JSON has no tag_name".into()))?
        .to_string();
    let assets = body["assets"]
        .as_array()
        .ok_or_else(|| BifrostError::Other("release JSON has no assets array".into()))?;
    let asset = assets
        .iter()
        .find(|a| is_portable_zip(a["name"].as_str().unwrap_or_default()))
        .ok_or_else(|| {
            BifrostError::Other(format!("release {tag} has no win32-x64 portable ZIP"))
        })?;

    Ok(ChromiumRelease {
        tag,
        zip_url: asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| BifrostError::Other("asset has no browser_download_url".into()))?
            .to_string(),
        size_bytes: asset["size"].as_u64().unwrap_or(0),
        published_at: body["published_at"].as_str().map(|s| s.to_string()),
    })
}

/// Where the portable browser lives on disk. Stable path so upgrades
/// replace in-place and so `pilot.browser_profile_dir` doesn't have
/// to track the install location.
pub fn install_dir(app_data: &Path) -> PathBuf {
    app_data.join("chromium").join("current")
}

/// Remove the portable browser install in its entirety. Safe no-op when
/// nothing's installed. We can do this with a plain
/// `remove_dir_all` because the portable build doesn't drop anything
/// outside this directory â€” no kernel driver, no service, no registry
/// hooks. Callers must ensure no Brave processes are holding handles
/// (the Settings UI does this only when no pilots have a browser
/// open).
pub fn uninstall(app_data: &Path) -> Result<()> {
    let dir = install_dir(app_data);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        tracing::info!("chromium: uninstalled (removed {})", dir.display());
    }
    Ok(())
}

fn version_marker(app_data: &Path) -> PathBuf {
    install_dir(app_data).join(".bifrost-version")
}

/// Read the tag Bifrost last installed (the contents of the
/// `.bifrost-version` marker file in [`install_dir`]). Returns `None`
/// when nothing is installed.
pub fn read_installed_version(app_data: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(version_marker(app_data)).ok()?;
    let trimmed = txt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Locate the browser executable inside the install directory. Bifrost
/// currently bundles Brave (`brave.exe`) but we keep `chrome.exe` as a
/// fallback so a future swap to vanilla Chromium / Chrome wouldn't
/// require touching this file. We check both names at the root and
/// one directory deep (some installers wrap the binary in a
/// version-named folder).
pub fn chrome_exe_path(app_data: &Path) -> Option<PathBuf> {
    const EXE_NAMES: &[&str] = &["brave.exe", "chrome.exe"];

    let root = install_dir(app_data);
    for name in EXE_NAMES {
        let direct = root.join(name);
        if direct.exists() {
            return Some(direct);
        }
    }
    // Walk one level deep looking for a folder containing the exe.
    for entry in std::fs::read_dir(&root).ok()?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        for name in EXE_NAMES {
            let candidate = entry.path().join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Combined live status for the Settings panel. `force_refresh = true`
/// bypasses the 30-min release cache for one call (wired to the "Check
/// for updates" button); normal mounts pass `false`.
pub async fn status(app_data: &Path, force_refresh: bool) -> ChromiumStatus {
    let installed_version = read_installed_version(app_data);
    let install_dir_str = if installed_version.is_some() {
        Some(install_dir(app_data).to_string_lossy().into_owned())
    } else {
        None
    };
    let chrome_exe_str = chrome_exe_path(app_data).map(|p| p.to_string_lossy().into_owned());

    let cached = release_cache::fetch_with_cache::<ChromiumRelease, _, _>(
        "chromium",
        force_refresh,
        fetch_latest_release,
    )
    .await;
    let (latest_version, latest_size_bytes, latest_error) = match cached {
        Ok(r) => (Some(r.tag), Some(r.size_bytes), None),
        Err(e) => {
            let msg = release_cache::friendly_fetch_error(&e.to_string());
            tracing::warn!("chromium: fetch_latest_release failed: {e}");
            (None, None, Some(msg))
        }
    };

    let update_available = match (&installed_version, &latest_version) {
        (Some(i), Some(l)) => i != l,
        (None, Some(_)) => true,
        _ => false,
    };

    ChromiumStatus {
        latest_version,
        installed_version,
        install_dir: install_dir_str,
        chrome_exe: chrome_exe_str,
        update_available,
        latest_size_bytes,
        latest_error,
    }
}

/// Download and extract the given Brave release into [`install_dir`],
/// replacing any existing install in place. Writes a `.bifrost-version`
/// marker on success so subsequent [`read_installed_version`] calls
/// can report the tag.
pub async fn install(app_data: &Path, release: &ChromiumRelease) -> Result<()> {
    tracing::info!(
        "chromium: downloading {} ({} MB)",
        release.tag,
        release.size_bytes / 1_048_576
    );
    // 600 s timeout for this specific request â€” it's a ~180 MB
    // download. Pool + DNS + TLS state is reused from prior fetches
    // via the shared client.
    let zip_bytes = http::client()
        .get(&release.zip_url)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| BifrostError::Other(format!("chromium zip download failed: {e}")))?
        .bytes()
        .await
        .map_err(|e| BifrostError::Other(format!("chromium zip body read failed: {e}")))?;

    let target = install_dir(app_data);
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::create_dir_all(&target)?;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes.as_ref()))
        .map_err(|e| BifrostError::Other(format!("chromium zip parse failed: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| BifrostError::Other(format!("chromium zip entry {i}: {e}")))?;
        let Some(name) = entry.enclosed_name() else {
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
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| BifrostError::Other(format!("chromium zip entry read: {e}")))?;
        std::fs::write(&out_path, &buf)?;
    }

    std::fs::write(version_marker(app_data), &release.tag)?;
    tracing::info!(
        "chromium: installed {} into {}",
        release.tag,
        target.display()
    );
    Ok(())
}
