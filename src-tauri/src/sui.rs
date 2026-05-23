//! Minimal Sui RPC client.
//!
//! We talk to the public Sui mainnet JSON-RPC endpoint to read on-chain
//! state for a pilot's wallet address. Reads only — Bifrost never signs or
//! submits transactions. Strictly aligned with the "official APIs only"
//! posture: this endpoint is publicly documented and requires no auth.
//!
//! Reference: <https://docs.sui.io/sui-api-ref>

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{BridgeError, Result};
use crate::http;

/// Default public mainnet RPC. EVE Frontier launched on Sui mainnet; if
/// CCP ever switches networks we can make this configurable.
pub const DEFAULT_RPC: &str = "https://fullnode.mainnet.sui.io:443";

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
/// Doesn't own a `reqwest::Client` — it uses the process-wide
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
    /// 10 s per-request timeout — Sui mainnet RPC is usually
    /// sub-second; longer than that means the node is slow or the
    /// link is bad, and we'd rather surface that quickly than wedge
    /// the reconcile loop.
    ///
    /// Private — callers go through [`get_sui_balance`] /
    /// [`get_eve_balance`] so the coin-type IDs aren't repeated at
    /// each call site.
    async fn get_balance(&self, address: &str, coin_type: &str) -> Result<u128> {
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
            .map_err(|e| BridgeError::Other(format!("Sui RPC request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(BridgeError::Other(format!(
                "Sui RPC returned HTTP {status}"
            )));
        }

        let body: JsonRpcResponse<BalanceResult> = resp
            .json()
            .await
            .map_err(|e| BridgeError::Other(format!("Sui RPC response parse failed: {e}")))?;

        if let Some(err) = body.error {
            return Err(BridgeError::Other(format!(
                "Sui RPC error {}: {}",
                err.code, err.message
            )));
        }

        let result = body.result.ok_or_else(|| {
            BridgeError::Other("Sui RPC response had no result and no error".into())
        })?;

        result
            .total_balance
            .parse::<u128>()
            .map_err(|e| BridgeError::Other(format!("invalid totalBalance value: {e}")))
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

/// Render a coin amount (in smallest unit, 9 decimals — true for both
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
