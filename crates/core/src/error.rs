//! Error types for the Solana Arbitrage system

use thiserror::Error;

/// Main error type for the arbitrage system
#[derive(Error, Debug)]
pub enum ArbitrageError {
    #[error("DEX connection error: {0}")]
    DexConnection(String),

    #[error("Price fetch error: {0}")]
    PriceFetch(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Http(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Insufficient liquidity: {0}")]
    InsufficientLiquidity(String),

    #[error("Slippage exceeded: expected {expected}%, got {actual}%")]
    SlippageExceeded { expected: f64, actual: f64 },

    #[error("Rate limited by {0}")]
    RateLimited(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

// Conditional From implementations for optional dependencies
// #[cfg(feature = "db")]
// impl From<sqlx::Error> for ArbitrageError {
//     fn from(e: sqlx::Error) -> Self {
//         ArbitrageError::Database(e.to_string())
//     }
// }

#[cfg(feature = "http")]
impl From<reqwest::Error> for ArbitrageError {
    fn from(e: reqwest::Error) -> Self {
        ArbitrageError::Http(e.to_string())
    }
}

#[cfg(feature = "cache")]
impl From<redis::RedisError> for ArbitrageError {
    fn from(e: redis::RedisError) -> Self {
        ArbitrageError::Redis(e.to_string())
    }
}

/// Result type alias for arbitrage operations
pub type ArbitrageResult<T> = Result<T, ArbitrageError>;
