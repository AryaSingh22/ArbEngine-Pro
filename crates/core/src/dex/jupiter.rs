//! Jupiter DEX Provider
//!
//! Jupiter is a DEX aggregator that routes trades through multiple DEXs
//! to find the best prices. We use their Price API V3 for price data.
//! The legacy price.jup.ag/v6 host was sunset by Jupiter; V3 returns USD
//! prices per mint, so pair prices are derived as a ratio of USD prices.

use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::{DexProvider, PriceStream};
use crate::{ArbitrageError, ArbitrageResult, DexType, PriceData, TokenPair};

/// Free-tier Jupiter Price API V3 host
const JUPITER_PRICE_API: &str = "https://lite-api.jup.ag/price/v3";
/// Keyed Jupiter Price API V3 host (requires JUPITER_API_KEY via x-api-key)
const JUPITER_PRICE_API_PRO: &str = "https://api.jup.ag/price/v3";

/// Jupiter DEX provider implementation
pub struct JupiterProvider {
    client: reqwest::Client,
    /// Token symbol to mint address mapping
    token_mints: HashMap<String, String>,
    /// Price API V3 base URL
    api_url: String,
    /// Optional API key for the api.jup.ag tier
    api_key: Option<String>,
}

/// Price API V3 returns a flat map of mint address -> price entry.
type JupiterPriceResponse = HashMap<String, JupiterTokenPrice>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JupiterTokenPrice {
    usd_price: f64,
}

impl JupiterProvider {
    pub fn new() -> Self {
        let mut token_mints = HashMap::new();
        // Common Solana tokens
        token_mints.insert(
            "SOL".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
        );
        token_mints.insert(
            "USDC".to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        );
        token_mints.insert(
            "USDT".to_string(),
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
        );
        token_mints.insert(
            "RAY".to_string(),
            "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R".to_string(),
        );
        token_mints.insert(
            "SRM".to_string(),
            "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt".to_string(),
        );
        token_mints.insert(
            "BONK".to_string(),
            "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263".to_string(),
        );
        token_mints.insert(
            "JUP".to_string(),
            "JUPyiwrYJFskUPiHa7hkeR8VUtAe6poCFFRLnWo6h7rL".to_string(),
        );
        token_mints.insert(
            "ORCA".to_string(),
            "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE".to_string(),
        );

        let api_key = std::env::var("JUPITER_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());
        let api_url = std::env::var("JUPITER_PRICE_API_URL").unwrap_or_else(|_| {
            if api_key.is_some() {
                JUPITER_PRICE_API_PRO.to_string()
            } else {
                JUPITER_PRICE_API.to_string()
            }
        });

        Self {
            client: crate::http::pool::create_optimized_client(),
            token_mints,
            api_url,
            api_key,
        }
    }

    /// Get the mint address for a token symbol
    fn get_mint(&self, symbol: &str) -> Option<&String> {
        self.token_mints.get(symbol)
    }

    /// Add a custom token mapping
    pub fn add_token(&mut self, symbol: String, mint: String) {
        self.token_mints.insert(symbol, mint);
    }

    /// Fetch USD prices for a comma-separated list of mint addresses
    async fn fetch_prices(
        client: &reqwest::Client,
        api_url: &str,
        api_key: &Option<String>,
        ids: &str,
    ) -> ArbitrageResult<JupiterPriceResponse> {
        let url = format!("{}?ids={}", api_url, ids);
        let req = match api_key {
            Some(key) => client.get(&url).header("x-api-key", key),
            None => client.get(&url),
        };
        Ok(req.send().await?.json().await?)
    }

    /// Derive a pair price from per-mint USD prices: base_usd / quote_usd
    fn pair_price(
        prices: &JupiterPriceResponse,
        base_mint: &str,
        quote_mint: &str,
    ) -> ArbitrageResult<Decimal> {
        let base_usd = prices
            .get(base_mint)
            .ok_or_else(|| ArbitrageError::PriceFetch("No price data returned".to_string()))?
            .usd_price;
        let quote_usd = prices
            .get(quote_mint)
            .ok_or_else(|| ArbitrageError::PriceFetch("No price data returned".to_string()))?
            .usd_price;

        if quote_usd <= 0.0 {
            return Err(ArbitrageError::PriceFetch(
                "Quote token USD price is zero".to_string(),
            ));
        }

        Decimal::try_from(base_usd / quote_usd)
            .map_err(|e| ArbitrageError::PriceFetch(format!("Invalid price: {}", e)))
    }
}

impl Default for JupiterProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DexProvider for JupiterProvider {
    fn dex_type(&self) -> DexType {
        DexType::Jupiter
    }

    fn get_price<'a>(&'a self, pair: &'a TokenPair) -> crate::dex::BoxFuture<'a, ArbitrageResult<PriceData>> {
        Box::pin(async move {
            let base_mint = self
                .get_mint(&pair.base)
                .ok_or_else(|| ArbitrageError::Config(format!("Unknown token: {}", pair.base)))?;

            let quote_mint = self
                .get_mint(&pair.quote)
                .ok_or_else(|| ArbitrageError::Config(format!("Unknown token: {}", pair.quote)))?;

            let response = Self::fetch_prices(
                &self.client,
                &self.api_url,
                &self.api_key,
                &format!("{},{}", base_mint, quote_mint),
            )
            .await?;

            let price = Self::pair_price(&response, base_mint, quote_mint)?;

            // Jupiter provides a single price, we estimate bid/ask with a small spread
            let spread = price * Decimal::new(1, 4); // 0.01% spread estimate
            let bid = price - spread;
            let ask = price + spread;

            Ok(PriceData::new(DexType::Jupiter, pair.clone(), bid, ask))
        })
    }

    fn subscribe<'a>(&'a self, pairs: Vec<TokenPair>) -> crate::dex::BoxFuture<'a, ArbitrageResult<PriceStream>> {
        Box::pin(async move {
            let (tx, rx) = mpsc::channel(100);
            let client = self.client.clone();
            let token_mints = self.token_mints.clone();
            let api_url = self.api_url.clone();
            let api_key = self.api_key.clone();

            tokio::spawn(async move {
                // Resolve each pair to (base_mint, quote_mint) once, and batch
                // every mint into a single Price API call per poll cycle.
                let resolved: Vec<(TokenPair, String, String)> = pairs
                    .iter()
                    .filter_map(|pair| {
                        let base = token_mints.get(&pair.base)?.clone();
                        let quote = token_mints.get(&pair.quote)?.clone();
                        Some((pair.clone(), base, quote))
                    })
                    .collect();

                let mut unique_mints: Vec<String> = resolved
                    .iter()
                    .flat_map(|(_, b, q)| [b.clone(), q.clone()])
                    .collect();
                unique_mints.sort();
                unique_mints.dedup();
                let ids = unique_mints.join(",");

                if ids.is_empty() {
                    return;
                }

                loop {
                    if let Ok(prices) =
                        Self::fetch_prices(&client, &api_url, &api_key, &ids).await
                    {
                        for (pair, base_mint, quote_mint) in &resolved {
                            if let Ok(price) = Self::pair_price(&prices, base_mint, quote_mint) {
                                let spread = price * Decimal::new(1, 4);
                                let bid = price - spread;
                                let ask = price + spread;

                                let price_data =
                                    PriceData::new(DexType::Jupiter, pair.clone(), bid, ask);

                                if tx.send(price_data).await.is_err() {
                                    return; // Channel closed
                                }
                            }
                        }
                    }

                    // Poll every 500ms for updates
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            });

            Ok(rx)
        })
    }

    fn health_check<'a>(&'a self) -> crate::dex::BoxFuture<'a, ArbitrageResult<bool>> {
        Box::pin(async move {
            let url = format!(
                "{}?ids=So11111111111111111111111111111111111111112",
                self.api_url
            );
            let req = match &self.api_key {
                Some(key) => self.client.get(&url).header("x-api-key", key),
                None => self.client.get(&url),
            };
            let response = req.send().await?;
            Ok(response.status().is_success())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access - run with: cargo test -- --ignored
    async fn test_jupiter_health_check() {
        let provider = JupiterProvider::new();
        let result = provider.health_check().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_pair_price_from_usd_prices() {
        let mut prices = JupiterPriceResponse::new();
        prices.insert(
            "So11111111111111111111111111111111111111112".to_string(),
            JupiterTokenPrice { usd_price: 64.0 },
        );
        prices.insert(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            JupiterTokenPrice { usd_price: 1.0 },
        );

        let price = JupiterProvider::pair_price(
            &prices,
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        )
        .expect("pair price should compute");
        assert_eq!(price, Decimal::from(64));
    }

    #[test]
    fn test_pair_price_rejects_zero_quote() {
        let mut prices = JupiterPriceResponse::new();
        prices.insert("base".to_string(), JupiterTokenPrice { usd_price: 5.0 });
        prices.insert("quote".to_string(), JupiterTokenPrice { usd_price: 0.0 });

        assert!(JupiterProvider::pair_price(&prices, "base", "quote").is_err());
    }

    #[test]
    fn test_pair_price_missing_mint() {
        let prices = JupiterPriceResponse::new();
        assert!(JupiterProvider::pair_price(&prices, "base", "quote").is_err());
    }
}
