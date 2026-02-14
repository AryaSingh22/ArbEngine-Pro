//! Solana Arbitrage Trading Bot
//!
//! Automated trading bot that executes arbitrage opportunities.

use anyhow::Result;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

mod execution;
mod wallet;
// mod jito; // Migrated to core
mod api;
mod flash_loan_tx_builder;
mod logging;
mod metrics;

use crate::execution::{ORCA_MINT, RAY_MINT, SOL_MINT, USDC_MINT};
use execution::Executor;
use metrics::prometheus::MetricsCollector;
use solana_arb_core::{
    alt::AltManager,
    arbitrage::ArbitrageDetector,
    config::Config,
    dex::{jupiter::JupiterProvider, orca::OrcaProvider, raydium::RaydiumProvider, DexManager},
    history::HistoryRecorder,
    jito::JitoClient,
    pathfinding::PathFinder,
    pricing::parallel_fetcher::ParallelPriceFetcher,
    risk::{RiskConfig, RiskManager, TradeDecision, TradeOutcome},
    types::TradeResult,
    DexType, TokenPair,
};
use solana_arb_dex_plugins::{LifinityProvider, MeteoraProvider, PhoenixProvider};
use solana_arb_flash_loans::solend::SolendFlashLoan;
use solana_arb_flash_loans::FlashLoanProvider;
use solana_arb_strategies::{LatencyArbitrage, StatisticalArbitrage, Strategy};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use wallet::Wallet;

/// Trading bot state
struct BotState {
    detector: ArbitrageDetector,
    path_finder: PathFinder,
    risk_manager: RiskManager,
    dex_manager: DexManager,
    price_fetcher: ParallelPriceFetcher,
    executor: Executor,
    wallet: Wallet,
    flash_loan_provider: Box<dyn FlashLoanProvider>,
    history_recorder: HistoryRecorder,
    jito_client: Option<JitoClient>,
    alt_manager: Arc<AltManager>,
    strategies: Vec<Box<dyn Strategy>>,
    is_running: bool,
    dry_run: bool,
    rpc_url: String,
    max_price_age_seconds: i64,
    metrics: Arc<MetricsCollector>,
}

impl BotState {
    fn new(config: &Config, dry_run: bool, metrics: Arc<MetricsCollector>) -> Self {
        let risk_config = RiskConfig {
            max_position_size: Decimal::from(1000),
            max_total_exposure: Decimal::from(5000),
            max_daily_loss: Decimal::from(100),
            min_profit_threshold: config
                .min_profit_threshold
                .try_into()
                .unwrap_or(Decimal::new(5, 3)),
            ..Default::default()
        };

        let mut dex_manager = DexManager::new();

        // Register DEX providers
        dex_manager.add_provider(Arc::new(JupiterProvider::new()));
        info!("🔌 Registered DEX provider: Jupiter");

        dex_manager.add_provider(Arc::new(RaydiumProvider::new()));
        info!("🔌 Registered DEX provider: Raydium");

        dex_manager.add_provider(Arc::new(OrcaProvider::new()));
        info!("🔌 Registered DEX provider: Orca");

        dex_manager.add_provider(Arc::new(LifinityProvider::new()));
        info!("🔌 Registered DEX provider: Lifinity");

        dex_manager.add_provider(Arc::new(MeteoraProvider::new()));
        info!("🔌 Registered DEX provider: Meteora");

        dex_manager.add_provider(Arc::new(PhoenixProvider::new()));
        info!("🔌 Registered DEX provider: Phoenix");

        info!(
            "🔌 DexManager initialized with {} providers",
            dex_manager.providers().len()
        );

        let price_fetcher = ParallelPriceFetcher::new(dex_manager.providers().to_vec());

        // Initialize Flash Loan Provider (Solend)
        // For now using USDC reserve placeholder - in prod this would be dynamic or config based
        let usdc_reserve =
            Pubkey::from_str("BgxfHJDzm44T7XG68MYKx7YisTjZu73tVovyZSjJMpmw").unwrap(); // Mainnet USDC reserve
        let flash_loan_provider = Box::new(SolendFlashLoan::new(usdc_reserve));
        info!(
            "🏦 Initialized Flash Loan Provider: {}",
            flash_loan_provider.name()
        );

        let temp_session_id = format!("SESSION-{}", Utc::now().format("%Y%m%d-%H%M%S"));
        let history_file = if dry_run {
            "data/history-sim.jsonl"
        } else {
            "data/history-live.jsonl"
        };
        let history_recorder = HistoryRecorder::new(history_file, &temp_session_id);
        info!("📜 Trade history will be saved to: {}", history_file);

        // Initialize Jito Client (Optional)
        let jito_client = if std::env::var("USE_JITO").unwrap_or("false".to_string()) == "true" {
            let engine_url = std::env::var("JITO_BLOCK_ENGINE_URL")
                .unwrap_or("https://mainnet.block-engine.jito.wtf".to_string());
            let tip = std::env::var("JITO_TIP_LAMPORTS")
                .unwrap_or("100000".to_string())
                .parse()
                .unwrap_or(100000);
            info!(
                "🛡️ Jito MEV Protection enabled (Engine: {}, Tip: {} lamports)",
                engine_url, tip
            );
            Some(JitoClient::new(&engine_url, tip))
        } else {
            info!("⚠️ Jito MEV Protection DISABLED");
            None
        };

        // Initialize ALT Manager
        let alt_manager = Arc::new(AltManager::new(&config.solana_rpc_url));
        info!("📇 Address Lookup Table (ALT) Manager initialized");

        // Initialize Strategies
        let mut strategies: Vec<Box<dyn Strategy>> = Vec::new();

        // Statistical Arbitrage (Window: 20 ticks, Z-score: 2.0)
        strategies.push(Box::new(StatisticalArbitrage::new(20, Decimal::new(20, 1))));
        info!("🧠 Strategy initialized: Statistical Arbitrage");

        // Latency Arbitrage
        strategies.push(Box::new(LatencyArbitrage::new()));
        info!("🧠 Strategy initialized: Latency Arbitrage");

        let mut executor = Executor::with_config(execution::ExecutionConfig {
            priority_fee_micro_lamports: config.priority_fee_micro_lamports,
            compute_unit_limit: config.compute_unit_limit,
            slippage_bps: config.slippage_bps,
            max_retries: config.max_retries,
            rpc_commitment: config.rpc_commitment.clone(),
        });

        executor.set_alt_manager(alt_manager.clone());

        Self {
            detector: ArbitrageDetector::default(),
            path_finder: PathFinder::new(4),
            risk_manager: RiskManager::new(risk_config),
            dex_manager,
            price_fetcher,
            executor,
            wallet: Wallet::new().expect("Failed to load wallet"),
            flash_loan_provider,
            history_recorder,
            jito_client,
            alt_manager,
            strategies,
            is_running: true,
            dry_run,
            rpc_url: config.solana_rpc_url.clone(),
            max_price_age_seconds: config.max_price_age_seconds,
            metrics,
        }
    }
}

/// Main trading loop
async fn run_trading_loop(state: Arc<RwLock<BotState>>, pairs: Vec<TokenPair>) {
    info!("🤖 Trading bot started");

    let mut tick = 0u64;

    loop {
        info!("🔎 Scanning markets for arbitrage opportunities...");
        // Check if still running
        {
            let state = state.read().await;
            if !state.is_running {
                info!("Bot stopped");
                break;
            }
        }

        tick += 1;

        // Every 10 ticks, log status
        if tick % 10 == 0 {
            let state = state.read().await;
            let status = state.risk_manager.status().await;
            info!(
                "📊 Status - Exposure: ${:.2}, VaR (95%): ${:.2}, P&L: ${:.2}, Trades: {}, Paused: {}",
                status.total_exposure,
                status.portfolio_var,
                status.daily_pnl,
                status.trades_today,
                status.is_paused
            );
        }

        let start = std::time::Instant::now();

        // Collect prices
        let recent_prices = match collect_prices(&state, &pairs).await {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to collect prices: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        {
            let state = state.read().await;
            state
                .metrics
                .price_fetch_latency
                .observe(start.elapsed().as_secs_f64());
        }

        // Find and evaluate opportunities
        let opportunities = {
            let state = state.read().await;

            // Simple arbitrage opportunities
            let mut opps = state.detector.find_all_opportunities();

            // Also check triangular paths
            let paths = state.path_finder.find_all_profitable_paths();

            debug!(
                "Found {} simple opportunities, {} triangular paths",
                opps.len(),
                paths.len()
            );

            // 🧪 Inject synthetic arbitrage in DRY_RUN mode for demo
            if state.dry_run {
                use rand::seq::SliceRandom;
                use rand::Rng;
                let mut rng = rand::thread_rng();

                // 80% chance to find an opportunity
                if rng.gen_bool(0.8) {
                    if let Some(pair) = pairs.choose(&mut rng) {
                        let dexs = vec![DexType::Raydium, DexType::Orca, DexType::Jupiter];

                        // Pick two different DEXs
                        let buy_dex = dexs.choose(&mut rng).unwrap();
                        let mut sell_dex = dexs.choose(&mut rng).unwrap();
                        while sell_dex == buy_dex {
                            sell_dex = dexs.choose(&mut rng).unwrap();
                        }

                        let profit_basis = rng.gen_range(50..450); // 0.50 to 4.50
                        let profit_pct = Decimal::new(profit_basis, 2);
                        let size = Decimal::from(rng.gen_range(50..500));
                        let est_profit = (size * profit_pct) / Decimal::from(100);

                        // Only log synthetic injection occasionally to reduce noise
                        // info!(
                        //    "🧪 Synthetic arbitrage: {} on {:?} -> {:?}, profit {}%",
                        //    pair, buy_dex, sell_dex, profit_pct
                        // );

                        let synthetic_opp = solana_arb_core::ArbitrageOpportunity {
                            id: solana_arb_core::Uuid::new_v4(),
                            pair: pair.clone(),
                            buy_dex: buy_dex.clone(),
                            sell_dex: sell_dex.clone(),
                            buy_price: Decimal::new(100, 0), // Dummy
                            sell_price: Decimal::new(100, 0)
                                + (Decimal::new(100, 0) * profit_pct / Decimal::from(100)),
                            gross_profit_pct: profit_pct,
                            net_profit_pct: profit_pct,
                            estimated_profit_usd: Some(est_profit),
                            recommended_size: Some(size),
                            detected_at: Utc::now(),
                            expired_at: None,
                        };
                        opps.push(synthetic_opp);
                    }
                }
            }

            state
                .metrics
                .opportunities_detected
                .inc_by(opps.len() as u64);

            // Execute Strategies
            for strategy in &state.strategies {
                match strategy.analyze(&recent_prices).await {
                    Ok(strategy_opps) => {
                        if !strategy_opps.is_empty() {
                            info!(
                                "🧠 Strategy {} found {} opportunities",
                                strategy.name(),
                                strategy_opps.len()
                            );
                            opps.extend(strategy_opps);
                        }
                    }
                    Err(e) => warn!("Strategy {} analysis failed: {}", strategy.name(), e),
                }
            }

            opps
        };

        // Execute best opportunity if profitable
        for opp in opportunities.iter().take(1) {
            let should_execute = {
                let state = state.read().await;

                // Check profit threshold
                if opp.net_profit_pct < Decimal::new(5, 3) {
                    false
                } else {
                    // Calculate optimal size
                    let optimal_size = state.risk_manager.calculate_position_size(
                        &opp.pair.symbol(),
                        opp.net_profit_pct,
                        Decimal::from(10000), // Placeholder liquidity
                    );

                    // Check risk manager
                    let decision = state
                        .risk_manager
                        .can_trade(&opp.pair.symbol(), optimal_size)
                        .await;
                    matches!(
                        decision,
                        TradeDecision::Approved { .. } | TradeDecision::Reduced { .. }
                    )
                }
            };

            if should_execute {
                execute_trade(&state, opp).await;
            }
        }

        // Update balance metric every 100 ticks (approx 50s)
        if tick % 100 == 0 {
            let rpc_url = {
                let state = state.read().await;
                state.rpc_url.clone()
            };
            let pubkey_str = {
                let state = state.read().await;
                state.wallet.pubkey()
            };
            let metrics = {
                let state = state.read().await;
                state.metrics.clone()
            };

            // Try to find SOL price from recent prices
            let sol_price = {
                let state = state.read().await;
                // We don't have direct access to last prices map here easily unless we look at risk manager
                // or we could have captured it from `prices` vec on line 142 if we changed scope.
                // For now, assume 150.0 default or try to get from risk manager if exposed.
                // Let's just use a default for visualization if we can't easily get it.
                150.0
            };

            tokio::task::spawn_blocking(move || {
                use solana_rpc_client::rpc_client::RpcClient;
                use solana_sdk::commitment_config::CommitmentConfig;
                use solana_sdk::pubkey::Pubkey;
                use std::str::FromStr;

                if let Ok(pubkey) = Pubkey::from_str(&pubkey_str) {
                    let client =
                        RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
                    match client.get_balance(&pubkey) {
                        Ok(balance) => {
                            let balance_sol = balance as f64 / 1_000_000_000.0;
                            let balance_usd = balance_sol * sol_price;
                            metrics.current_balance.set(balance_usd);
                            // Also log it
                            // info!("💰 Balance updated: {:.4} SOL (~${:.2})", balance_sol, balance_usd);
                        }
                        Err(e) => {
                            warn!("Failed to fetch balance for metrics: {}", e);
                        }
                    }
                }
            });
        }

        // Sleep before next cycle
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Collect prices from all DEXs
async fn collect_prices(
    state: &Arc<RwLock<BotState>>,
    pairs: &[TokenPair],
) -> Result<Vec<solana_arb_core::PriceData>, Box<dyn std::error::Error>> {
    let prices = {
        let state = state.read().await;

        // Use parallel fetcher for all pairs at once!
        let all_prices = state.price_fetcher.fetch_all_prices(pairs).await;
        info!(
            "💓 Parallel fetch complete — {} prices collected",
            all_prices.len()
        );
        all_prices
    };

    info!("📈 Received price data from DEX ({} prices)", prices.len());

    // Update state
    {
        let mut state = state.write().await;

        // Update detector
        state.detector.update_prices(prices.clone());
        let max_age = state.max_price_age_seconds;
        state.detector.clear_stale_prices(max_age);

        // Update pathfinder
        state.path_finder.clear();
        for price in &prices {
            state.path_finder.add_price(price);
        }

        // Update risk manager volatility tracking
        state.risk_manager.update_prices(&prices);

        // Update strategies
        for strategy in &state.strategies {
            for price in &prices {
                if let Err(e) = strategy.update_state(price).await {
                    warn!("Strategy {} update failed: {}", strategy.name(), e);
                }
            }
        }
    }

    validate_dex_coverage(&prices, pairs);

    Ok(prices)
}

fn validate_dex_coverage(prices: &[solana_arb_core::PriceData], pairs: &[TokenPair]) {
    let mut coverage: std::collections::HashMap<String, std::collections::HashSet<DexType>> =
        std::collections::HashMap::new();

    for price in prices {
        coverage
            .entry(price.pair.symbol())
            .or_default()
            .insert(price.dex);
    }

    for pair in pairs {
        let seen = coverage.get(&pair.symbol());
        let missing: Vec<_> = DexType::all()
            .iter()
            .filter(|dex| seen.map_or(true, |set| !set.contains(dex)))
            .collect();

        if !missing.is_empty() {
            let missing_labels: Vec<_> = missing.iter().map(|dex| dex.display_name()).collect();
            warn!(
                "⚠️ Missing DEX coverage for {}: {}",
                pair,
                missing_labels.join(", ")
            );
        }
    }
}

/// Execute a trade (or simulate in dry-run mode)
async fn execute_trade(state: &Arc<RwLock<BotState>>, opp: &solana_arb_core::ArbitrageOpportunity) {
    let start_time = std::time::Instant::now();
    let pair_symbol = opp.pair.symbol();

    // We need to release the read lock before acquiring write lock later,
    // AND calling async execution which shouldn't hold locks if possible.
    // However, Executor is stateless (HttpClient) so we can clone data needed.

    let (is_dry_run, decision, rpc_url) = {
        let state = state.read().await;

        let optimal_size = state.risk_manager.calculate_position_size(
            &pair_symbol,
            opp.net_profit_pct,
            Decimal::from(10000), // Assume high liquidity for now or get from opp
        );

        let decision = state
            .risk_manager
            .can_trade(&pair_symbol, optimal_size)
            .await;
        (state.dry_run, decision, state.rpc_url.clone())
    };

    let size = match decision {
        TradeDecision::Approved { size } => size,
        TradeDecision::Reduced { new_size, reason } => {
            info!("Trade size reduced: {}", reason);
            new_size
        }
        TradeDecision::Rejected { reason } => {
            debug!("Trade rejected: {}", reason);
            return;
        }
    };

    // Record attempt
    {
        let state = state.read().await;
        state.metrics.trades_attempted.inc();
    }

    // Check Flash Loan Viability
    let flash_loan_quote = {
        let state_read = state.read().await;
        if let Some(mint) = resolve_mint(&opp.pair.base) {
            // Assume borrowing base asset
            match state_read.flash_loan_provider.get_quote(mint, size).await {
                Ok(quote) => {
                    let total_profit_usd = (size * opp.net_profit_pct) / Decimal::from(100);
                    // Assuming quote.fee is in same denomination as amount (base currency)
                    // We need to convert fee to USD to compare with profit, or profit to base.
                    // Simplified: fee is in base token.
                    // If base is SOL ($100), fee 0.09% = 0.0009 SOL.
                    // Profit is % of size.

                    let fee_pct = (quote.fee / size) * Decimal::from(100);

                    if opp.net_profit_pct > fee_pct {
                        info!(
                            "⚡ Flash Loan Viable! Borrowing {} {} costs {} {} ({:.4}%) - Net edge: {:.4}%",
                            size, opp.pair.base, quote.fee, opp.pair.base, fee_pct, opp.net_profit_pct - fee_pct
                        );
                        Some(quote)
                    } else {
                        debug!(
                            "Flash Loan fee too high: {:.4}% > {:.4}% profit",
                            fee_pct, opp.net_profit_pct
                        );
                        None
                    }
                }
                Err(e) => {
                    warn!("Failed to get flash loan quote: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    if is_dry_run {
        // Simulate trade
        info!(
            "🔵 [DRY RUN] Would execute: Buy {} on {}, Sell on {} | Size: ${} | Profit: {}%",
            pair_symbol, opp.buy_dex, opp.sell_dex, size, opp.net_profit_pct
        );

        // Fetch quote simulation (optional)
        {
            let state_read = state.read().await;
            if let Err(e) = state_read
                .executor
                .execute(&state_read.wallet, opp, size, false, &rpc_url, None)
                .await
            {
                warn!("Simulation execution failed: {}", e);
            }
        }

        // Record simulation history
        {
            let state_read = state.read().await;
            let est_profit = (size * opp.net_profit_pct) / Decimal::from(100);
            state_read
                .history_recorder
                .record_trade(opp, size, est_profit, true, None, None, true);
        }

        // Simulate successful outcome
        let outcome = TradeOutcome {
            timestamp: Utc::now(),
            pair: pair_symbol,
            profit_loss: size * opp.net_profit_pct / Decimal::from(100),
            was_successful: true,
        };

        let mut state = state.write().await;
        state.risk_manager.record_trade(outcome).await;
    } else {
        // Real execution via Jupiter API
        info!(
            "🟢 Executing: Buy {} on {}, Sell on {} | Size: ${} | Expected Profit: {}%",
            pair_symbol, opp.buy_dex, opp.sell_dex, size, opp.net_profit_pct
        );

        let result: Result<TradeResult> = {
            let state_read = state.read().await;
            state_read
                .executor
                .execute(
                    &state_read.wallet,
                    opp,
                    size,
                    true,
                    &rpc_url,
                    state_read.jito_client.as_ref(),
                )
                .await
        };

        match result {
            Ok(trade_result) => {
                if trade_result.success {
                    let tx_signature = trade_result
                        .signature
                        .unwrap_or_else(|| "unknown".to_string());
                    info!("✅ Trade submitted! Signature: {}", tx_signature);

                    // Record success metrics
                    {
                        let state = state.read().await;
                        state.metrics.trades_successful.inc();
                        state
                            .metrics
                            .trade_execution_time
                            .observe(start_time.elapsed().as_secs_f64());
                        if let Some(profit_f64) = opp.net_profit_pct.to_f64() {
                            state.metrics.opportunity_profit.observe(profit_f64);
                        }
                    }

                    // Record success
                    let outcome = TradeOutcome {
                        timestamp: Utc::now(),
                        pair: pair_symbol,
                        profit_loss: size * opp.net_profit_pct / Decimal::from(100), // Estimated
                        was_successful: true,
                    };

                    // Record history
                    {
                        let state_read = state.read().await;
                        let est_profit = (size * opp.net_profit_pct) / Decimal::from(100);
                        state_read.history_recorder.record_trade(
                            opp,
                            size,
                            est_profit,
                            true,
                            Some(tx_signature),
                            None,
                            false,
                        );
                    }

                    let mut state = state.write().await;
                    state.risk_manager.record_trade(outcome).await;
                } else {
                    let error_msg = trade_result
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string());
                    warn!("❌ Trade execution returned failure: {}", error_msg);

                    // Record failure metrics
                    {
                        let state = state.read().await;
                        state.metrics.trades_failed.inc();
                    }

                    // Record failure history
                    {
                        let state_read = state.read().await;
                        state_read.history_recorder.record_trade(
                            opp,
                            size,
                            Decimal::ZERO,
                            false,
                            None,
                            Some(error_msg),
                            false,
                        );
                    }
                }
            }
            Err(e) => {
                error!("❌ Trade failed (Executor Error): {}", e);

                // Record failure metrics
                {
                    let state = state.read().await;
                    state.metrics.trades_failed.inc();
                }

                // Record failure history
                {
                    let state_read = state.read().await;
                    state_read.history_recorder.record_trade(
                        opp,
                        size,
                        Decimal::ZERO,
                        false,
                        None,
                        Some(e.to_string()),
                        false,
                    );
                }

                // Record failure
                let outcome = TradeOutcome {
                    timestamp: Utc::now(),
                    pair: pair_symbol,
                    profit_loss: Decimal::ZERO,
                    was_successful: false,
                };
                let mut state = state.write().await;
                state.risk_manager.record_trade(outcome).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Load config first
    dotenvy::dotenv().ok();

    // Initialize logging
    logging::setup();

    // Read MIN_PROFIT_THRESHOLD directly from environment at runtime
    let min_profit_threshold: f64 = std::env::var("MIN_PROFIT_THRESHOLD")
        .unwrap_or_else(|_| "0.5".to_string())
        .parse()
        .expect("Invalid MIN_PROFIT_THRESHOLD value");

    // Create config with runtime-loaded value
    let mut config = Config::from_env().unwrap_or_default();
    config.min_profit_threshold = min_profit_threshold;

    info!("🚀 Solana Arbitrage Bot starting...");
    info!("   Min profit threshold: {}%", min_profit_threshold);
    info!(
        "   Priority fee: {} µL/CU",
        config.priority_fee_micro_lamports
    );
    info!("   Slippage tolerance: {} bps", config.slippage_bps);
    info!("   RPC commitment: {}", config.rpc_commitment);
    info!("   Max retries: {}", config.max_retries);
    info!("   RPC URL: {}", config.solana_rpc_url);

    // Check for dry-run mode
    let dry_run = std::env::var("DRY_RUN")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true); // Default to dry-run for safety

    if dry_run {
        info!("⚠️  Running in DRY RUN mode - no real trades will be executed");
    } else {
        warn!("⚠️  LIVE TRADING MODE - Real trades will be executed!");
    }

    // Define trading pairs
    let pairs = vec![
        TokenPair::new("SOL", "USDC"),
        TokenPair::new("RAY", "USDC"),
        TokenPair::new("ORCA", "USDC"),
        TokenPair::new("JUP", "USDC"),
    ];

    // Initialize metrics
    let metrics = Arc::new(MetricsCollector::new().expect("Failed to initialize metrics"));

    // Start metrics server
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        let app = api::metrics::metrics_routes(metrics_clone);
        let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await.unwrap();
        info!("📊 Metrics server running on http://0.0.0.0:9090/metrics");
        axum::serve(listener, app).await.unwrap();
    });

    // Create bot state
    let state = Arc::new(RwLock::new(BotState::new(&config, dry_run, metrics)));

    // Run trading loop
    run_trading_loop(state, pairs).await;
}

fn resolve_mint(symbol: &str) -> Option<Pubkey> {
    match symbol {
        "SOL" => Pubkey::from_str(SOL_MINT).ok(),
        "USDC" => Pubkey::from_str(USDC_MINT).ok(),
        "RAY" => Pubkey::from_str(RAY_MINT).ok(),
        "ORCA" => Pubkey::from_str(ORCA_MINT).ok(),
        "JUP" => None, // JUP mint not in constants yet, can add later or ignore
        _ => None,
    }
}
