use solana_arb_core::{
    types::{ArbitrageOpportunity, PriceData},
    ArbitrageResult,
};

pub mod latency;
pub mod statistical;
pub mod plugin;

pub use latency::LatencyArbitrage;
pub use statistical::StatisticalArbitrage;
pub use plugin::*;

/// Trait for trading strategies
pub trait Strategy: Send + Sync {
    /// Unique name of the strategy
    fn name(&self) -> &'static str;

    /// Analyze price data and generate arbitrage opportunities
    fn analyze(&self, prices: &[PriceData]) -> ArbitrageResult<Vec<ArbitrageOpportunity>>;

    /// Update internal state with new market data (e.g., for moving averages)
    fn update_state(&self, price: &PriceData) -> ArbitrageResult<()>;
}
