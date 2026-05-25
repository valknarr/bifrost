//! Favicon fetch + on-disk cache for companion-site icons.
//!
//! Why this exists in Rust rather than as a frontend `<img>` to a
//! third-party service:
//!
//!   - The shared [`crate::http::client`] sends a
//!     `bifrost/<CARGO_PKG_VERSION>` User-Agent (built at compile
//!     time in `http.rs`), so the favicon-service operator (Google)
//!     sees Bifrost as a distinct caller in their logs rather than
//!     anonymous Edge WebView2 traffic.
//!   - Caching: favicons rarely change. We keep them under
//!     `<app-data>/favicons/<host>.png` with a 7-day TTL, so a typical
//!     boot is pure disk reads — no network at all.
//!
//! Negative-cache sentinel: failed fetches write a **0-byte file**
//! at the same path. Subsequent `fetch_data_url` calls within the TTL
//! see the empty file and return None without re-hitting upstream.
//! Without this the log fills with "no favicon available for X" for
//! every config refresh — the user sees stale glyphs and spam logs.
//! The Settings "refresh" affordance can use `force=true` later to
//! bypass the sentinel; for the lean re-introduction we just rely
//! on the 7-day TTL to retry automatically.
//!
//! Two-step upstream: Google's favicon service first (broad coverage,
//! 64 px PNGs), then a direct `<host>/favicon.ico` if Google 404s
//! (catches small / community sites Google hasn't indexed).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::error::{BifrostError, Result};
use crate::http;

/// Size in px to request from Google's favicon service.
const FAVICON_SIZE: u32 = 64;

/// Per-request timeout. Favicons are tiny — anything longer almost
/// certainly means the host hung the connection.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a cached file (success OR sentinel) is considered fresh.
/// Weekly retry catches the occasional rebrand without hammering
/// upstreams on every config refresh.
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn cache_dir(app_data: &Path) -> PathBuf {
    app_data.join("favicons")
}

/// Filesystem-safe filename for the given host. Hostnames are already
/// mostly safe; we sanitise just-in-case so a weird URL can't escape
/// the cache dir.
fn cache_path(app_data: &Path, host: &str) -> PathBuf {
    let sanitized: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cache_dir(app_data).join(format!("{sanitized}.png"))
}

fn is_fresh(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    elapsed < CACHE_TTL
}

fn parse_host(url: &str) -> Result<String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|e| BifrostError::Other(format!("invalid URL: {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| BifrostError::Other("URL has no host".into()))?
        .to_string();
    Ok(host)
}

/// Cap on a single favicon body. 256 KiB is two orders of magnitude
/// larger than the largest real-world favicon (~10–20 KiB for a
/// 64 × 64 PNG with palette + alpha), so legitimate icons fit
/// comfortably while a malicious or misbehaving host can't feed us
/// a multi-GB body that would OOM the Tauri process.
///
/// Favicon URLs come from user-added companion sites — Google's
/// `s2/favicons` endpoint sanitises the upstream icon for us, but
/// the direct-host fallback (`https://{host}/favicon.ico`) talks to
/// whatever the user typed, with the bifrost user-agent. Capping
/// the response size at the read boundary is the cheap defence.
const MAX_FAVICON_BYTES: u64 = 256 * 1024;

/// Pull a single favicon URL via the shared HTTP client. Returns the
/// raw bytes on a 2xx, Err otherwise. Used twice by `fetch_cached`
/// (Google first, direct fallback second).
///
/// Body cap enforced via the shared `http::download_capped` helper
/// (same code path used by the Brave / EVE Vault / Sandboxie ZIP
/// downloads). An attacker-controlled favicon host can't drown the
/// process with an unbounded response.
async fn try_fetch(url: &str) -> Result<Vec<u8>> {
    let resp = http::client()
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| BifrostError::Other(format!("transport: {e}")))?;
    if !resp.status().is_success() {
        return Err(BifrostError::Other(format!("HTTP {}", resp.status())));
    }
    http::download_capped(resp, MAX_FAVICON_BYTES).await
}

/// Ensure the favicon for `url` is cached locally. Returns the cache
/// path (which may be a real PNG or a 0-byte sentinel — callers
/// distinguish by reading the file). When `force` is true the
/// freshness check is skipped so a manual retry can re-hit upstream.
async fn fetch_cached(app_data: &Path, url: &str, force: bool) -> Result<PathBuf> {
    let host = parse_host(url)?;
    let path = cache_path(app_data, &host);

    if !force && is_fresh(&path) {
        return Ok(path);
    }

    let google_url = format!(
        "https://www.google.com/s2/favicons?domain={host}&sz={FAVICON_SIZE}&app=bifrost&v={ver}",
        ver = env!("CARGO_PKG_VERSION"),
    );
    let direct_url = format!("https://{host}/favicon.ico");

    let bytes_result = match try_fetch(&google_url).await {
        Ok(bytes) => Ok(bytes),
        Err(google_err) => try_fetch(&direct_url).await.map_err(|direct_err| {
            BifrostError::Other(format!(
                "favicon fetch failed (google: {google_err}; direct: {direct_err})"
            ))
        }),
    };

    match bytes_result {
        Ok(bytes) if !bytes.is_empty() => {
            std::fs::create_dir_all(cache_dir(app_data))?;
            std::fs::write(&path, &bytes)?;
            Ok(path)
        }
        _ => {
            // No favicon retrievable. Either fall back to an existing
            // non-empty cache (stale-while-error) or write a 0-byte
            // sentinel so we don't hammer upstream on every refresh.
            let has_real_stale = std::fs::metadata(&path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);
            if has_real_stale {
                Ok(path)
            } else {
                std::fs::create_dir_all(cache_dir(app_data))?;
                std::fs::write(&path, [])?;
                tracing::debug!("favicon: negative-cached for {host}");
                Ok(path)
            }
        }
    }
}

/// Frontend-facing helper: produce a `data:image/png;base64,…` URL
/// for the favicon, or None when no favicon is available (host
/// returned nothing AND no stale cache exists). The 0-byte sentinel
/// surface AS None — callers render the user-defined letter glyph.
pub async fn fetch_data_url(app_data: &Path, url: &str) -> Option<String> {
    let path = fetch_cached(app_data, url, false).await.ok()?;
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(format!("data:image/png;base64,{}", BASE64.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn cache_path_stays_inside_cache_dir() {
        let tmp = TempDir::new().expect("tempdir");
        // A pathological "hostname" with separators must NOT escape
        // the cache dir.
        let path = cache_path(tmp.path(), "evil/../escape\\nested");
        assert!(path.starts_with(cache_dir(tmp.path())));
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(!filename.contains('/'));
        assert!(!filename.contains('\\'));
        assert!(filename.ends_with(".png"));
    }

    #[test]
    fn cache_path_preserves_legitimate_host_chars() {
        let tmp = TempDir::new().expect("tempdir");
        let path = cache_path(tmp.path(), "sub-domain.example.com");
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "sub-domain.example.com.png"
        );
    }

    #[test]
    fn parse_host_rejects_invalid_urls() {
        assert!(parse_host("not-a-url").is_err());
        assert!(parse_host("").is_err());
        // `file:` URLs have no `host_str`.
        assert!(parse_host("file:///etc/passwd").is_err());
    }

    #[test]
    fn parse_host_accepts_well_formed_urls() {
        assert_eq!(parse_host("https://ef-map.com/").unwrap(), "ef-map.com");
        assert_eq!(
            parse_host("https://www.example.com/path?q=1").unwrap(),
            "www.example.com"
        );
    }

    #[test]
    fn fresh_is_false_for_missing_file() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(!is_fresh(&tmp.path().join("missing.png")));
    }

    #[test]
    fn fresh_is_true_for_just_written_file() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("recent.png");
        std::fs::write(&path, b"x").expect("write");
        assert!(is_fresh(&path));
    }

    /// 0-byte sentinel from a previous failed fetch must surface as
    /// None — empty bytes are not a valid PNG and rendering them as a
    /// data URL would draw a broken-image icon.
    #[tokio::test]
    async fn data_url_is_none_for_zero_byte_sentinel() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(cache_dir(tmp.path())).expect("mkdir");
        let path = cache_path(tmp.path(), "no-favicon.example");
        std::fs::write(&path, []).expect("sentinel");
        assert!(is_fresh(&path), "fresh sentinel keeps the cache hot");
        let result = fetch_data_url(tmp.path(), "https://no-favicon.example/").await;
        assert!(result.is_none(), "sentinel must surface as None");
    }
}
