//! Per-rider wallet address + on-chain balance fetches via the public
//! Sui mainnet RPC.

use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::error::{BifrostError, Result};
use crate::rider::Rider;
use crate::state::AppState;
use crate::sui::{format_coin, is_address_rate_limited, SuiClient};

/// Unix-millis "now" helper. Wraps the `SystemTime::now()` boilerplate
/// + the rare-but-possible negative-elapsed clock-skew case so callers
/// stay single-expression.
fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Set or clear a rider's wallet address, then immediately fetch its
/// current SUI + EVE balances from the public Sui mainnet RPC. Empty
/// / blank input clears both the address and the cached balances.
#[tauri::command]
pub async fn set_rider_wallet(
    state: State<'_, AppState>,
    id: String,
    address: String,
) -> Result<Rider> {
    let trimmed = address.trim().to_string();

    // Clearing case: empty input wipes both fields.
    if trimmed.is_empty() {
        let updated = {
            let mut riders = state.riders_lock();
            let p = riders
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
            p.wallet_address = None;
            p.wallet_balance = None;
            p.eve_balance = None;
            p.clone()
        };
        state.save_riders()?;
        return Ok(updated);
    }

    // Cheap sanity check — Sui addresses are 0x-prefixed, 64 hex chars.
    if !is_valid_sui_address(&trimmed) {
        return Err(BifrostError::Other(
            "Address must be 0x-prefixed and 64 hex chars long.".into(),
        ));
    }

    // Persist the address before the network call so it survives even
    // if the RPC is unreachable.
    {
        let mut riders = state.riders_lock();
        let p = riders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        p.wallet_address = Some(trimmed.clone());
    }
    state.save_riders()?;

    // Best-effort balance fetches for both coins. Fan out concurrently
    // via `tokio::join` so the two RPC calls overlap (was sequential —
    // ~2× latency for no good reason). `join` collects both results
    // even if one fails, so a single-coin failure leaves the other
    // populated; matches the previous sequential behaviour.
    let sui = SuiClient::new();
    let (sui_res, eve_res) = tokio::join!(
        sui.get_sui_balance(&trimmed),
        sui.get_eve_balance(&trimmed),
    );
    let sui_fmt = match sui_res {
        Ok(mist) => Some(format_coin(mist)),
        Err(e) => {
            tracing::warn!("SUI balance fetch for {trimmed} failed: {e}");
            None
        }
    };
    let eve_fmt = match eve_res {
        Ok(raw) => Some(format_coin(raw)),
        Err(e) => {
            tracing::warn!("EVE balance fetch for {trimmed} failed: {e}");
            None
        }
    };

    // Stamp the fetch time only when AT LEAST ONE coin came back
    // successfully. If both failed we keep the previous timestamp
    // (likely `None` since this is set_rider_wallet's first attempt)
    // so the UI staleness indicator stays accurate.
    let touched_at = if sui_fmt.is_some() || eve_fmt.is_some() {
        Some(now_unix_ms())
    } else {
        None
    };

    let updated = {
        let mut riders = state.riders_lock();
        let p = riders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BifrostError::RiderNotFound(id.clone()))?;
        p.wallet_balance = sui_fmt;
        p.eve_balance = eve_fmt;
        if touched_at.is_some() {
            p.wallet_balance_fetched_at = touched_at;
        }
        p.clone()
    };
    state.save_riders()?;
    Ok(updated)
}

/// One rider's just-fetched balances. `(rider_id, sui_result,
/// eve_result)` — kept as a `Result` per coin so the per-call error
/// is visible to the caller even when the other coin succeeded.
type BalanceResult = (
    String,
    std::result::Result<u128, BifrostError>,
    std::result::Result<u128, BifrostError>,
);

/// Re-fetch SUI + EVE balances for every rider that has a wallet
/// address. Concurrent across the two coins for each rider via
/// `tokio::join`; riders are still iterated sequentially because the
/// Sui RPC's per-IP throttling triggers on bursts, not on steady-
/// state load — back-to-back-with-a-tiny-gap is friendlier than
/// fanned-out-N-riders-at-once.
///
/// Two pre-flight checks per rider before any network call:
///
/// 1. `is_address_rate_limited(addr)` — if we recently got 429/503
///    for this address, skip both calls until the backoff window
///    expires. Without this the 30 s reconcile tick re-fires every
///    rider's pair of requests on every tick, extending the lockout.
/// 2. (implicit, via `get_balance`) — even if 1. somehow passes a
///    stale flag, the inner cache check inside `get_balance` is the
///    second line of defence.
///
/// Returns `true` if any rider's balance or fetch-timestamp changed,
/// so the caller can skip `save_riders()` on a no-op tick. This is
/// the second half of the "stop writing riders.json every 30 s" fix
/// — `reconcile_riders` does the first half by tracking status
/// changes; both signals together drive the save decision.
///
/// Called by [`super::lifecycle::reconcile_riders`].
pub async fn refresh_balances(state: &State<'_, AppState>) -> bool {
    let targets: Vec<(String, String)> = {
        let riders = state.riders_lock();
        riders
            .iter()
            .filter_map(|p| {
                p.wallet_address
                    .as_ref()
                    .map(|addr| (p.id.clone(), addr.clone()))
            })
            .collect()
    };
    if targets.is_empty() {
        return false;
    }

    let sui = SuiClient::new();
    let mut results: Vec<BalanceResult> = Vec::new();
    for (id, addr) in targets {
        // Pre-flight backoff check. Without this, a single
        // throttled rider would still consume one round-trip every
        // 30 s ("the inner cache will reject it"). The throughput
        // win is small but the symmetry with `release_cache` makes
        // the code easier to reason about.
        if is_address_rate_limited(&addr) {
            tracing::debug!(
                "wallet refresh: skipping rider {id} — Sui RPC backoff active"
            );
            // Surface the skip as a "no fresh data this tick" pair
            // of synthetic errors so the timestamp stays unchanged
            // and the UI staleness clock keeps ticking.
            let err = || {
                BifrostError::Other(
                    "Sui RPC rate-limit backoff active".into(),
                )
            };
            results.push((id, Err(err()), Err(err())));
            continue;
        }
        let (s, e) = tokio::join!(
            sui.get_sui_balance(&addr),
            sui.get_eve_balance(&addr),
        );
        results.push((id, s, e));
    }

    let now = now_unix_ms();
    let mut anything_changed = false;
    let mut riders = state.riders_lock();
    for (id, sui_res, eve_res) in results {
        if let Some(p) = riders.iter_mut().find(|p| p.id == id) {
            let mut any_success = false;
            match sui_res {
                Ok(mist) => {
                    let formatted = Some(format_coin(mist));
                    if p.wallet_balance != formatted {
                        p.wallet_balance = formatted;
                        anything_changed = true;
                    }
                    any_success = true;
                }
                Err(e) => tracing::debug!("SUI refresh for rider {id} failed: {e}"),
            }
            match eve_res {
                Ok(raw) => {
                    let formatted = Some(format_coin(raw));
                    if p.eve_balance != formatted {
                        p.eve_balance = formatted;
                        anything_changed = true;
                    }
                    any_success = true;
                }
                Err(e) => tracing::debug!("EVE refresh for rider {id} failed: {e}"),
            }
            // Stamp the refresh time only when SOMETHING came back —
            // a tick that failed both coins should leave the
            // staleness clock ticking from its previous value, not
            // reset it to "fresh". Bump `anything_changed` so the
            // serialised JSON reflects the new timestamp even when
            // the actual coin values were already current.
            if any_success {
                p.wallet_balance_fetched_at = Some(now);
                anything_changed = true;
            }
        }
    }
    anything_changed
}

/// Sui address shape check. Doesn't verify the address actually owns
/// anything — just that it's syntactically a Sui address (0x + 64 hex
/// chars) so we surface bad input before hitting the RPC.
fn is_valid_sui_address(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("0x") else {
        return false;
    };
    rest.len() == 64 && rest.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_sui_address_accepts_canonical() {
        let addr = format!("0x{}", "a".repeat(64));
        assert!(is_valid_sui_address(&addr));
    }

    #[test]
    fn valid_sui_address_rejects_missing_prefix() {
        let addr = "a".repeat(64);
        assert!(!is_valid_sui_address(&addr));
    }

    #[test]
    fn valid_sui_address_rejects_wrong_length() {
        // Far-from-canonical lengths.
        assert!(!is_valid_sui_address("0xdeadbeef"));
        assert!(!is_valid_sui_address(&format!("0x{}", "a".repeat(65))));
        // Empty hex body after the `0x` prefix.
        assert!(!is_valid_sui_address("0x"));
        // Single-char hex body — exercises the "is the loop guarded
        // by length, not just emptiness" path.
        assert!(!is_valid_sui_address("0xa"));
        // Off-by-one on both sides of the canonical 64 hex chars.
        assert!(!is_valid_sui_address(&format!("0x{}", "a".repeat(63))));
        assert!(!is_valid_sui_address(&format!("0x{}", "a".repeat(65))));
    }

    #[test]
    fn valid_sui_address_rejects_non_hex() {
        let addr = format!("0x{}", "z".repeat(64));
        assert!(!is_valid_sui_address(&addr));
    }

    /// Sui addresses are mixed-case hex by convention. The validator
    /// must accept uppercase A-F as well as lowercase a-f. If a
    /// future refactor narrows the check to lowercase-only, a real
    /// user who pasted a checksummed address would be rejected.
    #[test]
    fn valid_sui_address_accepts_mixed_case_hex() {
        let addr = format!("0x{}", "DeadBeefCafe1234".repeat(4)); // 64 hex chars, mixed case
        assert!(is_valid_sui_address(&addr));
    }
}
