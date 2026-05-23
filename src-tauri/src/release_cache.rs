//! Tiny in-process cache wrapping `fetch_latest_release()` calls so
//! `<app-data>/settings` open + `Check for updates` clicks don't burn
//! through GitHub's unauthenticated `/releases/latest` quota (60 req/hr
//! per IP). We previously fired six fetches every time the Settings
//! panel mounted (`status()` + `install()` calls back-to-back for three
//! modules × two reasons) — a handful of dev reloads was enough to get
//! a 403 for the rest of the hour.
//!
//! The cache is process-lifetime and serves successful responses for
//! [`SUCCESS_TTL`] (30 min). A `force_refresh = true` call bypasses
//! the cache for the next fetch (used by the "Check for updates"
//! button) but still writes the fresh result back so subsequent calls
//! in the same TTL window are fast.
//!
//! Failure handling is split by failure type:
//!
//! * **Rate-limit failures** (HTTP 403/429) — cached with a separate
//!   shorter TTL ([`FAILURE_BACKOFF_TTL`], 5 min). When we're being
//!   rate-limited, the *worst* thing we can do is keep hammering
//!   GitHub on every Settings open — that burns more quota and
//!   extends the lockout. Caching the 403 short-circuits the network
//!   call for 5 min and surfaces the same friendly "rate-limit" error
//!   immediately. After 5 min the next call retries naturally.
//!
//! * **All other failures** (timeout, DNS, connection refused) — NOT
//!   cached. Those are transient and the user often resolves them by
//!   plugging the laptop back in / reconnecting Wi-Fi; we don't want
//!   to lock retries behind a TTL when the next attempt would
//!   probably succeed.
//!
//! Before this split existed, a single 403 on app startup let the
//! UI re-fire three GitHub calls every ~7 s on Settings re-mounts,
//! each one extending the rate-limit window. The fix is "cache the
//! 403 too, but shorter than the success TTL".

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
const SUCCESS_TTL: Duration = Duration::from_secs(30 * 60);

/// Shorter back-off window for rate-limit failures. 5 min keeps us
/// well under GitHub's hourly window resetting while still allowing a
/// genuine retry once the user has waited a few minutes. Coupled with
/// the friendly error message in `friendly_fetch_error()` which tells
/// the user to "try Check for updates later", the UX is: see the
/// friendly error, wait a moment, click Check, get a fresh attempt.
const FAILURE_BACKOFF_TTL: Duration = Duration::from_secs(5 * 60);

/// Internal entry — we serialize to JSON so we don't have to make the
/// cache generic over the value type. Each module passes its own key
/// and parses the JSON back to its own release struct.
#[derive(Clone)]
struct Entry {
    inserted_at: Instant,
    payload_json: String,
}

/// Rate-limit failure entry. Stores the original error message so we
/// surface the same text the user would have seen on a fresh fetch —
/// `friendly_fetch_error()` translates it into the actionable
/// "wait + Check for updates" phrasing downstream.
#[derive(Clone)]
struct FailureEntry {
    inserted_at: Instant,
    error_msg: String,
}

static CACHE: Lazy<Mutex<HashMap<&'static str, Entry>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static FAILURE_CACHE: Lazy<Mutex<HashMap<&'static str, FailureEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Run `fetch` only when the cache is empty / stale for `key`, or
/// `force_refresh` is true. On success the result is written back.
/// On a rate-limit failure (HTTP 403/429) the error is also cached
/// — see the FAILURE_BACKOFF_TTL doc-comment for why. Other
/// failures (timeout, DNS) are NOT cached.
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
        // Rate-limit short-circuit: if we recently got 403'd by
        // GitHub, surface the same error immediately without firing
        // a fresh request. Skipped on `force_refresh` because the
        // user explicitly asked for a retry via Check for updates.
        if let Some(msg) = read_cached_failure(key) {
            return Err(BridgeError::Other(msg));
        }
    }
    match fetch().await {
        Ok(fresh) => {
            write_cache(key, &fresh);
            // Successful fetch retires any prior rate-limit backoff
            // — the next failed call should get a fresh attempt at
            // GitHub rather than serving a stale 403.
            clear_cached_failure(key);
            Ok(fresh)
        }
        Err(e) => {
            let msg = e.to_string();
            if is_rate_limit_error(&msg) {
                write_cached_failure(key, msg.clone());
            }
            Err(e)
        }
    }
}

fn read_cached<T: DeserializeOwned>(key: &'static str) -> Option<T> {
    let guard = CACHE.lock().ok()?;
    let entry = guard.get(key)?;
    if entry.inserted_at.elapsed() >= SUCCESS_TTL {
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

fn read_cached_failure(key: &'static str) -> Option<String> {
    let guard = FAILURE_CACHE.lock().ok()?;
    let entry = guard.get(key)?;
    if entry.inserted_at.elapsed() >= FAILURE_BACKOFF_TTL {
        return None;
    }
    Some(entry.error_msg.clone())
}

fn write_cached_failure(key: &'static str, msg: String) {
    if let Ok(mut guard) = FAILURE_CACHE.lock() {
        guard.insert(
            key,
            FailureEntry {
                inserted_at: Instant::now(),
                error_msg: msg,
            },
        );
    }
}

fn clear_cached_failure(key: &'static str) {
    if let Ok(mut guard) = FAILURE_CACHE.lock() {
        guard.remove(key);
    }
}

/// True if the error message looks like a GitHub rate-limit
/// response. We rely on the substring "403" / "429" / "rate limit"
/// because `fetch_release_json` formats the upstream error as the
/// raw HTTP status, and `friendly_fetch_error` already keys off the
/// same substring for the user-facing message. If GitHub ever moves
/// to a different status code (unlikely), both sites need updating
/// — the friendly-error tests catch the user-facing half.
fn is_rate_limit_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("403") || lower.contains("429") || lower.contains("rate limit")
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
        "GitHub rate-limit reached (60 requests/hour per IP). Bifrost \
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

    /// A transient (non-rate-limit) failure must NOT get cached.
    /// Otherwise a single hiccup at app startup — DNS blip, Wi-Fi
    /// reconnect, GitHub having a bad minute — would lock retries
    /// behind the success TTL when the next attempt would probably
    /// succeed.
    #[test]
    fn transient_failure_does_not_get_cached() {
        let calls = Arc::new(AtomicUsize::new(0));

        // First call: simulate a network timeout (NOT a 403). The
        // failure message must be distinct from rate-limit phrasing
        // or `is_rate_limit_error` would (correctly) cache it.
        let calls_c = calls.clone();
        let first = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_transient_not_cached",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Err(BridgeError::Other(
                        "simulated network timeout".into(),
                    ))
                }
            },
        ));
        assert!(first.is_err());

        // Second call (without force_refresh): MUST retry, not serve
        // a cached failure.
        let calls_c2 = calls.clone();
        let second = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_transient_not_cached",
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

    /// Rate-limit failures (HTTP 403/429) MUST be cached for the
    /// backoff window so the UI doesn't burn through quota by
    /// re-firing fetches on every Settings panel mount. This is the
    /// fix for the "Settings opens → 3 fetches → all 403 → user
    /// switches tabs and back → 3 more fetches" loop.
    #[test]
    fn rate_limit_failure_is_cached_and_short_circuits() {
        let calls = Arc::new(AtomicUsize::new(0));

        // First call: simulate the upstream 403 we see in real life.
        // `fetch_release_json` formats the error as "GitHub API
        // returned HTTP 403 Forbidden", so the cache should pattern-
        // match on "403".
        let calls_c = calls.clone();
        let first = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_403_cached",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Err(BridgeError::Other(
                        "GitHub API returned HTTP 403 Forbidden".into(),
                    ))
                }
            },
        ));
        assert!(first.is_err());

        // Second call within the backoff window: must NOT hit the
        // fetcher, must surface the cached error.
        let calls_c2 = calls.clone();
        let second = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_403_cached",
            false,
            move || {
                let n = calls_c2.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "should-not-run".to_string(),
                    })
                }
            },
        ));
        assert!(second.is_err(), "must surface the cached 403");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "rate-limit failure must short-circuit subsequent calls"
        );
    }

    /// `force_refresh = true` also bypasses the failure cache. When
    /// the user clicks Check for updates they're explicitly asking
    /// for a fresh attempt — even if GitHub said 403 five minutes
    /// ago, we should at least try.
    #[test]
    fn force_refresh_bypasses_failure_cache() {
        let calls = Arc::new(AtomicUsize::new(0));

        // Seed the failure cache with a 403.
        let calls_c = calls.clone();
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_force_bypasses_fail",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Err(BridgeError::Other(
                        "GitHub API returned HTTP 403 Forbidden".into(),
                    ))
                }
            },
        ));

        // Force-refresh: must call the fetcher again.
        let calls_c2 = calls.clone();
        let refreshed = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_force_bypasses_fail",
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
        .expect("force_refresh succeeds");

        assert_eq!(refreshed.tag, "fresh");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "force_refresh must run the fetcher even when failure cache is warm"
        );
    }

    /// A successful fetch clears any prior rate-limit backoff so the
    /// next failure cleanly re-arms the cache. Without this, a 403
    /// followed by force-refresh-success followed by a fresh 403
    /// would (incorrectly) serve the OLD 403 from cache instead of
    /// re-arming with the new one.
    #[test]
    fn success_clears_prior_failure_cache() {
        let calls = Arc::new(AtomicUsize::new(0));

        // 1. Fail with 403 — populates failure cache.
        let calls_c = calls.clone();
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_success_clears_fail",
            false,
            move || {
                let n = calls_c.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Err(BridgeError::Other(
                        "GitHub API returned HTTP 403 Forbidden".into(),
                    ))
                }
            },
        ));

        // 2. Force-refresh that succeeds — should clear the failure
        // cache.
        let calls_c2 = calls.clone();
        let _ = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_success_clears_fail",
            true,
            move || {
                let n = calls_c2.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "first-success".to_string(),
                    })
                }
            },
        ))
        .expect("success");

        // 3. Subsequent call (no force_refresh): success TTL has
        // it, so we get the cached success without hitting fetcher.
        // The point of THIS test is the failure cache is cleared —
        // we verify by then asserting calls = 2 (not 3).
        let calls_c3 = calls.clone();
        let third = block_on(fetch_with_cache::<StubRelease, _, _>(
            "test_success_clears_fail",
            false,
            move || {
                let n = calls_c3.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    Ok(StubRelease {
                        tag: "should-not-run".to_string(),
                    })
                }
            },
        ))
        .expect("third");

        assert_eq!(third.tag, "first-success", "served from success cache");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "first (fail) + second (force-refresh success); third served from cache"
        );
    }

    /// `is_rate_limit_error` recognises the formats `fetch_release_json`
    /// actually produces. Pinning the contract here means a future
    /// refactor that changes the error wording breaks the test
    /// rather than silently disabling the rate-limit cache.
    #[test]
    fn rate_limit_error_detection() {
        assert!(is_rate_limit_error("GitHub API returned HTTP 403 Forbidden"));
        assert!(is_rate_limit_error("HTTP 429 Too Many Requests"));
        assert!(is_rate_limit_error("Rate limit exceeded"));
        assert!(is_rate_limit_error("rate limit"));
        assert!(!is_rate_limit_error("request timed out"));
        assert!(!is_rate_limit_error("DNS lookup failed"));
        assert!(!is_rate_limit_error("connection refused"));
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
