use crate::Strategy;
use async_trait::async_trait;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use solana_arb_core::{
    types::{ArbitrageOpportunity, PriceData},
    ArbitrageResult,
};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct StatisticalArbitrage {
    // Sliding window of price ratios for pairs
    // Key: Pair symbol, Value: Queue of (price_ratio, timestamp)
    history: RwLock<std::collections::HashMap<String, VecDeque<(Decimal, i64)>>>,
    window_size: usize,
    z_score_threshold: Decimal,
}

impl StatisticalArbitrage {
    pub fn new(window_size: usize, z_score_threshold: Decimal) -> Self {
        Self {
            history: RwLock::new(std::collections::HashMap::new()),
            window_size,
            z_score_threshold,
        }
    }

    fn calculate_z_score(
        &self,
        value: Decimal,
        history: &VecDeque<(Decimal, i64)>,
    ) -> Option<Decimal> {
        if history.len() < self.window_size {
            return None;
        }

        let sum: Decimal = history.iter().map(|(v, _)| *v).sum();
        let count = Decimal::from(history.len());
        let mean = sum / count;

        let variance_sum: Decimal = history.iter().map(|(v, _)| (*v - mean) * (*v - mean)).sum();

        if variance_sum.is_zero() {
            return Some(Decimal::ZERO);
        }

        let variance = variance_sum / count;
        // Decimal sqrt is not standard, convert to f64
        let std_dev = variance
            .to_f64()
            .map(|f| f.sqrt())
            .map(Decimal::from_f64_retain)
            .flatten()?;

        if std_dev.is_zero() {
            return Some(Decimal::ZERO); // Should be covered by variance check but safe
        }

        Some((value - mean) / std_dev)
    }
}

#[async_trait]
impl Strategy for StatisticalArbitrage {
    fn name(&self) -> &'static str {
        "Statistical Arbitrage (Mean Reversion)"
    }

    async fn update_state(&self, price: &PriceData) -> ArbitrageResult<()> {
        let mut history = self.history.write().await;
        // Simplified: tracking raw price for now, ideally price ratio between correlated pairs
        let pair_symbol = price.pair.symbol();

        let entry = history.entry(pair_symbol).or_insert_with(VecDeque::new);
        entry.push_back((price.mid_price, price.timestamp.timestamp()));

        if entry.len() > self.window_size {
            entry.pop_front();
        }

        Ok(())
    }

    async fn analyze(&self, prices: &[PriceData]) -> ArbitrageResult<Vec<ArbitrageOpportunity>> {
        let history = self.history.read().await;
        let mut opportunities = Vec::new();

        for price in prices {
            if let Some(queue) = history.get(&price.pair.symbol()) {
                if let Some(z_score) = self.calculate_z_score(price.mid_price, queue) {
                    // Mean reversion logic:
                    // If Z-score > threshold, price is historically high -> SELL or SHORT
                    // If Z-score < -threshold, price is historically low -> BUY or LONG

                    if z_score.abs() > self.z_score_threshold {
                        tracing::info!(
                            "📈 StatArb signal: {} Z-score {} (Threshold {})",
                            price.pair.symbol(),
                            z_score,
                            self.z_score_threshold
                        );
                        // Construct Opportunity object here (omitted for brevity, requires partner Dex/Pool)
                    }
                }
            }
        }

        Ok(opportunities)
    }
}
