//! Per-pilot wallet address + on-chain balance fetches via the public
//! Sui mainnet RPC.

use tauri::State;

use crate::error::{BridgeError, Result};
use crate::pilot::Pilot;
use crate::state::AppState;
use crate::sui::{format_coin, SuiClient};

/// Set or clear a pilot's wallet address, then immediately fetch its
/// current SUI + EVE balances from the public Sui mainnet RPC. Empty
/// / blank input clears both the address and the cached balances.
#[tauri::command]
pub async fn set_pilot_wallet(
    state: State<'_, AppState>,
    id: String,
    address: String,
) -> Result<Pilot> {
    let trimmed = address.trim().to_string();

    // Clearing case: empty input wipes both fields.
    if trimmed.is_empty() {
        let updated = {
            let mut pilots = state.pilots.lock().unwrap();
            let p = pilots
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| BridgeError::PilotNotFound(id.clone()))?;
            p.wallet_address = None;
            p.wallet_balance = None;
            p.eve_balance = None;
            p.clone()
        };
        state.save_pilots()?;
        return Ok(updated);
    }

    // Cheap sanity check — Sui addresses are 0x-prefixed, 64 hex chars.
    if !is_valid_sui_address(&trimmed) {
        return Err(BridgeError::Other(
            "Address must be 0x-prefixed and 64 hex chars long.".into(),
        ));
    }

    // Persist the address before the network call so it survives even
    // if the RPC is unreachable.
    {
        let mut pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BridgeError::PilotNotFound(id.clone()))?;
        p.wallet_address = Some(trimmed.clone());
    }
    state.save_pilots()?;

    // Best-effort balance fetches for both coins. Failure on either
    // leaves that balance unset; the address persists either way.
    let sui = SuiClient::new();
    let sui_fmt = match sui.get_sui_balance(&trimmed).await {
        Ok(mist) => Some(format_coin(mist)),
        Err(e) => {
            tracing::warn!("SUI balance fetch for {trimmed} failed: {e}");
            None
        }
    };
    let eve_fmt = match sui.get_eve_balance(&trimmed).await {
        Ok(raw) => Some(format_coin(raw)),
        Err(e) => {
            tracing::warn!("EVE balance fetch for {trimmed} failed: {e}");
            None
        }
    };

    let updated = {
        let mut pilots = state.pilots.lock().unwrap();
        let p = pilots
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| BridgeError::PilotNotFound(id.clone()))?;
        p.wallet_balance = sui_fmt;
        p.eve_balance = eve_fmt;
        p.clone()
    };
    state.save_pilots()?;
    Ok(updated)
}

/// One pilot's just-fetched balances. `(pilot_id, sui_result,
/// eve_result)` — kept as a `Result` per coin so the per-call error
/// is visible to the caller even when the other coin succeeded.
type BalanceResult = (
    String,
    std::result::Result<u128, BridgeError>,
    std::result::Result<u128, BridgeError>,
);

/// Re-fetch SUI + EVE balances for every pilot that has a wallet
/// address. Issues RPC calls sequentially — typical N is small (1–10)
/// so the latency win from concurrency isn't worth the lock complexity.
/// Called by [`super::lifecycle::reconcile_pilots`].
pub async fn refresh_balances(state: &State<'_, AppState>) {
    let targets: Vec<(String, String)> = {
        let pilots = state.pilots.lock().unwrap();
        pilots
            .iter()
            .filter_map(|p| {
                p.wallet_address
                    .as_ref()
                    .map(|addr| (p.id.clone(), addr.clone()))
            })
            .collect()
    };
    if targets.is_empty() {
        return;
    }

    let sui = SuiClient::new();
    let mut results: Vec<BalanceResult> = Vec::new();
    for (id, addr) in targets {
        let s = sui.get_sui_balance(&addr).await;
        let e = sui.get_eve_balance(&addr).await;
        results.push((id, s, e));
    }

    let mut pilots = state.pilots.lock().unwrap();
    for (id, sui_res, eve_res) in results {
        if let Some(p) = pilots.iter_mut().find(|p| p.id == id) {
            match sui_res {
                Ok(mist) => p.wallet_balance = Some(format_coin(mist)),
                Err(e) => tracing::warn!("SUI refresh for pilot {id} failed: {e}"),
            }
            match eve_res {
                Ok(raw) => p.eve_balance = Some(format_coin(raw)),
                Err(e) => tracing::warn!("EVE refresh for pilot {id} failed: {e}"),
            }
        }
    }
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
        assert!(!is_valid_sui_address("0xdeadbeef"));
        assert!(!is_valid_sui_address(&format!("0x{}", "a".repeat(65))));
    }

    #[test]
    fn valid_sui_address_rejects_non_hex() {
        let addr = format!("0x{}", "z".repeat(64));
        assert!(!is_valid_sui_address(&addr));
    }
}
