//! Price sanity guard
//!
//! A single bad feed (stale cache, decimal bug, wrong pool decoded) shows up
//! as a huge spread against every healthy feed and would otherwise be traded
//! as a phantom arbitrage. This filter drops quotes whose mid price deviates
//! too far from the per-pair median when enough independent sources exist to
//! tell who the outlier is.

use std::collections::HashMap;

use rust_decimal::Decimal;
use tracing::warn;

use crate::PriceData;

/// Minimum number of sources for a pair before outliers can be identified.
const MIN_SOURCES_FOR_FILTERING: usize = 3;

/// Drop prices deviating more than `max_deviation_pct` (e.g. 20 = 20%) from
/// the per-pair median mid price. Pairs with fewer than three sources are
/// passed through unchanged — with two sources there is no way to know which
/// one is wrong, and the profit threshold still guards execution.
pub fn filter_price_outliers(prices: Vec<PriceData>, max_deviation_pct: Decimal) -> Vec<PriceData> {
    let mut by_pair: HashMap<String, Vec<Decimal>> = HashMap::new();
    for price in &prices {
        by_pair
            .entry(price.pair.symbol())
            .or_default()
            .push(price.mid_price);
    }

    let medians: HashMap<String, Decimal> = by_pair
        .into_iter()
        .filter(|(_, mids)| mids.len() >= MIN_SOURCES_FOR_FILTERING)
        .map(|(symbol, mut mids)| {
            mids.sort();
            (symbol, mids[mids.len() / 2])
        })
        .collect();

    let max_deviation = max_deviation_pct / Decimal::from(100);

    prices
        .into_iter()
        .filter(|price| {
            let Some(&median) = medians.get(&price.pair.symbol()) else {
                return true;
            };
            if median.is_zero() {
                return true;
            }
            let deviation = ((price.mid_price - median) / median).abs();
            if deviation > max_deviation {
                warn!(
                    "🚧 Dropping outlier price for {} on {:?}: mid {} deviates {:.1}% from median {}",
                    price.pair,
                    price.dex,
                    price.mid_price,
                    deviation * Decimal::from(100),
                    median
                );
                false
            } else {
                true
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DexType, TokenPair};

    fn price(dex: DexType, mid: i64) -> PriceData {
        let mid = Decimal::from(mid);
        PriceData::new(dex, TokenPair::new("SOL", "USDC"), mid, mid)
    }

    #[test]
    fn drops_outlier_with_three_sources() {
        let prices = vec![
            price(DexType::Jupiter, 100),
            price(DexType::Raydium, 101),
            price(DexType::Orca, 150), // 49% off the median
        ];
        let filtered = filter_price_outliers(prices, Decimal::from(20));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.dex != DexType::Orca));
    }

    #[test]
    fn keeps_everything_within_tolerance() {
        let prices = vec![
            price(DexType::Jupiter, 100),
            price(DexType::Raydium, 102),
            price(DexType::Orca, 98),
        ];
        assert_eq!(filter_price_outliers(prices, Decimal::from(20)).len(), 3);
    }

    #[test]
    fn passes_through_with_two_sources() {
        // With two sources the outlier is unidentifiable — keep both.
        let prices = vec![price(DexType::Jupiter, 100), price(DexType::Orca, 200)];
        assert_eq!(filter_price_outliers(prices, Decimal::from(20)).len(), 2);
    }

    #[test]
    fn different_pairs_filtered_independently() {
        let mut prices = vec![
            price(DexType::Jupiter, 100),
            price(DexType::Raydium, 100),
            price(DexType::Orca, 300),
        ];
        let ray = Decimal::from(5);
        prices.push(PriceData::new(
            DexType::Raydium,
            TokenPair::new("RAY", "USDC"),
            ray,
            ray,
        ));
        let filtered = filter_price_outliers(prices, Decimal::from(20));
        // SOL outlier dropped, lone RAY quote untouched.
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn empty_input() {
        assert!(filter_price_outliers(Vec::new(), Decimal::from(20)).is_empty());
    }
}
