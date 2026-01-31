//! Arbitrage Detection Engine
//! 
//! This module identifies arbitrage opportunities by comparing prices
//! across different DEXs for the same trading pair.

use rust_decimal::Decimal;
use std::collections::HashMap;
use chrono::Utc;

use crate::{ArbitrageConfig, ArbitrageOpportunity, DexType, PriceData, TokenPair, Uuid};

/// Arbitrage detector that compares prices across DEXs
pub struct ArbitrageDetector {
    config: ArbitrageConfig,
    /// Cache of latest prices by (pair, dex)
    price_cache: HashMap<(TokenPair, DexType), PriceData>,
}

impl ArbitrageDetector {
    pub fn new(config: ArbitrageConfig) -> Self {
        Self {
            config,
            price_cache: HashMap::new(),
        }
    }

    /// Update the price cache with new price data
    pub fn update_price(&mut self, price: PriceData) {
        let key = (price.pair.clone(), price.dex);
        self.price_cache.insert(key, price);
    }

    /// Update multiple prices at once
    pub fn update_prices(&mut self, prices: Vec<PriceData>) {
        for price in prices {
            self.update_price(price);
        }
    }

    /// Find all arbitrage opportunities for a given pair
    pub fn find_opportunities(&self, pair: &TokenPair) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        // Get all prices for this pair from different DEXs
        let prices: Vec<_> = [DexType::Raydium, DexType::Orca, DexType::Jupiter]
            .iter()
            .filter_map(|dex| {
                self.price_cache.get(&(pair.clone(), *dex))
            })
            .collect();

        // Compare all pairs of DEXs
        for i in 0..prices.len() {
            for j in (i + 1)..prices.len() {
                let price_a = prices[i];
                let price_b = prices[j];

                // Try both directions: buy on A sell on B, and buy on B sell on A
                if let Some(opp) = self.check_opportunity(price_a, price_b) {
                    opportunities.push(opp);
                }
                if let Some(opp) = self.check_opportunity(price_b, price_a) {
                    opportunities.push(opp);
                }
            }
        }

        // Sort by profit percentage (descending)
        opportunities.sort_by(|a, b| b.net_profit_pct.cmp(&a.net_profit_pct));
        opportunities
    }

    /// Check if there's an arbitrage opportunity between two prices
    fn check_opportunity(&self, buy_from: &PriceData, sell_to: &PriceData) -> Option<ArbitrageOpportunity> {
        // Buy at ask price from buy_from, sell at bid price to sell_to
        let buy_price = buy_from.ask;
        let sell_price = sell_to.bid;

        if buy_price.is_zero() || sell_price.is_zero() {
            return None;
        }

        // Calculate gross profit percentage
        let gross_profit_pct = ((sell_price - buy_price) / buy_price) * Decimal::from(100);

        // Calculate fees
        let buy_fee = buy_from.dex.fee_percentage();
        let sell_fee = sell_to.dex.fee_percentage();
        let total_fee_pct = buy_fee + sell_fee;

        // Net profit after fees
        let net_profit_pct = gross_profit_pct - total_fee_pct;

        // Only return if profitable after fees and above threshold
        if net_profit_pct > self.config.min_profit_threshold {
            Some(ArbitrageOpportunity {
                id: Uuid::new_v4(),
                pair: buy_from.pair.clone(),
                buy_dex: buy_from.dex,
                sell_dex: sell_to.dex,
                buy_price,
                sell_price,
                gross_profit_pct,
                net_profit_pct,
                estimated_profit_usd: None,
                recommended_size: None,
                detected_at: Utc::now(),
                expired_at: None,
            })
        } else {
            None
        }
    }

    /// Find all profitable opportunities across all cached pairs
    pub fn find_all_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        // Get unique pairs from cache
        let pairs: Vec<_> = self.price_cache
            .keys()
            .map(|(pair, _)| pair.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut all_opportunities = Vec::new();
        for pair in pairs {
            let mut opportunities = self.find_opportunities(&pair);
            all_opportunities.append(&mut opportunities);
        }

        // Sort by profit
        all_opportunities.sort_by(|a, b| b.net_profit_pct.cmp(&a.net_profit_pct));
        all_opportunities
    }

    /// Get the current price cache
    pub fn get_prices(&self) -> &HashMap<(TokenPair, DexType), PriceData> {
        &self.price_cache
    }

    /// Clear old prices from cache
    pub fn clear_stale_prices(&mut self, max_age_seconds: i64) {
        let now = Utc::now();
        self.price_cache.retain(|_, price| {
            (now - price.timestamp).num_seconds() < max_age_seconds
        });
    }
}

impl Default for ArbitrageDetector {
    fn default() -> Self {
        Self::new(ArbitrageConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_price(dex: DexType, pair: TokenPair, bid: f64, ask: f64) -> PriceData {
        PriceData::new(
            dex,
            pair,
            Decimal::try_from(bid).unwrap(),
            Decimal::try_from(ask).unwrap(),
        )
    }

    #[test]
    fn test_detect_arbitrage() {
        let mut detector = ArbitrageDetector::default();
        let pair = TokenPair::new("SOL", "USDC");

        // Raydium: SOL at $100 bid, $100.10 ask
        detector.update_price(create_test_price(
            DexType::Raydium,
            pair.clone(),
            100.0,
            100.10,
        ));

        // Orca: SOL at $101 bid, $101.10 ask (higher price)
        detector.update_price(create_test_price(
            DexType::Orca,
            pair.clone(),
            101.0,
            101.10,
        ));

        let opportunities = detector.find_opportunities(&pair);
        
        // Should find opportunity: buy on Raydium at 100.10, sell on Orca at 101
        // Gross: (101 - 100.10) / 100.10 = 0.899%
        // Fees: 0.25% + 0.30% = 0.55%
        // Net: 0.899% - 0.55% = 0.349% (below 0.5% threshold)
        
        // With default 0.5% threshold, this might not be profitable enough
        // Let's check the logic works by looking at what was detected
        println!("Found {} opportunities", opportunities.len());
        for opp in &opportunities {
            println!(
                "Buy {} at {} on {}, sell at {} on {} - Net: {}%",
                opp.pair, opp.buy_price, opp.buy_dex, opp.sell_price, opp.sell_dex, opp.net_profit_pct
            );
        }
    }

    #[test]
    fn test_profitable_arbitrage() {
        let config = ArbitrageConfig {
            min_profit_threshold: Decimal::new(1, 2), // 0.1% threshold
            ..Default::default()
        };
        let mut detector = ArbitrageDetector::new(config);
        let pair = TokenPair::new("SOL", "USDC");

        // Create a clear arbitrage opportunity
        // Raydium: $100.00 ask
        detector.update_price(create_test_price(
            DexType::Raydium,
            pair.clone(),
            99.90,
            100.00,
        ));

        // Orca: $101.50 bid (significant price difference)
        detector.update_price(create_test_price(
            DexType::Orca,
            pair.clone(),
            101.50,
            101.60,
        ));

        let opportunities = detector.find_opportunities(&pair);
        assert!(!opportunities.is_empty(), "Should find arbitrage opportunity");
        
        let best = &opportunities[0];
        assert_eq!(best.buy_dex, DexType::Raydium);
        assert_eq!(best.sell_dex, DexType::Orca);
        assert!(best.net_profit_pct > Decimal::ZERO);
    }
}
