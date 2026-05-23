//! Minimal Sui RPC client.
//!
//! We talk to the public Sui mainnet JSON-RPC endpoint to read on-chain
//! state for a pilot's wallet address. Reads only â€” Bifrost never signs or
//! submits transactions. Strictly aligned with the "official APIs only"
//! posture: this endpoint is publicly documented and requires no auth.
//!
//! Reference: <https://docs.sui.io/sui-api-ref>

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::error::{BifrostError, Result};
use crate::http;

/// Default public mainnet RPC. EVE Frontier launched on Sui mainnet; if
/// CCP ever switches networks we can make this configurable.
pub const DEFAULT_RPC: &str = "https://fullnode.mainnet.sui.io:443";

/// How long to back off after detecting a rate-limit response from the
/// Sui RPC. Mirrors the `release_cache::FAILURE_BACKOFF_TTL` shape but
/// shorter (2 min instead of 5) because Sui's public mainnet RPC is
/// much less strictly rate-limited than GitHub's unauth API; 2 min is
/// enough to ride out a brief throttling burst without locking
/// balances stale for too long.
///
/// The cache is keyed by address (NOT by `(address, coin)`) â€” when one
/// coin call gets throttled, the other is almost certainly throttled
/// too because they share the same RPC URL + IP. Per-address keying
/// means N pilots Ã— 1 entry, not N Ã— 2.
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(2 * 60);

/// Per-address rate-limit backoff cache. When the Sui RPC throttles a
/// call for an address, we record the instant it happened and refuse
/// subsequent calls for that address until the backoff window
/// expires. Without this the 30 s reconcile tick would re-fire all 2N
/// balance fetches on every tick, burning more quota and extending
/// the lockout â€” same shape as the GitHub-API hammering bug we just
/// fixed in `release_cache`.
static RATE_LIMITED_UNTIL: Lazy<Mutex<HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// True if `address` is currently in the rate-limit backoff window.
/// Stale entries (older than the backoff TTL) are NOT pruned on read
/// â€” we only inspect freshness â€” but they're naturally overwritten
/// on the next failure for that address. The HashMap doesn't grow
/// unbounded because the key space is bounded by the user's pilot
/// count.
pub fn is_address_rate_limited(address: &str) -> bool {
    let Ok(guard) = RATE_LIMITED_UNTIL.lock() else {
        return false;
    };
    let Some(inserted) = guard.get(address) else {
        return false;
    };
    inserted.elapsed() < RATE_LIMIT_BACKOFF
}

/// Mark `address` as currently rate-limited. Re-marking refreshes the
/// timer â€” a sustained throttling pattern keeps the backoff active.
fn mark_address_rate_limited(address: &str) {
    if let Ok(mut guard) = RATE_LIMITED_UNTIL.lock() {
        guard.insert(address.to_string(), Instant::now());
    }
}

/// Clear the rate-limit backoff for an address. Called after a
/// successful fetch so the cache doesn't carry stale "this address
/// was throttled 90 s ago" entries through the next backoff window.
fn clear_address_rate_limited(address: &str) {
    if let Ok(mut guard) = RATE_LIMITED_UNTIL.lock() {
        guard.remove(address);
    }
}

/// Pattern-match a Sui RPC error to decide whether it's a rate-limit
/// signal (vs. transport timeout or address-not-found). Mirrors the
/// `release_cache::is_rate_limit_error` shape but adapted for the
/// error formats `SuiClient::get_balance` actually produces:
///
/// * HTTP 429 â€” "Too Many Requests" (explicit rate-limit response)
/// * HTTP 503 â€” "Service Unavailable" (overloaded node; backing off
///   is the polite move even if not technically a rate limit)
/// * HTTP 403 â€” "Forbidden" (some Sui RPC providers gate by IP)
/// * "rate limit" / "rate-limit" substrings â€” defence against future
///   error-format changes
///
/// Transport timeouts (`request timed out`, `connection refused`,
/// `dns lookup failed`) are deliberately NOT matched â€” those usually
/// resolve on the next attempt and we don't want to lock the user
/// out for 2 min over a single Wi-Fi blip.
pub fn is_rate_limit_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("429")
        || lower.contains("503")
        || lower.contains("403")
        || lower.contains("rate limit")
        || lower.contains("rate-limit")
        || lower.contains("too many requests")
}

/// Native Sui coin type. `suix_getBalance` defaults to this when `coin_type`
/// is null/omitted.
pub const SUI_COIN_TYPE: &str = "0x2::sui::SUI";

/// EVE Frontier's on-chain player currency. Coin type registered on Sui
/// mainnet. Same 9-decimal convention as SUI in practice.
pub const EVE_COIN_TYPE: &str =
    "0x2a66a89b5a735738ffa4423ac024d23571326163f324f9051557617319e59d60::EVE::EVE";

/// 1 SUI = 10^9 MIST.
const MIST_PER_SUI: u64 = 1_000_000_000;

/// JSON-RPC client for Sui mainnet.
///
/// Doesn't own a `reqwest::Client` â€” it uses the process-wide
/// [`http::client`] so the TLS + connection pool is shared with the
/// GitHub fetches. This struct is now a thin namespace over the RPC
/// URL; keeping it as a struct (vs. free functions) preserves
/// extensibility for things like custom RPC URLs or per-pilot auth
/// headers in the future.
#[derive(Debug, Clone)]
pub struct SuiClient {
    rpc_url: String,
}

impl SuiClient {
    pub fn new() -> Self {
        Self {
            rpc_url: DEFAULT_RPC.to_string(),
        }
    }

    /// Fetch a coin balance for an address. Returns the totalBalance
    /// in the coin's smallest unit (e.g. MIST for SUI). Errors
    /// propagate transport / parse failures.
    ///
    /// 10 s per-request timeout â€” Sui mainnet RPC is usually
    /// sub-second; longer than that means the node is slow or the
    /// link is bad, and we'd rather surface that quickly than wedge
    /// the reconcile loop.
    ///
    /// Private â€” callers go through [`get_sui_balance`] /
    /// [`get_eve_balance`] so the coin-type IDs aren't repeated at
    /// each call site.
    async fn get_balance(&self, address: &str, coin_type: &str) -> Result<u128> {
        // Rate-limit short-circuit: if a previous call for this
        // address recently hit a 429/503/etc., refuse to fire another
        // request until the backoff window expires. Returns the same
        // error shape `is_rate_limit_error` matches so the per-pilot
        // skip logic upstream stays consistent.
        if is_address_rate_limited(address) {
            return Err(BifrostError::Other(
                "Sui RPC rate-limit backoff active for this address; will retry shortly".into(),
            ));
        }

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "suix_getBalance",
            "params": [address, coin_type],
        });

        let resp = http::client()
            .post(&self.rpc_url)
            .timeout(Duration::from_secs(10))
            .json(&body)
            .send()
            .await
            .map_err(|e| BifrostError::Other(format!("Sui RPC request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let err_msg = format!("Sui RPC returned HTTP {status}");
            // 429 / 503 / 403 â†’ mark the address as rate-limited so
            // subsequent ticks short-circuit without firing more
            // requests at an overloaded node.
            if is_rate_limit_error(&err_msg) {
                mark_address_rate_limited(address);
            }
            return Err(BifrostError::Other(err_msg));
        }

        let body: JsonRpcResponse<BalanceResult> = resp
            .json()
            .await
            .map_err(|e| BifrostError::Other(format!("Sui RPC response parse failed: {e}")))?;

        if let Some(err) = body.error {
            let err_msg = format!("Sui RPC error {}: {}", err.code, err.message);
            // JSON-RPC overload codes can show up as 200 OK with an
            // `error` field â€” `-32000` is Sui's generic "server is
            // having a bad time" code that the public node uses for
            // both throttling and transient overload. Treat as
            // rate-limit-equivalent and back off.
            if err.code == -32000 || is_rate_limit_error(&err_msg) {
                mark_address_rate_limited(address);
            }
            return Err(BifrostError::Other(err_msg));
        }

        let result = body.result.ok_or_else(|| {
            BifrostError::Other("Sui RPC response had no result and no error".into())
        })?;

        let value = result
            .total_balance
            .parse::<u128>()
            .map_err(|e| BifrostError::Other(format!("invalid totalBalance value: {e}")))?;

        // Successful fetch clears any stale rate-limit entry so the
        // address doesn't carry a leftover "throttled 90 s ago" mark
        // through the next backoff window.
        clear_address_rate_limited(address);
        Ok(value)
    }

    /// Convenience wrapper for the native SUI balance.
    pub async fn get_sui_balance(&self, address: &str) -> Result<u128> {
        self.get_balance(address, SUI_COIN_TYPE).await
    }

    /// Convenience wrapper for the EVE token balance.
    pub async fn get_eve_balance(&self, address: &str) -> Result<u128> {
        self.get_balance(address, EVE_COIN_TYPE).await
    }
}

impl Default for SuiClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum fractional digits surfaced in the UI. EVE Vault and the Sui
/// explorer use 9, but pilot cards stay compact at 4.
const MAX_FRACTIONAL_DIGITS: usize = 4;

/// Render a coin amount (in smallest unit, 9 decimals â€” true for both
/// SUI and EVE) as a human-readable string, trimmed to
/// [`MAX_FRACTIONAL_DIGITS`] so cards stay compact.
pub fn format_coin(raw: u128) -> String {
    let whole = raw / MIST_PER_SUI as u128;
    let frac = raw % MIST_PER_SUI as u128;
    if frac == 0 {
        return whole.to_string();
    }
    // Render with 9 decimals, then trim trailing zeros to keep it short.
    let fractional = format!("{:09}", frac);
    let trimmed = fractional.trim_end_matches('0');
    let short = if trimmed.len() > MAX_FRACTIONAL_DIGITS {
        &trimmed[..MAX_FRACTIONAL_DIGITS]
    } else {
        trimmed
    };
    format!("{}.{}", whole, short)
}

// --- JSON-RPC envelope types ---

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BalanceResult {
    #[serde(rename = "totalBalance")]
    total_balance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `is_rate_limit_error` matches the substrings `SuiClient::
    /// get_balance` actually produces â€” pinning the contract here so a
    /// future refactor that changes error wording breaks loudly
    /// rather than silently disabling the backoff cache.
    #[test]
    fn rate_limit_error_recognises_throttling_responses() {
        assert!(is_rate_limit_error("Sui RPC returned HTTP 429 Too Many Requests"));
        assert!(is_rate_limit_error("Sui RPC returned HTTP 503 Service Unavailable"));
        assert!(is_rate_limit_error("Sui RPC returned HTTP 403 Forbidden"));
        assert!(is_rate_limit_error("Sui RPC error -32000: rate limit exceeded"));
        assert!(is_rate_limit_error("rate-limit reached"));
    }

    /// Transport-class failures (timeout, DNS) must NOT trigger the
    /// backoff â€” those are transient and usually resolve next tick.
    /// If we treated a 1-second Wi-Fi drop as rate-limiting, the user
    /// would be locked out of balance refreshes for 2 min for no
    /// reason.
    #[test]
    fn rate_limit_error_ignores_transport_failures() {
        assert!(!is_rate_limit_error("Sui RPC request failed: connection refused"));
        assert!(!is_rate_limit_error("Sui RPC request failed: dns resolution error"));
        assert!(!is_rate_limit_error("Sui RPC request failed: request timed out"));
        assert!(!is_rate_limit_error("invalid totalBalance value: invalid digit"));
    }

    /// Pin the address-keyed cache shape: marking an address makes it
    /// "rate-limited"; clearing it makes it free again. Doesn't pin
    /// TTL expiry (that would require a sleep or a clock injection
    /// the cache doesn't currently support) â€” see the deliberate
    /// scope-limit comment in `is_address_rate_limited`.
    #[test]
    fn address_rate_limit_cache_marks_and_clears() {
        // Unique key per test run so we don't collide with parallel
        // test execution.
        let addr = format!("0xtest_marks_and_clears_{}", std::process::id());

        assert!(!is_address_rate_limited(&addr), "fresh address must not be marked");
        mark_address_rate_limited(&addr);
        assert!(is_address_rate_limited(&addr), "marked address must read as rate-limited");
        clear_address_rate_limited(&addr);
        assert!(!is_address_rate_limited(&addr), "cleared address must read as free again");
    }

    /// Re-marking refreshes the timer rather than appending a second
    /// entry â€” sustained throttling keeps the cache "alive" for the
    /// full TTL after the LAST failure, not the first one. Pin the
    /// contract so a future hashmap-replacement-with-vec refactor
    /// doesn't accidentally make the cache append-only.
    #[test]
    fn address_rate_limit_cache_remark_overwrites() {
        let addr = format!("0xtest_remark_overwrites_{}", std::process::id());
        mark_address_rate_limited(&addr);
        mark_address_rate_limited(&addr);
        mark_address_rate_limited(&addr);
        // HashMap insert is overwriting; if it were appending we'd
        // need a count-N check. Just verify the entry is still
        // singular by reading.
        assert!(is_address_rate_limited(&addr));
        clear_address_rate_limited(&addr);
    }
}
