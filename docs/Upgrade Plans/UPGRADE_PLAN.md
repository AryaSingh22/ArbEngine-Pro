# ArbEngine-Pro - Phase-by-Phase Upgrade Plan

## 🎯 End Goals
1. **Maximum Profitability**: Capture more arbitrage opportunities with higher success rates
2. **Minimal Latency**: Reduce execution time from opportunity detection to trade completion
3. **Battle-Tested Reliability**: 99.9% uptime with comprehensive risk management
4. **Scalability**: Handle 100+ DEX pairs with minimal performance degradation
5. **Maintainability**: Clean, modular architecture that's easy to extend

---

## 📋 Overview of Phases

| Phase | Focus | Timeline | Difficulty |
|-------|-------|----------|------------|
| **Phase 4** | Performance & Latency Optimization | 2-3 weeks | Medium |
| **Phase 5** | Advanced Risk & Safety | 2 weeks | Medium |
| **Phase 6** | Data Infrastructure & Analytics | 2 weeks | Medium |
| **Phase 7** | Flash Loans & Capital Efficiency | 2-3 weeks | High |
| **Phase 8** | Advanced Execution & MEV | 2-3 weeks | High |
| **Phase 9** | Additional DEX Integration | 2 weeks | Low-Medium |
| **Phase 10** | Multi-Strategy Engine | 3 weeks | High |
| **Add-Ons** | Optional Enhancements | Ongoing | Variable |

---

# PHASE 4: Performance & Latency Optimization 🚀
**Goal**: Reduce total latency from 500ms+ to <100ms for opportunity detection

## 4.1 Parallel Price Fetching
**Impact**: 3-5x faster price updates
**Location**: `crates/core/src/pricing/`

### Implementation Steps:

1. **Create Concurrent Price Fetcher**
```rust
// File: crates/core/src/pricing/parallel_fetcher.rs

use tokio::task::JoinSet;
use std::time::Instant;

pub struct ParallelPriceFetcher {
    dex_clients: Vec<Box<dyn DexClient>>,
    max_concurrent: usize,
}

impl ParallelPriceFetcher {
    pub async fn fetch_all_prices(&self, pairs: &[TradingPair]) -> PriceSnapshot {
        let start = Instant::now();
        let mut join_set = JoinSet::new();
        
        // Spawn concurrent tasks for each DEX
        for dex in &self.dex_clients {
            let dex_clone = dex.clone();
            let pairs_clone = pairs.to_vec();
            
            join_set.spawn(async move {
                dex_clone.fetch_prices(&pairs_clone).await
            });
        }
        
        // Collect results as they complete
        let mut all_prices = Vec::new();
        while let Some(result) = join_set.join_next().await {
            if let Ok(Ok(prices)) = result {
                all_prices.extend(prices);
            }
        }
        
        tracing::debug!(
            elapsed_ms = start.elapsed().as_millis(),
            price_count = all_prices.len(),
            "Parallel price fetch completed"
        );
        
        PriceSnapshot::from_prices(all_prices)
    }
}
```

2. **Add Connection Pooling**
```rust
// File: crates/core/src/http/pool.rs

use reqwest::{Client, ClientBuilder};
use std::time::Duration;

pub fn create_optimized_client() -> Client {
    ClientBuilder::new()
        .pool_max_idle_per_host(50)  // Keep connections alive
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_nodelay(true)  // Disable Nagle's algorithm
        .timeout(Duration::from_millis(500))
        .build()
        .expect("Failed to create HTTP client")
}
```

3. **Update Main Loop**
```rust
// File: crates/bot/src/main.rs

let fetcher = ParallelPriceFetcher::new(dex_clients);

loop {
    let prices = fetcher.fetch_all_prices(&trading_pairs).await;
    let opportunities = pathfinder.find_opportunities(&prices);
    
    // Process opportunities...
    
    tokio::time::sleep(Duration::from_millis(100)).await;  // Reduced from 500ms
}
```

**Testing Checklist**:
- [ ] Benchmark latency improvement (should see 60-70% reduction)
- [ ] Verify no data loss during parallel fetching
- [ ] Test with 10+ DEXs to ensure scalability
- [ ] Monitor CPU usage (should not exceed 40%)

---

## 4.2 WebSocket Price Streaming
**Impact**: Real-time updates instead of polling
**Location**: `crates/core/src/streaming/`

### Implementation Steps:

1. **Create WebSocket Manager**
```rust
// File: crates/core/src/streaming/ws_manager.rs

use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{StreamExt, SinkExt};

pub struct WebSocketManager {
    connections: HashMap<String, WebSocketConnection>,
    price_tx: mpsc::Sender<PriceUpdate>,
}

impl WebSocketManager {
    pub async fn subscribe_to_pair(&mut self, dex: &str, pair: TradingPair) {
        let url = match dex {
            "jupiter" => format!("wss://quote-api.jup.ag/v6/quote-ws"),
            "raydium" => format!("wss://api.raydium.io/v2/main/price/{}", pair),
            _ => return,
        };
        
        let (ws_stream, _) = connect_async(url).await.unwrap();
        let (mut write, mut read) = ws_stream.split();
        
        // Subscribe to pair
        let subscribe_msg = json!({
            "method": "subscribe",
            "params": [pair.to_string()]
        });
        write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
        
        // Listen for updates
        let price_tx = self.price_tx.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(price_update) = serde_json::from_str::<PriceUpdate>(&text) {
                        let _ = price_tx.send(price_update).await;
                    }
                }
            }
        });
    }
}
```

2. **Hybrid Approach (WebSocket + Polling Fallback)**
```rust
// File: crates/core/src/pricing/hybrid_fetcher.rs

pub struct HybridPriceFetcher {
    ws_manager: WebSocketManager,
    http_fetcher: ParallelPriceFetcher,
    price_cache: Arc<RwLock<HashMap<TradingPair, Price>>>,
}

impl HybridPriceFetcher {
    pub async fn start(&mut self) {
        // Start WebSocket streams
        self.ws_manager.start().await;
        
        // Fallback HTTP polling for DEXs without WS
        tokio::spawn(async move {
            loop {
                let prices = http_fetcher.fetch_all_prices(&fallback_pairs).await;
                // Update cache
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }
    
    pub async fn get_latest_price(&self, pair: &TradingPair) -> Option<Price> {
        self.price_cache.read().await.get(pair).cloned()
    }
}
```

**Testing Checklist**:
- [ ] Test WebSocket reconnection on disconnect
- [ ] Verify HTTP fallback works when WS unavailable
- [ ] Monitor message throughput (should handle 1000+ msg/sec)
- [ ] Test with network interruptions

---

## 4.3 Zero-Copy JSON Parsing
**Impact**: 30-40% faster deserialization
**Location**: `crates/core/src/parsers/`

### Implementation Steps:

1. **Add Dependencies**
```toml
# File: Cargo.toml

[dependencies]
simd-json = "0.13"
# OR
sonic-rs = "0.3"
```

2. **Create Fast Parser**
```rust
// File: crates/core/src/parsers/fast_json.rs

use simd_json::{BorrowedValue, ValueAccess};

pub struct FastJsonParser;

impl FastJsonParser {
    pub fn parse_price_update(data: &mut [u8]) -> Result<PriceUpdate> {
        // Use simd-json for ~2-3x faster parsing
        let parsed = simd_json::to_borrowed_value(data)?;
        
        Ok(PriceUpdate {
            pair: parsed["pair"].as_str().unwrap().to_string(),
            price: parsed["price"].as_f64().unwrap(),
            timestamp: parsed["timestamp"].as_u64().unwrap(),
            source: parsed["source"].as_str().unwrap().to_string(),
        })
    }
    
    pub fn parse_batch(data: &mut [u8]) -> Result<Vec<PriceUpdate>> {
        let parsed = simd_json::to_borrowed_value(data)?;
        let array = parsed.as_array().ok_or("Expected array")?;
        
        array.iter()
            .filter_map(|item| self.parse_single_item(item).ok())
            .collect()
    }
}
```

3. **Integrate into Price Fetcher**
```rust
// File: crates/core/src/pricing/raydium_client.rs

pub async fn fetch_prices(&self, pairs: &[TradingPair]) -> Result<Vec<Price>> {
    let mut response_bytes = self.http_client
        .get(&self.api_url)
        .send()
        .await?
        .bytes()
        .await?
        .to_vec();
    
    // Use fast parser instead of serde_json
    let updates = FastJsonParser::parse_batch(&mut response_bytes)?;
    
    Ok(updates.into_iter().map(|u| u.into()).collect())
}
```

**Testing Checklist**:
- [ ] Benchmark parsing speed vs serde_json
- [ ] Verify correctness with 1000+ random samples
- [ ] Test with malformed JSON (should not panic)
- [ ] Monitor memory usage

---

## 4.4 Memory-Mapped Price Cache
**Impact**: Sub-microsecond price lookups
**Location**: `crates/core/src/cache/`

### Implementation Steps:

1. **Create Shared Memory Cache**
```rust
// File: crates/core/src/cache/mmap_cache.rs

use memmap2::MmapMut;
use std::sync::Arc;

const CACHE_SIZE: usize = 100 * 1024 * 1024; // 100MB

pub struct MmapPriceCache {
    mmap: Arc<MmapMut>,
    index: HashMap<TradingPair, usize>, // Offset in mmap
}

impl MmapPriceCache {
    pub fn new() -> Self {
        let mmap = MmapMut::map_anon(CACHE_SIZE).unwrap();
        Self {
            mmap: Arc::new(mmap),
            index: HashMap::new(),
        }
    }
    
    pub fn write_price(&mut self, pair: &TradingPair, price: &Price) {
        let offset = self.get_or_allocate_offset(pair);
        let bytes = bincode::serialize(price).unwrap();
        
        // Write directly to memory-mapped region
        let slice = &mut self.mmap[offset..offset + bytes.len()];
        slice.copy_from_slice(&bytes);
    }
    
    pub fn read_price(&self, pair: &TradingPair) -> Option<Price> {
        let offset = *self.index.get(pair)?;
        let bytes = &self.mmap[offset..offset + std::mem::size_of::<Price>()];
        bincode::deserialize(bytes).ok()
    }
}
```

2. **Use Lock-Free Reads**
```rust
// File: crates/core/src/cache/lockfree_cache.rs

use crossbeam::epoch::{self, Atomic, Owned};

pub struct LockFreePriceCache {
    prices: Arc<dashmap::DashMap<TradingPair, Price>>, // Concurrent HashMap
}

impl LockFreePriceCache {
    pub fn get(&self, pair: &TradingPair) -> Option<Price> {
        self.prices.get(pair).map(|r| *r.value())
    }
    
    pub fn insert(&self, pair: TradingPair, price: Price) {
        self.prices.insert(pair, price);
    }
}
```

**Testing Checklist**:
- [ ] Benchmark read/write latency (should be <100ns)
- [ ] Test with high concurrency (1000+ threads)
- [ ] Verify data integrity under load
- [ ] Monitor memory consumption

---

## 4.5 SIMD Profit Calculations
**Impact**: 4x faster profit calculations for large path sets
**Location**: `crates/core/src/pathfinding/profit.rs`

### Implementation Steps:

1. **Add SIMD Dependencies**
```toml
# File: Cargo.toml

[dependencies]
packed_simd_2 = "0.3"
```

2. **Vectorized Profit Calculator**
```rust
// File: crates/core/src/pathfinding/simd_profit.rs

use packed_simd_2::f64x8;

pub struct SimdProfitCalculator;

impl SimdProfitCalculator {
    pub fn calculate_batch_profits(paths: &[TradePath]) -> Vec<f64> {
        let mut profits = vec![0.0; paths.len()];
        
        // Process 8 paths at a time using SIMD
        for (i, chunk) in paths.chunks(8).enumerate() {
            let mut buy_prices = [0.0; 8];
            let mut sell_prices = [0.0; 8];
            let mut fees = [0.0; 8];
            
            for (j, path) in chunk.iter().enumerate() {
                buy_prices[j] = path.buy_price;
                sell_prices[j] = path.sell_price;
                fees[j] = path.total_fees;
            }
            
            let buy_vec = f64x8::from_slice_unaligned(&buy_prices);
            let sell_vec = f64x8::from_slice_unaligned(&sell_prices);
            let fee_vec = f64x8::from_slice_unaligned(&fees);
            
            // Vectorized calculation: profit = (sell - buy - fees) / buy * 10000
            let profit_vec = ((sell_vec - buy_vec - fee_vec) / buy_vec) * f64x8::splat(10000.0);
            
            let mut result = [0.0; 8];
            profit_vec.write_to_slice_unaligned(&mut result);
            
            for (j, &profit) in result.iter().enumerate().take(chunk.len()) {
                profits[i * 8 + j] = profit;
            }
        }
        
        profits
    }
}
```

**Testing Checklist**:
- [ ] Benchmark vs scalar implementation
- [ ] Verify numerical accuracy (should match within 0.01%)
- [ ] Test with edge cases (zero prices, huge fees)
- [ ] Profile SIMD utilization

---

## Phase 4 Deliverables

### Expected Performance Improvements:
- Price fetch latency: **500ms → 80ms** (6x improvement)
- Opportunity detection: **50ms → 15ms** (3x improvement)
- Total loop time: **600ms → 100ms** (6x improvement)

### Configuration Updates:
```toml
# File: .env

# Phase 4 Performance Settings
ENABLE_PARALLEL_FETCHING=true
ENABLE_WEBSOCKET_STREAMING=true
USE_SIMD_CALCULATIONS=true
PRICE_CACHE_TYPE=lockfree  # Options: lockfree, mmap, standard
MAX_CONCURRENT_REQUESTS=20
POLL_INTERVAL_MS=100
```

### Monitoring & Validation:
```rust
// Add performance metrics
pub struct PerformanceMetrics {
    pub avg_fetch_latency: Duration,
    pub p99_fetch_latency: Duration,
    pub prices_per_second: u64,
    pub cache_hit_rate: f64,
}
```

**Success Criteria**:
- [ ] Latency reduced to <100ms total loop time
- [ ] Zero data loss during parallel operations
- [ ] CPU usage remains <50% on 4-core machine
- [ ] Memory usage <2GB
- [ ] All tests passing

---

# PHASE 5: Advanced Risk & Safety Management 🛡️
**Goal**: Prevent catastrophic losses and ensure system stability

## 5.1 Multi-Tier Circuit Breakers
**Impact**: Prevent runaway losses
**Location**: `crates/core/src/risk/`

### Implementation Steps:

1. **Create Circuit Breaker System**
```rust
// File: crates/core/src/risk/circuit_breaker.rs

use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum CircuitState {
    Closed,      // Normal operation
    HalfOpen,    // Testing if system recovered
    Open,        // Trading disabled
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: usize,
    success_threshold: usize,
    timeout: Duration,
    
    // Counters
    consecutive_failures: Arc<RwLock<usize>>,
    consecutive_successes: Arc<RwLock<usize>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    pub async fn record_success(&self) {
        let mut successes = self.consecutive_successes.write().await;
        *successes += 1;
        
        let mut failures = self.consecutive_failures.write().await;
        *failures = 0;
        
        // Transition from HalfOpen to Closed if enough successes
        if *successes >= self.success_threshold {
            let mut state = self.state.write().await;
            if matches!(*state, CircuitState::HalfOpen) {
                *state = CircuitState::Closed;
                tracing::info!("Circuit breaker CLOSED - system recovered");
            }
        }
    }
    
    pub async fn record_failure(&self) {
        let mut failures = self.consecutive_failures.write().await;
        *failures += 1;
        
        let mut successes = self.consecutive_successes.write().await;
        *successes = 0;
        
        *self.last_failure_time.write().await = Some(Instant::now());
        
        // Open circuit if threshold exceeded
        if *failures >= self.failure_threshold {
            let mut state = self.state.write().await;
            *state = CircuitState::Open;
            tracing::error!(
                failures = *failures,
                "Circuit breaker OPEN - trading halted"
            );
        }
    }
    
    pub async fn can_execute(&self) -> bool {
        let mut state = self.state.write().await;
        
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout elapsed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.timeout {
                        *state = CircuitState::HalfOpen;
                        tracing::warn!("Circuit breaker HALF-OPEN - testing recovery");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,  // Allow test trades
        }
    }
}
```

2. **Implement Multi-Level Breakers**
```rust
// File: crates/core/src/risk/multi_level_breaker.rs

pub struct MultiLevelBreaker {
    // Different thresholds for different severity
    trade_level: CircuitBreaker,     // Individual trade failures
    session_level: CircuitBreaker,   // Session-level P&L
    daily_level: CircuitBreaker,     // Daily loss limits
}

impl MultiLevelBreaker {
    pub async fn check_can_trade(&self) -> Result<(), String> {
        if !self.trade_level.can_execute().await {
            return Err("Trade circuit breaker open".to_string());
        }
        
        if !self.session_level.can_execute().await {
            return Err("Session circuit breaker open".to_string());
        }
        
        if !self.daily_level.can_execute().await {
            return Err("Daily circuit breaker open - max losses reached".to_string());
        }
        
        Ok(())
    }
}
```

3. **Integration with Executor**
```rust
// File: crates/bot/src/executor.rs

pub async fn execute_trade(&mut self, opportunity: &Arbitrage) -> Result<TradeResult> {
    // Check circuit breakers first
    self.circuit_breaker.check_can_trade().await?;
    
    // Execute trade
    let result = self.submit_transaction(opportunity).await;
    
    // Update circuit breaker state
    match &result {
        Ok(_) => self.circuit_breaker.record_success().await,
        Err(_) => self.circuit_breaker.record_failure().await,
    }
    
    result
}
```

**Configuration**:
```toml
# File: .env

[risk.circuit_breaker]
TRADE_FAILURE_THRESHOLD=5          # Stop after 5 consecutive failures
TRADE_RECOVERY_THRESHOLD=3         # Resume after 3 successes
TRADE_TIMEOUT_SECONDS=300          # Wait 5 min before testing

SESSION_LOSS_THRESHOLD=-100.0      # Stop if session loss > $100
SESSION_TIMEOUT_SECONDS=1800       # Wait 30 min before resuming

DAILY_LOSS_THRESHOLD=-500.0        # Stop if daily loss > $500
DAILY_RESET_HOUR=0                 # Reset at midnight UTC
```

---

## 5.2 Value at Risk (VaR) Calculator
**Impact**: Quantify maximum expected loss
**Location**: `crates/core/src/risk/var.rs`

### Implementation Steps:

1. **Historical VaR Calculator**
```rust
// File: crates/core/src/risk/var.rs

pub struct VaRCalculator {
    historical_returns: VecDeque<f64>,
    confidence_level: f64,  // 0.95 or 0.99
    window_size: usize,
}

impl VaRCalculator {
    pub fn calculate_var(&self) -> f64 {
        let mut sorted_returns: Vec<f64> = self.historical_returns
            .iter()
            .copied()
            .collect();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        // Get percentile based on confidence level
        let index = ((1.0 - self.confidence_level) * sorted_returns.len() as f64) as usize;
        sorted_returns.get(index).copied().unwrap_or(0.0).abs()
    }
    
    pub fn calculate_conditional_var(&self) -> f64 {
        // CVaR = average of losses beyond VaR threshold
        let var = self.calculate_var();
        let losses: Vec<f64> = self.historical_returns
            .iter()
            .filter(|&&r| r < -var)
            .copied()
            .collect();
        
        if losses.is_empty() {
            var
        } else {
            losses.iter().sum::<f64>() / losses.len() as f64
        }
    }
    
    pub fn record_return(&mut self, profit: f64, capital: f64) {
        let return_pct = profit / capital;
        self.historical_returns.push_back(return_pct);
        
        if self.historical_returns.len() > self.window_size {
            self.historical_returns.pop_front();
        }
    }
}
```

2. **Position Sizer Using VaR**
```rust
// File: crates/core/src/risk/position_sizing.rs

pub struct VaRPositionSizer {
    var_calculator: VaRCalculator,
    max_var_pct: f64,  // Max 2% of capital at risk
    total_capital: f64,
}

impl VaRPositionSizer {
    pub fn calculate_max_position(&self, expected_profit_bps: f64) -> f64 {
        let var_95 = self.var_calculator.calculate_var();
        let max_loss_allowed = self.total_capital * self.max_var_pct;
        
        // Position size = max_loss / expected_loss_rate
        let position_size = max_loss_allowed / var_95.abs();
        
        // Cap at available capital
        position_size.min(self.total_capital * 0.5)  // Never use >50%
    }
}
```

---

## 5.3 Volatility-Adjusted Position Sizing
**Impact**: Reduce position size during high volatility
**Location**: `crates/core/src/risk/volatility.rs`

### Implementation Steps:

1. **Volatility Tracker**
```rust
// File: crates/core/src/risk/volatility.rs

pub struct VolatilityTracker {
    price_history: VecDeque<PriceSnapshot>,
    window_size: usize,
}

impl VolatilityTracker {
    pub fn calculate_realized_volatility(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }
        
        let returns: Vec<f64> = self.price_history
            .iter()
            .zip(self.price_history.iter().skip(1))
            .map(|(prev, curr)| {
                (curr.price - prev.price) / prev.price
            })
            .collect();
        
        // Standard deviation of returns
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
    
    pub fn get_volatility_regime(&self) -> VolatilityRegime {
        let vol = self.calculate_realized_volatility();
        
        match vol {
            v if v < 0.01 => VolatilityRegime::Low,
            v if v < 0.03 => VolatilityRegime::Medium,
            _ => VolatilityRegime::High,
        }
    }
}

pub enum VolatilityRegime {
    Low,     // Normal trading
    Medium,  // Reduce size 50%
    High,    // Reduce size 75% or pause
}
```

2. **Volatility-Adjusted Executor**
```rust
// File: crates/bot/src/executor.rs

pub struct VolatilityAdjustedExecutor {
    base_position_size: f64,
    volatility_tracker: VolatilityTracker,
}

impl VolatilityAdjustedExecutor {
    pub fn calculate_position_size(&self, opportunity: &Arbitrage) -> f64 {
        let base_size = self.base_position_size;
        
        match self.volatility_tracker.get_volatility_regime() {
            VolatilityRegime::Low => base_size,
            VolatilityRegime::Medium => base_size * 0.5,
            VolatilityRegime::High => base_size * 0.25,
        }
    }
}
```

---

## 5.4 Correlation Matrix & Position Limits
**Impact**: Avoid overexposure to correlated assets
**Location**: `crates/core/src/risk/correlation.rs`

### Implementation Steps:

1. **Correlation Calculator**
```rust
// File: crates/core/src/risk/correlation.rs

pub struct CorrelationMatrix {
    returns: HashMap<String, VecDeque<f64>>,
    correlations: HashMap<(String, String), f64>,
}

impl CorrelationMatrix {
    pub fn calculate_correlation(&self, asset1: &str, asset2: &str) -> f64 {
        let returns1 = self.returns.get(asset1).unwrap();
        let returns2 = self.returns.get(asset2).unwrap();
        
        let n = returns1.len().min(returns2.len());
        if n < 10 {
            return 0.0;  // Not enough data
        }
        
        let mean1 = returns1.iter().sum::<f64>() / n as f64;
        let mean2 = returns2.iter().sum::<f64>() / n as f64;
        
        let mut covariance = 0.0;
        let mut var1 = 0.0;
        let mut var2 = 0.0;
        
        for i in 0..n {
            let diff1 = returns1[i] - mean1;
            let diff2 = returns2[i] - mean2;
            covariance += diff1 * diff2;
            var1 += diff1 * diff1;
            var2 += diff2 * diff2;
        }
        
        if var1 == 0.0 || var2 == 0.0 {
            return 0.0;
        }
        
        covariance / (var1.sqrt() * var2.sqrt())
    }
    
    pub fn get_correlated_positions(&self, asset: &str, threshold: f64) -> Vec<String> {
        self.correlations
            .iter()
            .filter(|((a1, a2), &corr)| {
                (a1 == asset || a2 == asset) && corr.abs() > threshold
            })
            .map(|((a1, a2), _)| {
                if a1 == asset { a2.clone() } else { a1.clone() }
            })
            .collect()
    }
}
```

2. **Position Limit Enforcer**
```rust
// File: crates/core/src/risk/position_limits.rs

pub struct PositionLimitEnforcer {
    current_positions: HashMap<String, f64>,
    correlation_matrix: CorrelationMatrix,
    max_correlated_exposure: f64,  // Max 20% in correlated assets
}

impl PositionLimitEnforcer {
    pub fn can_add_position(&self, asset: &str, size: f64) -> Result<(), String> {
        // Check total exposure to correlated assets
        let correlated = self.correlation_matrix.get_correlated_positions(asset, 0.7);
        
        let total_correlated_exposure: f64 = correlated
            .iter()
            .filter_map(|a| self.current_positions.get(a))
            .sum();
        
        if total_correlated_exposure + size > self.max_correlated_exposure {
            return Err(format!(
                "Would exceed correlated exposure limit: {} + {} > {}",
                total_correlated_exposure, size, self.max_correlated_exposure
            ));
        }
        
        Ok(())
    }
}
```

---

## 5.5 Emergency Stop & Safety Mechanisms
**Impact**: Quick manual intervention capability
**Location**: `crates/bot/src/safety/`

### Implementation Steps:

1. **Emergency Stop Handler**
```rust
// File: crates/bot/src/safety/emergency_stop.rs

use tokio::sync::watch;

pub struct EmergencyStop {
    stop_signal: watch::Receiver<bool>,
    reason: Arc<RwLock<Option<String>>>,
}

impl EmergencyStop {
    pub fn new() -> (Self, EmergencyStopTrigger) {
        let (tx, rx) = watch::channel(false);
        let reason = Arc::new(RwLock::new(None));
        
        (
            Self {
                stop_signal: rx,
                reason: reason.clone(),
            },
            EmergencyStopTrigger {
                trigger: tx,
                reason,
            }
        )
    }
    
    pub fn is_stopped(&self) -> bool {
        *self.stop_signal.borrow()
    }
    
    pub async fn get_reason(&self) -> Option<String> {
        self.reason.read().await.clone()
    }
}

pub struct EmergencyStopTrigger {
    trigger: watch::Sender<bool>,
    reason: Arc<RwLock<Option<String>>>,
}

impl EmergencyStopTrigger {
    pub async fn trigger(&self, reason: String) {
        *self.reason.write().await = Some(reason.clone());
        let _ = self.trigger.send(true);
        tracing::error!("🚨 EMERGENCY STOP TRIGGERED: {}", reason);
    }
}
```

2. **HTTP API for Emergency Stop**
```rust
// File: crates/bot/src/api/emergency.rs

use axum::{Router, Json};

pub fn emergency_routes(stop_trigger: Arc<EmergencyStopTrigger>) -> Router {
    Router::new()
        .route("/emergency/stop", post(emergency_stop))
        .layer(Extension(stop_trigger))
}

async fn emergency_stop(
    Json(payload): Json<EmergencyStopRequest>,
    Extension(trigger): Extension<Arc<EmergencyStopTrigger>>,
) -> Json<ApiResponse> {
    trigger.trigger(payload.reason).await;
    
    Json(ApiResponse {
        success: true,
        message: "Emergency stop activated".to_string(),
    })
}
```

3. **Webhook-based Alerts**
```rust
// File: crates/core/src/alerts/webhook.rs

pub struct WebhookAlerter {
    telegram_webhook: Option<String>,
    discord_webhook: Option<String>,
}

impl WebhookAlerter {
    pub async fn send_critical_alert(&self, message: &str) {
        // Send to Telegram
        if let Some(url) = &self.telegram_webhook {
            let payload = json!({
                "text": format!("🚨 CRITICAL: {}", message),
                "parse_mode": "HTML"
            });
            let _ = reqwest::Client::new()
                .post(url)
                .json(&payload)
                .send()
                .await;
        }
        
        // Send to Discord
        if let Some(url) = &self.discord_webhook {
            let payload = json!({
                "content": format!("@everyone 🚨 **CRITICAL**: {}", message),
                "username": "ArbEngine Alert"
            });
            let _ = reqwest::Client::new()
                .post(url)
                .json(&payload)
                .send()
                .await;
        }
    }
}
```

---

## Phase 5 Deliverables

### Risk Management Components:
- ✅ Multi-tier circuit breakers (trade/session/daily)
- ✅ Value at Risk (VaR) calculation
- ✅ Volatility-adjusted position sizing
- ✅ Correlation-based position limits
- ✅ Emergency stop mechanism
- ✅ Real-time alerting system

### Configuration:
```toml
# File: .env

[risk]
ENABLE_CIRCUIT_BREAKERS=true
ENABLE_VAR_LIMITS=true
ENABLE_CORRELATION_CHECKS=true

VAR_CONFIDENCE_LEVEL=0.95
MAX_VAR_PERCENT=2.0
MAX_CORRELATED_EXPOSURE=0.20

TELEGRAM_WEBHOOK_URL=https://api.telegram.org/bot...
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...

EMERGENCY_STOP_API_PORT=8080
EMERGENCY_STOP_AUTH_TOKEN=your-secret-token-here
```

### Testing Checklist:
- [ ] Simulate 10 consecutive trade failures → circuit opens
- [ ] Test VaR calculation with historical data
- [ ] Verify position sizing scales with volatility
- [ ] Test emergency stop via HTTP API
- [ ] Confirm alerts sent to Telegram/Discord
- [ ] Load test with 100+ concurrent opportunities

**Success Criteria**:
- [ ] No single trade can lose >2% of capital
- [ ] Circuit breaker activates within 1 second of threshold
- [ ] Emergency stop halts all trading within 100ms
- [ ] Alerts delivered <5 seconds after trigger
- [ ] All safety mechanisms tested and documented

---

# PHASE 6: Data Infrastructure & Analytics 📊
**Goal**: Build comprehensive logging and performance tracking

## 6.1 TimescaleDB Integration
**Impact**: Fast time-series queries for analysis
**Location**: `crates/core/src/database/`

### Implementation Steps:

1. **Add Dependencies**
```toml
# File: Cargo.toml

[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "postgres", "chrono"] }
tokio-postgres = "0.7"
```

2. **Database Schema**
```sql
-- File: migrations/001_create_timescale_schema.sql

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Price ticks table
CREATE TABLE price_ticks (
    time TIMESTAMPTZ NOT NULL,
    pair VARCHAR(20) NOT NULL,
    source VARCHAR(20) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    volume DOUBLE PRECISION,
    liquidity BIGINT
);

SELECT create_hypertable('price_ticks', 'time');
CREATE INDEX idx_price_pair_time ON price_ticks (pair, time DESC);

-- Opportunities table
CREATE TABLE opportunities (
    time TIMESTAMPTZ NOT NULL,
    opportunity_id UUID PRIMARY KEY,
    path TEXT NOT NULL,
    expected_profit_bps DOUBLE PRECISION NOT NULL,
    input_amount DOUBLE PRECISION NOT NULL,
    dex_route TEXT NOT NULL,
    status VARCHAR(20) NOT NULL  -- detected, executed, failed, expired
);

SELECT create_hypertable('opportunities', 'time');
CREATE INDEX idx_opp_status ON opportunities (status, time DESC);

-- Trades table
CREATE TABLE trades (
    time TIMESTAMPTZ NOT NULL,
    trade_id UUID PRIMARY KEY,
    opportunity_id UUID REFERENCES opportunities(opportunity_id),
    signature VARCHAR(100) NOT NULL,
    actual_profit DOUBLE PRECISION,
    execution_time_ms INTEGER,
    slippage_bps DOUBLE PRECISION,
    gas_used BIGINT,
    priority_fee BIGINT,
    status VARCHAR(20) NOT NULL  -- success, failed, timeout
);

SELECT create_hypertable('trades', 'time');
CREATE INDEX idx_trades_signature ON trades (signature);

-- Performance metrics table
CREATE TABLE performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_name VARCHAR(50) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    tags JSONB
);

SELECT create_hypertable('performance_metrics', 'time');
CREATE INDEX idx_metrics_name ON performance_metrics (metric_name, time DESC);

-- Continuous aggregates for fast queries
CREATE MATERIALIZED VIEW hourly_profits
WITH (timescaledb.continuous) AS
SELECT time_bucket('1 hour', time) AS bucket,
       COUNT(*) as trade_count,
       SUM(actual_profit) as total_profit,
       AVG(actual_profit) as avg_profit,
       MAX(actual_profit) as max_profit,
       MIN(actual_profit) as min_profit,
       AVG(slippage_bps) as avg_slippage
FROM trades
WHERE status = 'success'
GROUP BY bucket;

SELECT add_continuous_aggregate_policy('hourly_profits',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

3. **Database Client**
```rust
// File: crates/core/src/database/timescale_client.rs

use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

pub struct TimescaleClient {
    pool: PgPool,
}

impl TimescaleClient {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        
        Ok(Self { pool })
    }
    
    pub async fn insert_price_tick(&self, tick: &PriceTick) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO price_ticks (time, pair, source, price, volume, liquidity)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            tick.timestamp,
            tick.pair.to_string(),
            tick.source,
            tick.price,
            tick.volume,
            tick.liquidity as i64
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn insert_opportunity(&self, opp: &Arbitrage) -> Result<Uuid> {
        let opp_id = Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO opportunities 
            (time, opportunity_id, path, expected_profit_bps, input_amount, dex_route, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            chrono::Utc::now(),
            opp_id,
            opp.path.to_string(),
            opp.profit_bps,
            opp.input_amount,
            opp.dex_route.to_string(),
            "detected"
        )
        .execute(&self.pool)
        .await?;
        
        Ok(opp_id)
    }
    
    pub async fn insert_trade(&self, trade: &TradeResult) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO trades 
            (time, trade_id, opportunity_id, signature, actual_profit, 
             execution_time_ms, slippage_bps, gas_used, priority_fee, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            chrono::Utc::now(),
            trade.trade_id,
            trade.opportunity_id,
            trade.signature.to_string(),
            trade.actual_profit,
            trade.execution_time.as_millis() as i32,
            trade.slippage_bps,
            trade.gas_used as i64,
            trade.priority_fee as i64,
            trade.status.to_string()
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_hourly_stats(&self, hours: i32) -> Result<Vec<HourlyStats>> {
        let stats = sqlx::query_as!(
            HourlyStats,
            r#"
            SELECT bucket, trade_count, total_profit, avg_profit, avg_slippage
            FROM hourly_profits
            WHERE bucket > NOW() - INTERVAL '1 hour' * $1
            ORDER BY bucket DESC
            "#,
            hours
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(stats)
    }
}
```

4. **Background Logger**
```rust
// File: crates/bot/src/logging/background_logger.rs

pub struct BackgroundLogger {
    db: TimescaleClient,
    price_queue: mpsc::Receiver<PriceTick>,
    trade_queue: mpsc::Receiver<TradeResult>,
}

impl BackgroundLogger {
    pub async fn start(mut self) {
        tokio::spawn(async move {
            let mut batch_prices = Vec::with_capacity(1000);
            let mut flush_interval = tokio::time::interval(Duration::from_secs(1));
            
            loop {
                tokio::select! {
                    Some(tick) = self.price_queue.recv() => {
                        batch_prices.push(tick);
                        
                        // Flush when batch is full
                        if batch_prices.len() >= 1000 {
                            self.flush_prices(&batch_prices).await;
                            batch_prices.clear();
                        }
                    }
                    
                    Some(trade) = self.trade_queue.recv() => {
                        // Trades are critical - write immediately
                        if let Err(e) = self.db.insert_trade(&trade).await {
                            tracing::error!("Failed to log trade: {}", e);
                        }
                    }
                    
                    _ = flush_interval.tick() => {
                        if !batch_prices.is_empty() {
                            self.flush_prices(&batch_prices).await;
                            batch_prices.clear();
                        }
                    }
                }
            }
        });
    }
    
    async fn flush_prices(&self, prices: &[PriceTick]) {
        // Bulk insert for performance
        for chunk in prices.chunks(100) {
            if let Err(e) = self.db.insert_price_batch(chunk).await {
                tracing::error!("Failed to flush price batch: {}", e);
            }
        }
    }
}
```

---

## 6.2 Prometheus Metrics Exporter
**Impact**: Real-time monitoring with Grafana
**Location**: `crates/bot/src/metrics/`

### Implementation Steps:

1. **Add Dependencies**
```toml
# File: Cargo.toml

[dependencies]
prometheus = "0.13"
axum = { version = "0.7", features = ["macros"] }
```

2. **Metrics Collector**
```rust
// File: crates/bot/src/metrics/prometheus.rs

use prometheus::{
    Registry, Counter, Histogram, Gauge, HistogramOpts, Opts,
    IntCounter, IntGauge,
};
use std::sync::Arc;

pub struct MetricsCollector {
    registry: Registry,
    
    // Counters
    pub opportunities_detected: IntCounter,
    pub trades_attempted: IntCounter,
    pub trades_successful: IntCounter,
    pub trades_failed: IntCounter,
    
    // Gauges
    pub current_balance: Gauge,
    pub active_positions: IntGauge,
    pub circuit_breaker_state: IntGauge,  // 0=closed, 1=half-open, 2=open
    
    // Histograms
    pub opportunity_profit: Histogram,
    pub trade_execution_time: Histogram,
    pub price_fetch_latency: Histogram,
    pub slippage_distribution: Histogram,
}

impl MetricsCollector {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        
        // Initialize counters
        let opportunities_detected = IntCounter::new(
            "arb_opportunities_detected_total",
            "Total number of arbitrage opportunities detected"
        )?;
        registry.register(Box::new(opportunities_detected.clone()))?;
        
        let trades_attempted = IntCounter::new(
            "arb_trades_attempted_total",
            "Total number of trades attempted"
        )?;
        registry.register(Box::new(trades_attempted.clone()))?;
        
        let trades_successful = IntCounter::new(
            "arb_trades_successful_total",
            "Total number of successful trades"
        )?;
        registry.register(Box::new(trades_successful.clone()))?;
        
        let trades_failed = IntCounter::new(
            "arb_trades_failed_total",
            "Total number of failed trades"
        )?;
        registry.register(Box::new(trades_failed.clone()))?;
        
        // Initialize gauges
        let current_balance = Gauge::new(
            "arb_current_balance_usd",
            "Current account balance in USD"
        )?;
        registry.register(Box::new(current_balance.clone()))?;
        
        let active_positions = IntGauge::new(
            "arb_active_positions",
            "Number of currently active positions"
        )?;
        registry.register(Box::new(active_positions.clone()))?;
        
        let circuit_breaker_state = IntGauge::new(
            "arb_circuit_breaker_state",
            "Circuit breaker state (0=closed, 1=half-open, 2=open)"
        )?;
        registry.register(Box::new(circuit_breaker_state.clone()))?;
        
        // Initialize histograms
        let opportunity_profit = Histogram::with_opts(
            HistogramOpts::new(
                "arb_opportunity_profit_bps",
                "Distribution of opportunity profit in basis points"
            )
            .buckets(vec![10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0])
        )?;
        registry.register(Box::new(opportunity_profit.clone()))?;
        
        let trade_execution_time = Histogram::with_opts(
            HistogramOpts::new(
                "arb_trade_execution_seconds",
                "Trade execution time in seconds"
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0])
        )?;
        registry.register(Box::new(trade_execution_time.clone()))?;
        
        let price_fetch_latency = Histogram::with_opts(
            HistogramOpts::new(
                "arb_price_fetch_seconds",
                "Price fetching latency in seconds"
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0])
        )?;
        registry.register(Box::new(price_fetch_latency.clone()))?;
        
        let slippage_distribution = Histogram::with_opts(
            HistogramOpts::new(
                "arb_slippage_bps",
                "Slippage distribution in basis points"
            )
            .buckets(vec![5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0])
        )?;
        registry.register(Box::new(slippage_distribution.clone()))?;
        
        Ok(Self {
            registry,
            opportunities_detected,
            trades_attempted,
            trades_successful,
            trades_failed,
            current_balance,
            active_positions,
            circuit_breaker_state,
            opportunity_profit,
            trade_execution_time,
            price_fetch_latency,
            slippage_distribution,
        })
    }
    
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}
```

3. **HTTP Metrics Endpoint**
```rust
// File: crates/bot/src/api/metrics.rs

use axum::{Router, response::IntoResponse};
use prometheus::{Encoder, TextEncoder};

pub fn metrics_routes(metrics: Arc<MetricsCollector>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .layer(Extension(metrics))
}

async fn metrics_handler(
    Extension(metrics): Extension<Arc<MetricsCollector>>,
) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry().gather();
    let mut buffer = Vec::new();
    
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        buffer
    )
}
```

4. **Integration with Bot**
```rust
// File: crates/bot/src/main.rs

#[tokio::main]
async fn main() -> Result<()> {
    let metrics = Arc::new(MetricsCollector::new()?);
    
    // Start metrics server
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        let app = metrics_routes(metrics_clone);
        axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    
    // Main trading loop
    loop {
        let start = Instant::now();
        
        // Fetch prices
        let prices = fetcher.fetch_all_prices().await;
        metrics.price_fetch_latency.observe(start.elapsed().as_secs_f64());
        
        // Find opportunities
        let opportunities = pathfinder.find_opportunities(&prices);
        metrics.opportunities_detected.inc_by(opportunities.len() as u64);
        
        for opp in opportunities {
            metrics.opportunity_profit.observe(opp.profit_bps as f64);
            
            // Execute trade
            metrics.trades_attempted.inc();
            let result = executor.execute_trade(&opp).await;
            
            match result {
                Ok(trade) => {
                    metrics.trades_successful.inc();
                    metrics.trade_execution_time.observe(
                        trade.execution_time.as_secs_f64()
                    );
                    metrics.slippage_distribution.observe(trade.slippage_bps as f64);
                }
                Err(_) => {
                    metrics.trades_failed.inc();
                }
            }
        }
        
        // Update balance gauge
        let balance = get_current_balance().await;
        metrics.current_balance.set(balance);
    }
}
```

---

## 6.3 Structured Logging with Tracing
**Impact**: Better debugging and audit trails
**Location**: `crates/core/src/logging/`

### Implementation Steps:

1. **Enhanced Logging Setup**
```rust
// File: crates/bot/src/logging/setup.rs

use tracing_subscriber::{
    Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    filter::EnvFilter,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn setup_logging() {
    // Console output (for development)
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_ansi(true);
    
    // File output (for production)
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "./logs",
        "arbengine.log"
    );
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .json();  // JSON format for easier parsing
    
    // Error-only file (for critical issues)
    let error_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "./logs",
        "errors.log"
    );
    let error_layer = fmt::layer()
        .with_writer(error_appender)
        .with_filter(tracing::Level::ERROR);
    
    // Combine layers
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(console_layer)
        .with(file_layer)
        .with(error_layer)
        .init();
}
```

2. **Structured Log Events**
```rust
// File: crates/bot/src/executor.rs

#[tracing::instrument(
    skip(self, opportunity),
    fields(
        opp_id = %opportunity.id,
        path = %opportunity.path,
        expected_profit_bps = opportunity.profit_bps,
    )
)]
pub async fn execute_trade(&self, opportunity: &Arbitrage) -> Result<TradeResult> {
    tracing::info!("Starting trade execution");
    
    let start = Instant::now();
    
    // Build transaction
    let tx = self.build_transaction(opportunity).await?;
    tracing::debug!(
        tx_size = tx.serialized_size(),
        compute_units = tx.compute_units,
        "Transaction built"
    );
    
    // Submit to blockchain
    let signature = self.submit_transaction(&tx).await?;
    tracing::info!(
        signature = %signature,
        elapsed_ms = start.elapsed().as_millis(),
        "Transaction submitted"
    );
    
    // Wait for confirmation
    let result = self.wait_for_confirmation(&signature).await?;
    
    tracing::info!(
        signature = %signature,
        actual_profit = result.actual_profit,
        slippage_bps = result.slippage_bps,
        total_time_ms = start.elapsed().as_millis(),
        "Trade completed successfully"
    );
    
    Ok(result)
}
```

---

## 6.4 Grafana Dashboard Setup
**Impact**: Visual monitoring and alerting
**Location**: `dashboard/grafana/`

### Implementation Steps:

1. **Docker Compose for Grafana + Prometheus**
```yaml
# File: docker-compose.monitoring.yml

version: '3.8'

services:
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: arbengine
    volumes:
      - timescale_data:/var/lib/postgresql/data
      - ./migrations:/docker-entrypoint-initdb.d
    ports:
      - "5432:5432"
  
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./dashboard/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=30d'
    ports:
      - "9091:9090"
  
  grafana:
    image: grafana/grafana:latest
    environment:
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_PASSWORD}
      GF_INSTALL_PLUGINS: grafana-clock-panel,grafana-simple-json-datasource
    volumes:
      - ./dashboard/grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./dashboard/grafana/datasources:/etc/grafana/provisioning/datasources
      - grafana_data:/var/lib/grafana
    ports:
      - "3001:3000"
    depends_on:
      - prometheus
      - timescaledb

volumes:
  timescale_data:
  prometheus_data:
  grafana_data:
```

2. **Prometheus Configuration**
```yaml
# File: dashboard/prometheus/prometheus.yml

global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'arbengine-bot'
    static_configs:
      - targets: ['host.docker.internal:9090']  # Bot metrics endpoint
    
  - job_name: 'node-exporter'
    static_configs:
      - targets: ['node-exporter:9100']  # System metrics
```

3. **Grafana Dashboard JSON**
```json
{
  "dashboard": {
    "title": "ArbEngine Pro - Live Trading",
    "panels": [
      {
        "title": "Success Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(arb_trades_successful_total[5m]) / rate(arb_trades_attempted_total[5m]) * 100"
          }
        ]
      },
      {
        "title": "Profit Over Time",
        "type": "graph",
        "targets": [
          {
            "expr": "sum(rate(arb_actual_profit_total[1m]))"
          }
        ]
      },
      {
        "title": "Execution Latency (p95)",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(arb_trade_execution_seconds_bucket[5m]))"
          }
        ]
      },
      {
        "title": "Circuit Breaker Status",
        "type": "stat",
        "targets": [
          {
            "expr": "arb_circuit_breaker_state"
          }
        ],
        "thresholds": [
          { "value": 0, "color": "green" },
          { "value": 1, "color": "yellow" },
          { "value": 2, "color": "red" }
        ]
      }
    ]
  }
}
```

---

## Phase 6 Deliverables

### Data Infrastructure:
- ✅ TimescaleDB for time-series data
- ✅ Prometheus metrics exporter
- ✅ Structured logging with tracing
- ✅ Grafana dashboards
- ✅ Background logging service

### Configuration:
```toml
# File: .env

[database]
DATABASE_URL=postgresql://postgres:password@localhost:5432/arbengine
ENABLE_TIMESCALE_LOGGING=true
BATCH_SIZE=1000
FLUSH_INTERVAL_SECONDS=1

[metrics]
PROMETHEUS_PORT=9090
ENABLE_METRICS=true

[logging]
LOG_LEVEL=info
LOG_FORMAT=json  # Options: json, pretty
LOG_TO_FILE=true
LOG_DIRECTORY=./logs
```

### Queries for Analysis:
```sql
-- Top 10 most profitable pairs (last 24h)
SELECT pair, COUNT(*) as trades, SUM(actual_profit) as total_profit
FROM trades
WHERE time > NOW() - INTERVAL '24 hours' AND status = 'success'
GROUP BY pair
ORDER BY total_profit DESC
LIMIT 10;

-- Average slippage by DEX
SELECT dex_route, AVG(slippage_bps) as avg_slippage
FROM trades
WHERE time > NOW() - INTERVAL '7 days'
GROUP BY dex_route;

-- Win rate over time
SELECT time_bucket('1 hour', time) as hour,
       COUNT(*) FILTER (WHERE status = 'success') * 100.0 / COUNT(*) as win_rate
FROM trades
GROUP BY hour
ORDER BY hour DESC;
```

### Success Criteria:
- [ ] All trades logged to TimescaleDB within 1 second
- [ ] Prometheus metrics updated every 15 seconds
- [ ] Grafana dashboards showing live data
- [ ] Query response time <100ms for hourly aggregates
- [ ] Log rotation working (daily rotation)
- [ ] Retention policy: 30 days raw data, 1 year aggregates

---

*Continuing with Phase 7 (Flash Loans), Phase 8 (Advanced MEV), and remaining phases in next section...*
