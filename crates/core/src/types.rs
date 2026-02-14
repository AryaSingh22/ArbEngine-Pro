//! Core types for the Solana Arbitrage system

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Supported DEX types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DexType {
    Raydium,
    Orca,
    Jupiter,
    Lifinity,
    Meteora,
    Phoenix,
}

impl DexType {
    /// Returns the trading fee percentage for this DEX
    pub fn fee_percentage(&self) -> Decimal {
        match self {
            DexType::Raydium => Decimal::new(25, 4),  // 0.25%
            DexType::Orca => Decimal::new(30, 4),     // 0.30%
            DexType::Jupiter => Decimal::new(0, 4),   // Variable, aggregator
            DexType::Lifinity => Decimal::new(10, 4), // 0.10% (approx)
            DexType::Meteora => Decimal::new(10, 4),  // Dynamic, varies
            DexType::Phoenix => Decimal::new(5, 4),   // 0.05% (maker/taker varies)
        }
    }

    /// Returns the display name for this DEX
    pub fn display_name(&self) -> &'static str {
        match self {
            DexType::Raydium => "Raydium",
            DexType::Orca => "Orca",
            DexType::Jupiter => "Jupiter",
            DexType::Lifinity => "Lifinity",
            DexType::Meteora => "Meteora",
            DexType::Phoenix => "Phoenix",
        }
    }

    /// Returns all supported DEXs in priority order.
    pub fn all() -> &'static [DexType] {
        const ALL: &[DexType] = &[
            DexType::Raydium,
            DexType::Orca,
            DexType::Jupiter,
            DexType::Lifinity,
            DexType::Meteora,
            DexType::Phoenix,
        ];
        ALL
    }
}

impl std::fmt::Display for DexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Represents a trading pair of tokens
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenPair {
    /// Base token mint address or symbol
    pub base: String,
    /// Quote token mint address or symbol
    pub quote: String,
}

impl TokenPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }

    /// Returns the pair as a symbol string (e.g., "SOL/USDC")
    pub fn symbol(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl std::fmt::Display for TokenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

/// Price data from a DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    /// The DEX this price is from
    pub dex: DexType,
    /// The trading pair
    pub pair: TokenPair,
    /// Best bid price (highest buy order)
    pub bid: Decimal,
    /// Best ask price (lowest sell order)
    pub ask: Decimal,
    /// Mid price (average of bid and ask)
    pub mid_price: Decimal,
    /// 24-hour trading volume in quote currency
    pub volume_24h: Option<Decimal>,
    /// Available liquidity depth
    pub liquidity: Option<Decimal>,
    /// Timestamp when this price was recorded
    pub timestamp: DateTime<Utc>,
}

impl PriceData {
    pub fn new(dex: DexType, pair: TokenPair, bid: Decimal, ask: Decimal) -> Self {
        let mid_price = (bid + ask) / Decimal::from(2);
        Self {
            dex,
            pair,
            bid,
            ask,
            mid_price,
            volume_24h: None,
            liquidity: None,
            timestamp: Utc::now(),
        }
    }

    /// Spread as a percentage
    pub fn spread_percentage(&self) -> Decimal {
        if self.mid_price.is_zero() {
            return Decimal::ZERO;
        }
        ((self.ask - self.bid) / self.mid_price) * Decimal::from(100)
    }
}

/// An arbitrage opportunity between two DEXs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Unique identifier
    pub id: uuid::Uuid,
    /// Token pair for this opportunity
    pub pair: TokenPair,
    /// DEX to buy from (lower price)
    pub buy_dex: DexType,
    /// DEX to sell on (higher price)
    pub sell_dex: DexType,
    /// Buy price (ask from buy DEX)
    pub buy_price: Decimal,
    /// Sell price (bid from sell DEX)
    pub sell_price: Decimal,
    /// Gross profit percentage before fees
    pub gross_profit_pct: Decimal,
    /// Net profit percentage after fees
    pub net_profit_pct: Decimal,
    /// Estimated profit in quote currency for a given trade size
    pub estimated_profit_usd: Option<Decimal>,
    /// Recommended trade size in base currency
    pub recommended_size: Option<Decimal>,
    /// When this opportunity was detected
    pub detected_at: DateTime<Utc>,
    /// When this opportunity expired (filled or price changed)
    pub expired_at: Option<DateTime<Utc>>,
}

impl ArbitrageOpportunity {
    /// Check if this opportunity is still active
    pub fn is_active(&self) -> bool {
        self.expired_at.is_none()
    }

    /// Duration this opportunity has been active
    pub fn duration(&self) -> chrono::Duration {
        let end = self.expired_at.unwrap_or_else(Utc::now);
        end - self.detected_at
    }
}

/// Configuration for arbitrage detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageConfig {
    /// Minimum profit percentage to consider (after fees)
    pub min_profit_threshold: Decimal,
    /// Maximum position size in USD
    pub max_position_size: Decimal,
    /// Slippage tolerance percentage
    pub slippage_tolerance: Decimal,
    /// Solana transaction fee in SOL
    pub solana_tx_fee: Decimal,
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: Decimal::new(50, 4), // 0.5%
            max_position_size: Decimal::from(1000),    // $1,000
            slippage_tolerance: Decimal::new(100, 4),  // 1%
            solana_tx_fee: Decimal::new(5, 6),         // 0.000005 SOL
        }
    }
}

/// Trade execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    /// The opportunity that was executed
    pub opportunity_id: uuid::Uuid,
    /// Transaction signature
    pub signature: Option<String>,
    /// Whether the trade was successful
    pub success: bool,
    /// Actual profit/loss in quote currency
    pub actual_profit: Decimal,
    /// Execution timestamp
    pub executed_at: DateTime<Utc>,
    /// Error message if failed
    pub error: Option<String>,
}

// Re-export uuid for convenience
pub use uuid::Uuid;
