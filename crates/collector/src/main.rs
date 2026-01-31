//! Solana Arbitrage Collector Service
//! 
//! This service collects price data from multiple DEXs and detects
//! arbitrage opportunities in real-time.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use solana_arb_core::{
    arbitrage::ArbitrageDetector,
    config::Config,
    dex::{jupiter::JupiterProvider, orca::OrcaProvider, raydium::RaydiumProvider, DexProvider},
    ArbitrageConfig, TokenPair,
};

/// Default trading pairs to monitor
fn default_pairs() -> Vec<TokenPair> {
    vec![
        TokenPair::new("SOL", "USDC"),
        TokenPair::new("SOL", "USDT"),
        TokenPair::new("RAY", "USDC"),
        TokenPair::new("RAY", "SOL"),
        TokenPair::new("ORCA", "USDC"),
        TokenPair::new("JUP", "USDC"),
        TokenPair::new("BONK", "SOL"),
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Solana Arbitrage Collector");

    // Load configuration
    let config = Config::from_env().unwrap_or_default();
    info!("Configuration loaded");
    info!("  RPC URL: {}", config.solana_rpc_url);
    info!("  Min profit threshold: {}%", config.min_profit_threshold);

    // Initialize DEX providers
    let jupiter = JupiterProvider::new();
    let raydium = RaydiumProvider::new();
    let orca = OrcaProvider::new();

    info!("DEX providers initialized");

    // Health check all providers
    for (name, result) in [
        ("Jupiter", jupiter.health_check().await),
        ("Raydium", raydium.health_check().await),
        ("Orca", orca.health_check().await),
    ] {
        match result {
            Ok(true) => info!("  {} - Connected", name),
            Ok(false) => warn!("  {} - Unhealthy", name),
            Err(e) => warn!("  {} - Error: {}", name, e),
        }
    }

    // Initialize arbitrage detector
    let arb_config = ArbitrageConfig {
        min_profit_threshold: rust_decimal::Decimal::try_from(config.min_profit_threshold)
            .unwrap_or_default(),
        ..Default::default()
    };
    let detector = Arc::new(RwLock::new(ArbitrageDetector::new(arb_config)));

    // Get pairs to monitor
    let pairs = default_pairs();
    info!("Monitoring {} trading pairs", pairs.len());

    // Collect prices from all DEXs
    let providers: Vec<Box<dyn DexProvider>> = vec![
        Box::new(jupiter),
        Box::new(raydium),
        Box::new(orca),
    ];

    // Main collection loop
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    
    info!("Starting price collection loop (500ms interval)");

    loop {
        interval.tick().await;

        // Fetch prices from all providers
        for provider in &providers {
            match provider.get_prices(&pairs).await {
                Ok(prices) => {
                    let mut detector_guard = detector.write().await;
                    detector_guard.update_prices(prices);
                    drop(detector_guard);
                }
                Err(e) => {
                    warn!("Failed to get prices from {}: {}", provider.dex_type(), e);
                }
            }
        }

        // Find opportunities
        let detector_guard = detector.read().await;
        let opportunities = detector_guard.find_all_opportunities();
        drop(detector_guard);

        if !opportunities.is_empty() {
            info!("Found {} arbitrage opportunities:", opportunities.len());
            for opp in opportunities.iter().take(5) {
                info!(
                    "  {} | Buy {} @ {} -> Sell {} @ {} | Net: {:.4}%",
                    opp.pair,
                    opp.buy_dex,
                    opp.buy_price,
                    opp.sell_dex,
                    opp.sell_price,
                    opp.net_profit_pct
                );
            }
        }

        // Clean up stale prices (older than 5 seconds)
        let mut detector_guard = detector.write().await;
        detector_guard.clear_stale_prices(5);
    }
}
