//! On-chain account streaming data plane (Phase 2)
//!
//! Streams pool state over the Solana WebSocket `accountSubscribe` API and
//! computes prices locally, removing aggregator HTTP round-trips — and their
//! staleness — from the detection path. A price event arrives when the pool
//! actually changes, not when a stats API refreshes its cache.
//!
//! Two pool families are supported:
//! - **Constant-product AMMs** (e.g. Raydium AMM v4): subscribe to the two
//!   vault token accounts; spot price is the decimal-adjusted reserve ratio.
//! - **Concentrated-liquidity pools** (Orca Whirlpool, Raydium CLMM):
//!   subscribe to the pool account itself and decode its u128 sqrt price.
//!
//! Yellowstone gRPC (Geyser) is the lower-latency endgame for this data
//! plane; this module provides the same event-driven architecture using only
//! the standard RPC WebSocket endpoint, so it works with any RPC provider.

#[cfg(feature = "ws")]
use std::collections::HashMap;
#[cfg(feature = "ws")]
use std::time::Duration;

#[cfg(feature = "ws")]
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
#[cfg(feature = "ws")]
use base64::Engine;
use rust_decimal::Decimal;
#[cfg(feature = "ws")]
use tokio::sync::mpsc;
#[cfg(feature = "ws")]
use tracing::{debug, info};
use tracing::warn;

#[cfg(feature = "ws")]
use crate::PriceData;
use crate::{DexType, TokenPair};

/// SPL token account layout: mint (32) + owner (32) precede the u64 amount.
const SPL_AMOUNT_OFFSET: usize = 64;

/// Orca Whirlpool account layout: 8 (anchor discriminator) +
/// whirlpools_config (32) + whirlpool_bump (1) + tick_spacing (2) +
/// tick_spacing_seed (2) + fee_rate (2) + protocol_fee_rate (2) +
/// liquidity (16) — the u128 `sqrt_price` (Q64.64) follows at byte 65.
pub const WHIRLPOOL_SQRT_PRICE_OFFSET: usize = 65;

/// Raydium CLMM PoolState layout: 8 (anchor discriminator) + bump (1) +
/// amm_config (32) + owner (32) + token_mint_0/1 (64) + token_vault_0/1 (64) +
/// observation_key (32) + mint_decimals_0/1 (2) + tick_spacing (2) +
/// liquidity (16) — the u128 `sqrt_price_x64` follows at byte 253.
pub const RAYDIUM_CLMM_SQRT_PRICE_OFFSET: usize = 253;

/// How a pool's price is derived from streamed account data.
#[derive(Debug, Clone)]
pub enum PoolKind {
    /// Constant-product pool: price = decimal-adjusted vault reserve ratio.
    CpmmVaults {
        base_vault: String,
        quote_vault: String,
    },
    /// Concentrated-liquidity pool: price decoded from the Q64.64 sqrt price
    /// in the pool account. The pair's base token must be the pool's
    /// token0/tokenA and the quote its token1/tokenB (mint sort order).
    SqrtPricePool {
        pool: String,
        sqrt_price_offset: usize,
    },
}

/// A pool to stream.
#[derive(Debug, Clone)]
pub struct PoolSubscription {
    pub dex: DexType,
    pub pair: TokenPair,
    pub kind: PoolKind,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

impl PoolSubscription {
    /// Parse a `STREAMING_POOLS` env entry list (`;`-separated).
    ///
    /// Constant-product pools (6 fields):
    /// `Dex:BASE-QUOTE:base_vault:quote_vault:base_decimals:quote_decimals`
    ///
    /// Concentrated-liquidity pools (5 fields, pool account address):
    /// `OrcaWhirlpool:BASE-QUOTE:pool_address:base_decimals:quote_decimals`
    /// `RaydiumClmm:BASE-QUOTE:pool_address:base_decimals:quote_decimals`
    pub fn parse_list(raw: &str) -> Vec<Self> {
        raw.split(';')
            .filter(|entry| !entry.trim().is_empty())
            .filter_map(|entry| {
                let parts: Vec<&str> = entry.trim().split(':').collect();
                let parsed = match parts.len() {
                    6 => Self::parse_cpmm(&parts),
                    5 => Self::parse_sqrt_price(&parts),
                    _ => None,
                };
                if parsed.is_none() {
                    warn!("Ignoring malformed STREAMING_POOLS entry: {}", entry);
                }
                parsed
            })
            .collect()
    }

    fn parse_cpmm(parts: &[&str]) -> Option<Self> {
        let dex = parse_dex(parts[0])?;
        let (base, quote) = parts[1].split_once('-')?;
        Some(Self {
            dex,
            pair: TokenPair::new(base, quote),
            kind: PoolKind::CpmmVaults {
                base_vault: parts[2].to_string(),
                quote_vault: parts[3].to_string(),
            },
            base_decimals: parts[4].parse().ok()?,
            quote_decimals: parts[5].parse().ok()?,
        })
    }

    fn parse_sqrt_price(parts: &[&str]) -> Option<Self> {
        let (dex, sqrt_price_offset) = match parts[0].to_ascii_lowercase().as_str() {
            "orcawhirlpool" | "whirlpool" => (DexType::Orca, WHIRLPOOL_SQRT_PRICE_OFFSET),
            "raydiumclmm" => (DexType::Raydium, RAYDIUM_CLMM_SQRT_PRICE_OFFSET),
            other => {
                warn!("Unknown CLMM pool kind in STREAMING_POOLS: {}", other);
                return None;
            }
        };
        let (base, quote) = parts[1].split_once('-')?;
        Some(Self {
            dex,
            pair: TokenPair::new(base, quote),
            kind: PoolKind::SqrtPricePool {
                pool: parts[2].to_string(),
                sqrt_price_offset,
            },
            base_decimals: parts[3].parse().ok()?,
            quote_decimals: parts[4].parse().ok()?,
        })
    }

    /// Account pubkeys this pool needs subscriptions for.
    pub fn accounts(&self) -> Vec<&str> {
        match &self.kind {
            PoolKind::CpmmVaults {
                base_vault,
                quote_vault,
            } => vec![base_vault, quote_vault],
            PoolKind::SqrtPricePool { pool, .. } => vec![pool],
        }
    }
}

fn parse_dex(s: &str) -> Option<DexType> {
    match s.to_ascii_lowercase().as_str() {
        "raydium" => Some(DexType::Raydium),
        "orca" => Some(DexType::Orca),
        "jupiter" => Some(DexType::Jupiter),
        "lifinity" => Some(DexType::Lifinity),
        "meteora" => Some(DexType::Meteora),
        "phoenix" => Some(DexType::Phoenix),
        other => {
            warn!("Unknown DEX in STREAMING_POOLS: {}", other);
            None
        }
    }
}

/// Decode the `amount` field from raw SPL token account data.
pub fn decode_spl_token_amount(data: &[u8]) -> Option<u64> {
    let bytes = data.get(SPL_AMOUNT_OFFSET..SPL_AMOUNT_OFFSET + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

/// Decode a little-endian u128 at `offset`.
pub fn decode_u128_le(data: &[u8], offset: usize) -> Option<u128> {
    let bytes = data.get(offset..offset + 16)?;
    Some(u128::from_le_bytes(bytes.try_into().ok()?))
}

/// Spot price of the base token in quote terms from CPMM vault reserves.
pub fn spot_price(
    base_amount: u64,
    quote_amount: u64,
    base_decimals: u8,
    quote_decimals: u8,
) -> Option<Decimal> {
    if base_amount == 0 || quote_amount == 0 {
        return None;
    }
    let base = Decimal::from(base_amount) / Decimal::from(10u64.checked_pow(base_decimals as u32)?);
    let quote =
        Decimal::from(quote_amount) / Decimal::from(10u64.checked_pow(quote_decimals as u32)?);
    if base.is_zero() {
        None
    } else {
        Some(quote / base)
    }
}

/// Convert a Q64.64 sqrt price (token1 per token0, raw units) into a
/// decimal-adjusted price of base (token0) in quote (token1) terms.
///
/// f64 carries ~15 significant digits — ample for detection; execution is
/// re-priced through swap quotes anyway.
pub fn sqrt_price_x64_to_price(
    sqrt_price_x64: u128,
    base_decimals: u8,
    quote_decimals: u8,
) -> Option<Decimal> {
    if sqrt_price_x64 == 0 {
        return None;
    }
    let sqrt = sqrt_price_x64 as f64 / 2f64.powi(64);
    let price = sqrt * sqrt * 10f64.powi(base_decimals as i32 - quote_decimals as i32);
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    Decimal::from_f64_retain(price)
}

/// Derive a WebSocket endpoint from an HTTP RPC URL.
pub fn derive_ws_url(rpc_url: &str) -> String {
    if let Some(rest) = rpc_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = rpc_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        rpc_url.to_string()
    }
}

/// Streams pool account updates and emits locally computed pool prices.
pub struct AccountStreamer {
    ws_url: String,
    pools: Vec<PoolSubscription>,
}

#[cfg(feature = "ws")]
impl AccountStreamer {
    /// `rpc_url` may be an HTTP RPC URL (converted to wss) or an explicit
    /// ws/wss URL. SOLANA_WS_URL should be passed here when the provider uses
    /// a dedicated WebSocket host.
    pub fn new(rpc_url: &str, pools: Vec<PoolSubscription>) -> Self {
        Self {
            ws_url: derive_ws_url(rpc_url),
            pools,
        }
    }

    /// Run forever, reconnecting with backoff. Emits a `PriceData` whenever a
    /// subscribed pool changes. Exits when the receiver is dropped.
    pub async fn run(self, tx: mpsc::Sender<PriceData>) {
        let mut backoff_secs = 1u64;
        loop {
            info!(
                "📡 Connecting account stream to {} ({} pools)",
                self.ws_url,
                self.pools.len()
            );
            match self.connect_and_stream(&tx).await {
                Ok(()) => {
                    // Receiver dropped — orderly shutdown.
                    return;
                }
                Err(e) => {
                    warn!(
                        "Account stream disconnected: {}. Reconnecting in {}s",
                        e, backoff_secs
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(30);
                }
            }
        }
    }

    async fn connect_and_stream(&self, tx: &mpsc::Sender<PriceData>) -> anyhow::Result<()> {
        use anyhow::anyhow;
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let (ws, _) = tokio_tungstenite::connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws.split();

        // account pubkey -> indices of pools that read it
        let mut account_pools: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, pool) in self.pools.iter().enumerate() {
            for account in pool.accounts() {
                account_pools.entry(account.to_string()).or_default().push(idx);
            }
        }

        // Subscribe to every account; request id maps back to the pubkey.
        let mut request_accounts: HashMap<u64, String> = HashMap::new();
        for (i, account) in account_pools.keys().enumerate() {
            let id = (i + 1) as u64;
            let req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "accountSubscribe",
                "params": [
                    account,
                    { "encoding": "base64", "commitment": "processed" }
                ]
            });
            write.send(Message::Text(req.to_string())).await?;
            request_accounts.insert(id, account.clone());
        }

        // subscription id -> account pubkey
        let mut subscriptions: HashMap<u64, String> = HashMap::new();
        // vault pubkey -> last seen token amount (CPMM pools only)
        let mut amounts: HashMap<String, u64> = HashMap::new();

        while let Some(msg) = read.next().await {
            let text = match msg? {
                Message::Text(t) => t,
                Message::Ping(payload) => {
                    write.send(Message::Pong(payload)).await?;
                    continue;
                }
                Message::Close(frame) => {
                    return Err(anyhow!("server closed connection: {:?}", frame));
                }
                _ => continue,
            };

            let value: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Subscription confirmation: {"result": <sub_id>, "id": <req_id>}
            if let (Some(req_id), Some(sub_id)) = (
                value.get("id").and_then(|v| v.as_u64()),
                value.get("result").and_then(|v| v.as_u64()),
            ) {
                if let Some(account) = request_accounts.get(&req_id) {
                    debug!("Subscribed to {} (subscription {})", account, sub_id);
                    subscriptions.insert(sub_id, account.clone());
                }
                continue;
            }

            // Account update notification
            if value.get("method").and_then(|m| m.as_str()) != Some("accountNotification") {
                continue;
            }
            let params = &value["params"];
            let Some(sub_id) = params.get("subscription").and_then(|v| v.as_u64()) else {
                continue;
            };
            let Some(account) = subscriptions.get(&sub_id).cloned() else {
                continue;
            };
            let Some(data_b64) = params["result"]["value"]["data"]
                .get(0)
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            let Ok(data) = BASE64_ENGINE.decode(data_b64) else {
                continue;
            };

            let Some(pool_indices) = account_pools.get(&account) else {
                continue;
            };

            for &pool_idx in pool_indices {
                let pool = &self.pools[pool_idx];
                let price = match &pool.kind {
                    PoolKind::CpmmVaults {
                        base_vault,
                        quote_vault,
                    } => {
                        let Some(amount) = decode_spl_token_amount(&data) else {
                            continue;
                        };
                        amounts.insert(account.clone(), amount);
                        let (Some(&base_amt), Some(&quote_amt)) =
                            (amounts.get(base_vault), amounts.get(quote_vault))
                        else {
                            // Wait until both vault balances have been observed.
                            continue;
                        };
                        spot_price(base_amt, quote_amt, pool.base_decimals, pool.quote_decimals)
                    }
                    PoolKind::SqrtPricePool {
                        sqrt_price_offset, ..
                    } => decode_u128_le(&data, *sqrt_price_offset).and_then(|sqrt| {
                        sqrt_price_x64_to_price(sqrt, pool.base_decimals, pool.quote_decimals)
                    }),
                };

                let Some(price) = price else {
                    continue;
                };

                // The pool's swap fee is the effective spread around spot.
                let fee = pool.dex.fee_percentage();
                let bid = price * (Decimal::ONE - fee);
                let ask = price * (Decimal::ONE + fee);
                let price_data = PriceData::new(pool.dex, pool.pair.clone(), bid, ask);

                if tx.send(price_data).await.is_err() {
                    return Ok(()); // Receiver dropped — shut down.
                }
            }
        }

        Err(anyhow!("websocket stream ended"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_cpmm_entry() {
        let pools = PoolSubscription::parse_list(
            "Raydium:SOL-USDC:VaultBase111:VaultQuote111:9:6;orca:RAY-USDC:A:B:6:6",
        );
        assert_eq!(pools.len(), 2);
        assert_eq!(pools[0].dex, DexType::Raydium);
        assert_eq!(pools[0].pair.symbol(), "SOL/USDC");
        assert!(matches!(
            &pools[0].kind,
            PoolKind::CpmmVaults { base_vault, .. } if base_vault == "VaultBase111"
        ));
        assert_eq!(pools[0].quote_decimals, 6);
        assert_eq!(pools[1].dex, DexType::Orca);
    }

    #[test]
    fn test_parse_list_clmm_entries() {
        let pools = PoolSubscription::parse_list(
            "OrcaWhirlpool:SOL-USDC:Pool111:9:6;RaydiumClmm:RAY-USDC:Pool222:6:6",
        );
        assert_eq!(pools.len(), 2);
        assert_eq!(pools[0].dex, DexType::Orca);
        assert!(matches!(
            &pools[0].kind,
            PoolKind::SqrtPricePool { pool, sqrt_price_offset }
                if pool == "Pool111" && *sqrt_price_offset == WHIRLPOOL_SQRT_PRICE_OFFSET
        ));
        assert_eq!(pools[1].dex, DexType::Raydium);
        assert!(matches!(
            &pools[1].kind,
            PoolKind::SqrtPricePool { sqrt_price_offset, .. }
                if *sqrt_price_offset == RAYDIUM_CLMM_SQRT_PRICE_OFFSET
        ));
    }

    #[test]
    fn test_parse_list_skips_malformed() {
        let pools = PoolSubscription::parse_list("garbage;Raydium:SOL-USDC:A:B:9:6;also:bad");
        assert_eq!(pools.len(), 1);
    }

    #[test]
    fn test_parse_list_empty() {
        assert!(PoolSubscription::parse_list("").is_empty());
        assert!(PoolSubscription::parse_list("  ;  ").is_empty());
    }

    #[test]
    fn test_decode_spl_token_amount() {
        // mint (32) + owner (32) + amount (8 LE)
        let mut data = vec![0u8; 165];
        data[64..72].copy_from_slice(&123_456_789u64.to_le_bytes());
        assert_eq!(decode_spl_token_amount(&data), Some(123_456_789));
    }

    #[test]
    fn test_decode_spl_token_amount_short_data() {
        assert_eq!(decode_spl_token_amount(&[0u8; 60]), None);
    }

    #[test]
    fn test_decode_u128_le() {
        let mut data = vec![0u8; 100];
        data[65..81].copy_from_slice(&42u128.to_le_bytes());
        assert_eq!(decode_u128_le(&data, 65), Some(42));
        assert_eq!(decode_u128_le(&data, 90), None); // out of bounds
    }

    #[test]
    fn test_spot_price_sol_usdc() {
        // 1,000 SOL (9 decimals) against 64,000 USDC (6 decimals) => 64 USDC/SOL
        let price = spot_price(1_000_000_000_000, 64_000_000_000, 9, 6).unwrap();
        assert_eq!(price, Decimal::from(64));
    }

    #[test]
    fn test_spot_price_zero_reserves() {
        assert!(spot_price(0, 1_000, 9, 6).is_none());
        assert!(spot_price(1_000, 0, 9, 6).is_none());
    }

    #[test]
    fn test_sqrt_price_equal_decimals() {
        // sqrt = 2.0 in Q64.64 => price 4.0 with equal decimals
        let sqrt_x64 = 2u128 << 64;
        let price = sqrt_price_x64_to_price(sqrt_x64, 6, 6).unwrap();
        assert_eq!(price, Decimal::from(4));
    }

    #[test]
    fn test_sqrt_price_decimal_adjustment() {
        // sqrt = 1.0 => raw price 1.0; base 9 decimals vs quote 6 decimals
        // adjusts by 10^(9-6) = 1000.
        let sqrt_x64 = 1u128 << 64;
        let price = sqrt_price_x64_to_price(sqrt_x64, 9, 6).unwrap();
        assert_eq!(price, Decimal::from(1000));
    }

    #[test]
    fn test_sqrt_price_zero() {
        assert!(sqrt_price_x64_to_price(0, 9, 6).is_none());
    }

    #[test]
    fn test_derive_ws_url() {
        assert_eq!(
            derive_ws_url("https://mainnet.helius-rpc.com/?api-key=x"),
            "wss://mainnet.helius-rpc.com/?api-key=x"
        );
        assert_eq!(derive_ws_url("http://localhost:8899"), "ws://localhost:8899");
        assert_eq!(derive_ws_url("wss://already.ws"), "wss://already.ws");
    }
}
