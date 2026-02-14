//! Solana Arbitrage Core Library
//!
//! This crate provides shared types, DEX integrations, and arbitrage detection
//! for the Solana Arbitrage Dashboard system.

pub mod arbitrage;
pub mod cache;
pub mod config;
pub mod database;
pub mod dex;
pub mod error;
pub mod flash_loan;
pub mod history;
pub mod http;
pub mod parsers;
pub mod pathfinding;
pub mod pricing;
pub mod risk;
pub mod streaming;
pub mod types;

// Phase 8 modules
pub mod alt;
pub mod jito;

#[cfg(test)]
mod simulation_logs;
#[cfg(test)]
mod tests;

pub use error::*;
pub use types::*;
