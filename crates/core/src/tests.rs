//! Tests for arbitrage detection and types

use crate::{
    arbitrage::ArbitrageDetector,
    types::{ArbitrageConfig, DexType, PriceData, TokenPair},
};
use rust_decimal::Decimal;

/// Helper to create test price data
fn make_price(dex: DexType, base: &str, quote: &str, bid: f64, ask: f64) -> PriceData {
    PriceData::new(
        dex,
        TokenPair::new(base, quote),
        Decimal::try_from(bid).unwrap(),
        Decimal::try_from(ask).unwrap(),
    )
}

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn test_token_pair_creation() {
        let pair = TokenPair::new("SOL", "USDC");
        assert_eq!(pair.base, "SOL");
        assert_eq!(pair.quote, "USDC");
        assert_eq!(pair.symbol(), "SOL/USDC");
    }

    #[test]
    fn test_dex_type_fees() {
        assert_eq!(DexType::Raydium.fee_percentage(), Decimal::new(25, 4)); // 0.25%
        assert_eq!(DexType::Orca.fee_percentage(), Decimal::new(30, 4)); // 0.30%
        assert_eq!(DexType::Jupiter.fee_percentage(), Decimal::new(0, 4)); // 0%
    }

    #[test]
    fn test_price_data_spread() {
        let price = make_price(DexType::Raydium, "SOL", "USDC", 99.0, 101.0);
        let spread = price.spread_percentage();
        // Spread = (101 - 99) / 100 * 100 = 2%
        assert!(spread > Decimal::from(1) && spread < Decimal::from(3));
    }

    #[test]
    fn test_price_data_mid_price() {
        let price = make_price(DexType::Orca, "SOL", "USDC", 100.0, 102.0);
        assert_eq!(price.mid_price, Decimal::from(101));
    }
}

#[cfg(test)]
mod arbitrage_tests {
    use super::*;
    use chrono::Duration;

    fn create_detector_with_low_threshold() -> ArbitrageDetector {
        let config = ArbitrageConfig {
            min_profit_threshold: Decimal::new(1, 3), // 0.1%
            ..Default::default()
        };
        ArbitrageDetector::new(config)
    }

    #[test]
    fn test_no_opportunity_same_prices() {
        let mut detector = create_detector_with_low_threshold();
        let pair = TokenPair::new("SOL", "USDC");

        // Same prices on both DEXs
        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 100.0, 100.1));
        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 100.0, 100.1));

        let opportunities = detector.find_opportunities(&pair);
        assert!(opportunities.is_empty());
    }

    #[test]
    fn test_detect_clear_arbitrage() {
        let mut detector = create_detector_with_low_threshold();
        let pair = TokenPair::new("SOL", "USDC");

        // Raydium: ask $100 (buy here)
        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 99.9, 100.0));

        // Orca: bid $102 (sell here) - 2% difference
        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 102.0, 102.1));

        let opportunities = detector.find_opportunities(&pair);
        assert!(
            !opportunities.is_empty(),
            "Should find arbitrage opportunity"
        );

        let best = &opportunities[0];
        assert_eq!(best.buy_dex, DexType::Raydium);
        assert_eq!(best.sell_dex, DexType::Orca);
        assert!(best.net_profit_pct > Decimal::ZERO);
    }

    #[test]
    fn test_arbitrage_respects_threshold() {
        let config = ArbitrageConfig {
            min_profit_threshold: Decimal::from(5), // 5% threshold
            ..Default::default()
        };
        let mut detector = ArbitrageDetector::new(config);
        let pair = TokenPair::new("SOL", "USDC");

        // Only 1% difference - below threshold
        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 99.9, 100.0));
        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 101.0, 101.1));

        let opportunities = detector.find_opportunities(&pair);
        assert!(
            opportunities.is_empty(),
            "Should not find opportunity below threshold"
        );
    }

    #[test]
    fn test_multiple_dexs() {
        let mut detector = create_detector_with_low_threshold();
        let pair = TokenPair::new("SOL", "USDC");

        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 99.9, 100.0));
        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 101.0, 101.1));
        detector.update_price(make_price(DexType::Jupiter, "SOL", "USDC", 102.0, 102.1));

        let opportunities = detector.find_opportunities(&pair);

        // Should find multiple opportunities: RAY->ORC, RAY->JUP, ORC->JUP
        assert!(opportunities.len() >= 2);
    }

    #[test]
    fn test_clear_stale_prices() {
        let mut detector = create_detector_with_low_threshold();

        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 100.0, 100.1));

        // Clear prices older than 0 seconds (all prices)
        detector.clear_stale_prices(0);

        assert!(detector.get_prices().is_empty());
    }

    #[test]
    fn test_find_all_opportunities() {
        let mut detector = create_detector_with_low_threshold();

        // SOL/USDC pair
        detector.update_price(make_price(DexType::Raydium, "SOL", "USDC", 99.9, 100.0));
        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 102.0, 102.1));

        // RAY/USDC pair
        detector.update_price(make_price(DexType::Raydium, "RAY", "USDC", 1.99, 2.0));
        detector.update_price(make_price(DexType::Jupiter, "RAY", "USDC", 2.1, 2.11));

        let all = detector.find_all_opportunities();
        assert!(
            all.len() >= 2,
            "Should find opportunities across multiple pairs"
        );
    }

    #[test]
    fn test_full_arbitrage_cycle_with_stale_prices() {
        let mut detector = create_detector_with_low_threshold();

        let mut raydium_price = make_price(DexType::Raydium, "SOL", "USDC", 99.9, 100.0);
        raydium_price.timestamp = raydium_price.timestamp - Duration::seconds(10);
        detector.update_price(raydium_price);

        detector.update_price(make_price(DexType::Orca, "SOL", "USDC", 102.0, 102.1));
        detector.update_price(make_price(DexType::Jupiter, "SOL", "USDC", 101.0, 101.1));

        detector.clear_stale_prices(5);

        let opportunities = detector.find_opportunities(&TokenPair::new("SOL", "USDC"));
        assert!(
            opportunities
                .iter()
                .all(|opp| opp.buy_dex != DexType::Raydium),
            "Stale prices should not contribute to opportunities"
        );
    }
}

#[cfg(test)]
mod config_tests {
    use crate::config::Config;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.api_port, 8080);
        assert_eq!(config.min_profit_threshold, 0.5);
        assert_eq!(config.max_price_age_seconds, 5);
        assert!(config.solana_rpc_url.contains("solana"));
    }
}
