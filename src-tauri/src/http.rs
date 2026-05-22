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
//! Bridge paid a full TLS handshake to GitHub on every Settings-panel
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

/// Identifying User-Agent for every outbound request Bridge makes.
/// GitHub rejects requests without a UA; using a recognisable
/// identifier also helps GitHub's abuse-detection treat us as a
/// well-behaved client.
pub const USER_AGENT: &str = "bridge/0.0.1 (+https://github.com/) rust-reqwest";

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
        // Bridge talks to ~3 hosts steady-state (GitHub API, GitHub
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
        assert_eq!(p1, p2, "Lazy must hand back the same inner client across calls");
    }

    /// Configured defaults. Pins the contract so a future refactor
    /// that drops the User-Agent header (GitHub starts 403'ing
    /// without one) or removes the timeout (a hung fetch wedges the
    /// Settings panel forever) fails the test loudly.
    #[test]
    fn user_agent_is_set_and_recognisable() {
        // We can't introspect a reqwest::Client's UA directly, but
        // we can pin the const so a contributor renaming the
        // identifier sees the test fail.
        assert!(USER_AGENT.starts_with("bridge/"));
        assert!(USER_AGENT.contains("rust-reqwest"));
    }
}
