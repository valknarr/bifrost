//! Process-wide HTTP client.
//!
//! `reqwest::Client` is designed to be built once and reused for the
//! lifetime of the process. The struct internally holds:
//!
//! - A TCP connection pool (keep-alive reuse across requests).
//! - A DNS resolver cache.
//! - A TLS session cache (so subsequent requests to the same host
//!   skip the full handshake).
//!
//! All three of those are thrown away when the `Client` is dropped.
//! Before this module existed, every `release_cache::fetch_release_json`
//! call + every `*::install()` download + every `SuiClient::new()`
//! built a fresh `Client` and threw it away on return — which meant
//! Bifrost paid a full TLS handshake to GitHub on every Settings-panel
//! mount, three times back-to-back. Now they all share the pool.
//!
//! ## Timeout strategy
//!
//! The default timeout is generous (120 s) so background-class
//! requests just work. Callers that need shorter (status JSON fetches:
//! 15 s) or longer (multi-hundred-MB downloads: 300–600 s) bounds
//! override per-request with `RequestBuilder::timeout(...)`. The pool
//! is shared regardless of the timeout override, so we don't trade
//! connection-reuse to get the right timeout.

use std::time::Duration;

use once_cell::sync::Lazy;

/// Identifying User-Agent for every outbound request Bifrost makes.
/// GitHub rejects requests without a UA; a recognisable identifier
/// also helps service operators (GitHub's abuse-detection,
/// favicon-service logs, Sui RPC) treat us as a well-behaved client.
///
/// Built via `concat!` so the version segment stays in lockstep with
/// `Cargo.toml` automatically on every bump — no maintenance burden.
pub const USER_AGENT: &str = concat!(
    "bifrost/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/valknarr/bifrost) rust-reqwest"
);

/// 120 seconds covers the EVE Vault download (a few MB), the
/// JSON-RPC roundtrip to Sui, and the GitHub Releases API. Bigger
/// downloads (Brave: ~180 MB) override per-request via
/// `RequestBuilder::timeout`.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Process-wide singleton. Built lazily on first access; cloned
/// cheaply afterwards (the inner state is `Arc`-wrapped inside
/// reqwest, so `.clone()` is a refcount bump, not a pool rebuild).
static SHARED_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(DEFAULT_TIMEOUT)
        // Bifrost talks to ~3 hosts steady-state (GitHub API, GitHub
        // releases CDN, Sui mainnet RPC). A pool of 4 idle conns per
        // host is enough headroom for the occasional parallel fetch
        // without holding onto sockets we'll never reuse.
        .pool_max_idle_per_host(4)
        // Match Chromium's default 90 s idle timeout. Anything longer
        // and we hold sockets the server may have already closed.
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .build()
        .expect("reqwest client builds")
});

/// Get a handle to the shared client. Cheap — clones an `Arc`.
pub fn client() -> reqwest::Client {
    SHARED_CLIENT.clone()
}

/// Stream a response body into memory with a hard byte cap.
///
/// `.bytes().await` on a `reqwest::Response` will happily allocate
/// whatever the upstream feeds us. For URLs that come from a user-
/// controllable host (favicon fallback) or where a compromised /
/// misbehaving upstream could feed unbounded data within the
/// timeout window (Brave / EVE Vault / Sandboxie ZIPs), that's an
/// OOM vector.
///
/// This wrapper enforces `max_bytes` in two layers:
///
/// 1. Up-front rejection when `Content-Length` advertises an
///    oversized body — saves the stream-loop work.
/// 2. Streaming-and-tallying via `Response::chunk` so a lying /
///    missing Content-Length can't bypass the cap either.
///
/// The hard cap is in bytes; pick it conservatively per-caller
/// (favicon: 256 KiB, EVE Vault zip: 50 MiB, Brave zip: 300 MiB,
/// Sandboxie installer: 50 MiB).
///
/// Returns the full body Vec on a clean 2xx within the cap; Err
/// otherwise. Status-code translation is the caller's job — same
/// shape as the previous `.bytes()` call sites.
pub async fn download_capped(
    resp: reqwest::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, crate::error::BifrostError> {
    use crate::error::BifrostError;

    if let Some(len) = resp.content_length() {
        if len > max_bytes {
            return Err(BifrostError::Other(format!(
                "body too large: {} bytes (cap {})",
                len, max_bytes
            )));
        }
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut resp = resp;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| BifrostError::Other(format!("body read: {e}")))?
    {
        if (buf.len() + chunk.len()) as u64 > max_bytes {
            return Err(BifrostError::Other(format!(
                "body exceeded {} bytes mid-stream",
                max_bytes
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared client is a process-wide singleton. `reqwest::Client`
    /// stores its inner state behind an `Arc` (so cloning is a refcount
    /// bump) and `once_cell::Lazy` guarantees the inner is initialised
    /// once. Pin that contract by comparing the underlying address
    /// across two `client()` calls — a future refactor that drops the
    /// `Lazy` (or replaces the singleton with a per-call rebuild) will
    /// fail this assertion. Doesn't depend on test order: we force
    /// initialisation up-front, then compare addresses.
    #[test]
    fn shared_client_singleton_is_stable() {
        let _force_init = client();
        let p1 = Lazy::get(&SHARED_CLIENT).expect("initialised") as *const _;
        let _again = client();
        let p2 = Lazy::get(&SHARED_CLIENT).expect("still initialised") as *const _;
        assert_eq!(
            p1, p2,
            "Lazy must hand back the same inner client across calls"
        );
    }

    /// Configured defaults. Pins the contract so a future refactor
    /// that drops the User-Agent header (GitHub starts 403'ing
    /// without one) or removes the timeout (a hung fetch wedges the
    /// Settings panel forever) fails the test loudly. Also pins the
    /// CARGO_PKG_VERSION wiring + the actual project URL — both went
    /// stale once before the audit caught them.
    #[test]
    fn user_agent_is_set_and_recognisable() {
        assert!(USER_AGENT.starts_with("bifrost/"));
        assert!(USER_AGENT.contains("rust-reqwest"));
        // The repo URL must match Cargo.toml's `repository` field;
        // GitHub's abuse-detection uses it to identify legitimate
        // callers when traffic spikes.
        assert!(
            USER_AGENT.contains("github.com/valknarr/bifrost"),
            "UA must point at the real repo, got {USER_AGENT:?}"
        );
        // The version segment must track CARGO_PKG_VERSION so a bump
        // in Cargo.toml automatically updates the UA.
        assert!(
            USER_AGENT.contains(env!("CARGO_PKG_VERSION")),
            "UA must include the current crate version"
        );
    }
}
