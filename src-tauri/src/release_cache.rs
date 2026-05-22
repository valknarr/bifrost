//! Tiny in-process cache wrapping `fetch_latest_release()` calls so
//! `<app-data>/settings` open + `Check for updates` clicks don't burn
//! through GitHub's unauthenticated `/releases/latest` quota (60 req/hr
//! per IP). We previously fired six fetches every time the Settings
//! panel mounted (`status()` + `install()` calls back-to-back for three
//! modules × two reasons) — a handful of dev reloads was enough to get
//! a 403 for the rest of the hour.
//!
//! The cache is process-lifetime, holds the last successful response
//! per cache key, and serves it for [`TTL`] before refetching. A
//! `force_refresh = true` call bypasses the cache for the next fetch
//! (used by the "Check for updates" button) but still writes the fresh
//! result back so subsequent calls in the same TTL window are fast.
//!
//! Failures are NOT cached — if GitHub returns 403 the next call will
//! attempt a real fetch again. Otherwise a single failed request would
//! lock us out of retries for the whole TTL.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;

use crate::error::{BridgeError, Result};
use crate::http;

/// 30 minutes balances "user clicks Check for updates and gets a real
/// answer" with "stop spamming GitHub". The `Check for updates` button
/// always bypasses the cache, so this is only the floor for automatic
/// refetches.
const TTL: Duration = Duration::from_secs(30 * 60);

/// Internal entry — we serialize to JSON so we don't have to make the
/// cache generic over the value type. Each module passes its own key
/// and parses the JSON back to its own release struct.
#[derive(Clone)]
struct Entry {
    inserted_at: Instant,
    payload_json: String,
}

static CACHE: Lazy<Mutex<HashMap<&'static str, Entry>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Run `fetch` only if the cache is empty / stale for `key`, or
/// `force_refresh` is true. On any successful fetch the result is
/// written back. Failed fetches are not cached.
pub async fn fetch_with_cache<T, F, Fut>(
    key: &'static str,
    force_refresh: bool,
    fetch: F,
) -> Result<T>
where
    T: Serialize + DeserializeOwned + Clone,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    if !force_refresh {
        if let Some(cached) = read_cached::<T>(key) {
            return Ok(cached);
        }
    }
    let fresh = fetch().await?;
    write_cache(key, &fresh);
    Ok(fresh)
}

fn read_cached<T: DeserializeOwned>(key: &'static str) -> Option<T> {
    let guard = CACHE.lock().ok()?;
    let entry = guard.get(key)?;
    if entry.inserted_at.elapsed() >= TTL {
        return None;
    }
    serde_json::from_str(&entry.payload_json).ok()
}

fn write_cache<T: Serialize>(key: &'static str, value: &T) {
    let Ok(json) = serde_json::to_string(value) else {
        return;
    };
    if let Ok(mut guard) = CACHE.lock() {
        guard.insert(
            key,
            Entry {
                inserted_at: Instant::now(),
                payload_json: json,
            },
        );
    }
}

// -------------------------------------------------------------------------
// Shared GitHub-fetch helpers used by chromium / evevault / sandboxie_installer.
// Each module supplies its own asset-matcher predicate + release struct;
// the fetch + parse + error-formatting boilerplate lives here.
// -------------------------------------------------------------------------

/// GET `repos/{repo}/releases/latest` and return the parsed JSON. Each
/// caller knows which asset shape it expects, so we hand back the raw
/// `serde_json::Value` rather than imposing a one-size-fits-all struct.
///
/// Uses the process-wide HTTP client ([`http::client`]) with a short
/// 15 s per-request timeout — long enough for GitHub's API on a
/// reasonable connection, short enough that a transient hang doesn't
/// wedge the Settings panel indefinitely.
pub async fn fetch_release_json(repo: &str) -> Result<serde_json::Value> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");

    let resp = http::client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| BridgeError::Other(format!("GitHub API request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(BridgeError::Other(format!(
            "GitHub API returned HTTP {status}"
        )));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| BridgeError::Other(format!("GitHub API response parse failed: {e}")))
}

/// Translate a raw fetch-error string into something a non-technical
/// user can act on. Used by every status() function so the UI shows the
/// same actionable phrasing regardless of which module hit the wall.
pub fn friendly_fetch_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("403") {
        "GitHub rate-limit reached (60 requests/hour per IP). Bridge \
         caches results for 30 min — try Check for updates later."
            .to_string()
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "GitHub request timed out. Check your connection and retry.".to_string()
    } else {
        format!("Couldn't reach GitHub: {raw}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_error_recognises_403_rate_limit() {
        let out = friendly_fetch_error("GitHub API returned HTTP 403 Forbidden");
        assert!(out.contains("rate-limit"));
        assert!(out.contains("30 min"));
    }

    #[test]
    fn friendly_error_recognises_timeout() {
        let out = friendly_fetch_error("request timed out after 15s");
        assert!(out.contains("timed out"));
    }

    #[test]
    fn friendly_error_falls_back_to_raw_for_unknown() {
        let raw = "connection refused";
        let out = friendly_fetch_error(raw);
        assert!(out.contains(raw));
    }

    #[test]
    fn friendly_error_is_case_insensitive_on_match() {
        // "TIMEOUT" should still trigger the timeout branch even though
        // the lower-cased comparison is what we test against.
        let out = friendly_fetch_error("TIMEOUT during request");
        assert!(out.contains("timed out"));
    }

    // ---- cache behaviour --------------------------------------------
    //
    // The cache is what keeps Settings open without burning through
    // GitHub's 60-req/hour limit. These tests pin the contract
    // `fetch_with_cache` promises: hit within TTL, force_refresh
    // always refetches, failures are NOT cached (otherwise a single
    // 403 would lock retries for the whole 30-min window). Each
    // test uses a unique cache key + an `AtomicUsize` to count how
    // many times the underlying fetcher actually ran.

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Serialize, serde::Deserialize, PartialEq, Debug)]
    struct StubRelease {
        tag: String,
    }

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(fut)
    }

    /// Happy path: first call hits the fetcher, second call within
    /// TTL hits the cache. The fetcher counter going from 0 → 1 → 1
    /// is the cascading signal — if cache writes break, both calls
    /// run the fetcher; if cache reads break, ditto.
    #[test]
    fn cached_call_serves_second_request_without_refetch() {
        let calls = Arc::new(AtomicUsize::new(0));

        let calls_c = calls.clone();
        let first = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_cached_serves",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "v1".to_string(),
                    })
                }
            },
        ))
        .expect("first");

        let calls_c2 = calls.clone();
        let second = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_cached_serves",
            false,
            move || {
                let n = calls_c2.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "v2-fresh".to_string(),
                    })
                }
            },
        ))
        .expect("second");

        assert_eq!(first.tag, "v1");
        assert_eq!(
            second.tag, "v1",
            "second call must serve the cached v1, not the fresh v2"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "fetcher must run exactly once across both calls"
        );
    }

    /// `force_refresh = true` bypasses the cache even when there's a
    /// valid entry. Wired to the "Check for updates" button — clicking
    /// it must produce a real network call so the user sees genuine
    /// new versions when they're published.
    #[test]
    fn force_refresh_bypasses_cache() {
        let calls = Arc::new(AtomicUsize::new(0));

        // Seed the cache.
        let calls_c = calls.clone();
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_force_refresh",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "cached".to_string(),
                    })
                }
            },
        ));

        // Force-refresh should call the fetcher again AND replace
        // the cached value.
        let calls_c2 = calls.clone();
        let refreshed = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_force_refresh",
            true,
            move || {
                let n = calls_c2.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "fresh".to_string(),
                    })
                }
            },
        ))
        .expect("force refresh");

        assert_eq!(refreshed.tag, "fresh");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "force_refresh must run the fetcher even when cache is warm"
        );
    }

    /// A failed fetch is NOT cached. If GitHub returns 403 once, the
    /// next call retries instead of serving the failure from cache
    /// for 30 minutes — otherwise a single transient failure on app
    /// startup would lock retries until the next process restart.
    #[test]
    fn failed_fetch_does_not_get_cached() {
        let calls = Arc::new(AtomicUsize::new(0));

        // First call: fail.
        let calls_c = calls.clone();
        let first = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_failed_not_cached",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Err(BridgeError::Other("simulated 403".into()))
                }
            },
        ));
        assert!(first.is_err());

        // Second call (without force_refresh): MUST retry, not serve
        // a cached failure.
        let calls_c2 = calls.clone();
        let second = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_failed_not_cached",
            false,
            move || {
                let n = calls_c2.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "recovered".to_string(),
                    })
                }
            },
        ))
        .expect("second call recovers");

        assert_eq!(second.tag, "recovered");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "both attempts must reach the fetcher when the first failed"
        );
    }

    /// Different cache keys must not collide. Catches "I accidentally
    /// used the same key for chromium and sandboxie_installer" — both
    /// fetchers run independently, both values come back unmodified.
    #[test]
    fn cache_keys_are_isolated() {
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_iso_a",
            false,
            || async {
                Ok(StubRelease {
                    tag: "A".to_string(),
                })
            },
        ))
        .expect("a");
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_iso_b",
            false,
            || async {
                Ok(StubRelease {
                    tag: "B".to_string(),
                })
            },
        ))
        .expect("b");

        // Each key should re-serve its own cached value when called
        // again.
        let a_again = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_iso_a",
            false,
            || async {
                Ok(StubRelease {
                    tag: "should-not-run".to_string(),
                })
            },
        ))
        .expect("a again");
        let b_again = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_iso_b",
            false,
            || async {
                Ok(StubRelease {
                    tag: "should-not-run".to_string(),
                })
            },
        ))
        .expect("b again");

        assert_eq!(a_again.tag, "A");
        assert_eq!(b_again.tag, "B");
    }
}
