# ArbEngine-Pro - Phase-by-Phase Upgrade Plan (Part 2)

## Continuation from Part 1

---

# PHASE 7: Flash Loans & Capital Efficiency ⚡
**Goal**: Execute larger arbitrages without requiring capital upfront

## 7.1 Flash Loan Integration Overview
**Impact**: 10-100x larger position sizes
**Location**: `crates/flash-loans/` (new crate)

### Supported Protocols:
- **Solend**: Most liquid, lower fees (0.003%)
- **MarginFi**: Alternative option
- **Kamino**: For specific token pairs

### Architecture:
```rust
// Flash loan execution flow:
// 1. Borrow tokens from protocol
// 2. Execute arbitrage swaps
// 3. Repay loan + fee
// 4. Keep profit
// All in ONE atomic transaction
```

---

## 7.2 Solend Flash Loan Implementation
**Location**: `crates/flash-loans/src/solend.rs`

### Implementation Steps:

1. **Create Flash Loan Crate**
```toml
# File: crates/flash-loans/Cargo.toml

[package]
name = "flash-loans"
version = "0.1.0"
edition = "2021"

[dependencies]
solana-sdk = "1.18"
solana-client = "1.18"
anchor-lang = "0.29"
spl-token = "4.0"
```

2. **Flash Loan Client**
```rust
// File: crates/flash-loans/src/solend.rs

use anchor_lang::prelude::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    transaction::Transaction,
};

pub struct SolendFlashLoan {
    program_id: Pubkey,
    lending_market: Pubkey,
}

impl SolendFlashLoan {
    pub const PROGRAM_ID: &'static str = "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo";
    pub const FEE_BPS: u64 = 3; // 0.03%
    
    pub fn new() -> Self {
        Self {
            program_id: Self::PROGRAM_ID.parse().unwrap(),
            lending_market: "4UpD2fh7xH3VP9QQaXtsS1YY3bxzWhtfpks7FatyKvdY".parse().unwrap(),
        }
    }
    
    /// Build flash loan instruction
    pub fn build_flash_borrow_ix(
        &self,
        amount: u64,
        source_liquidity: Pubkey,
        destination: Pubkey,
    ) -> Instruction {
        // Solend flash loan instruction
        let accounts = vec![
            AccountMeta::new(source_liquidity, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(self.lending_market, false),
            // ... other required accounts
        ];
        
        Instruction {
            program_id: self.program_id,
            accounts,
            data: self.encode_flash_borrow_data(amount),
        }
    }
    
    /// Build repay instruction
    pub fn build_flash_repay_ix(
        &self,
        amount: u64,
        source: Pubkey,
        destination_liquidity: Pubkey,
    ) -> Instruction {
        let repay_amount = self.calculate_repay_amount(amount);
        
        let accounts = vec![
            AccountMeta::new(source, false),
            AccountMeta::new(destination_liquidity, false),
            AccountMeta::new_readonly(self.lending_market, false),
        ];
        
        Instruction {
            program_id: self.program_id,
            accounts,
            data: self.encode_flash_repay_data(repay_amount),
        }
    }
    
    pub fn calculate_repay_amount(&self, borrowed: u64) -> u64 {
        borrowed + (borrowed * Self::FEE_BPS / 10000)
    }
}
```

3. **Flash Loan Transaction Builder**
```rust
// File: crates/flash-loans/src/builder.rs

pub struct FlashLoanTxBuilder {
    flash_loan_client: SolendFlashLoan,
    swap_instructions: Vec<Instruction>,
}

impl FlashLoanTxBuilder {
    /// Build complete flash loan transaction
    /// 
    /// Transaction structure:
    /// 1. Flash borrow tokens
    /// 2. Swap 1 (e.g., USDC -> SOL)
    /// 3. Swap 2 (e.g., SOL -> BONK)
    /// 4. Swap 3 (e.g., BONK -> USDC)
    /// 5. Flash repay
    pub fn build_transaction(
        &self,
        arbitrage: &Arbitrage,
        flash_amount: u64,
        user_pubkey: Pubkey,
    ) -> Result<Transaction> {
        let mut instructions = Vec::new();
        
        // 1. Compute budget (important!)
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)
        );
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_price(50_000) // 0.05 SOL priority fee
        );
        
        // 2. Create temp token account for borrowed funds
        let temp_account = Keypair::new();
        instructions.push(
            system_instruction::create_account(
                &user_pubkey,
                &temp_account.pubkey(),
                Rent::default().minimum_balance(165),
                165,
                &spl_token::id(),
            )
        );
        
        // 3. Flash borrow
        let source_liquidity = self.get_reserve_liquidity(&arbitrage.input_token)?;
        instructions.push(
            self.flash_loan_client.build_flash_borrow_ix(
                flash_amount,
                source_liquidity,
                temp_account.pubkey(),
            )
        );
        
        // 4. Add swap instructions (arbitrage execution)
        instructions.extend(self.swap_instructions.clone());
        
        // 5. Flash repay (borrow amount + fee)
        instructions.push(
            self.flash_loan_client.build_flash_repay_ix(
                flash_amount,
                temp_account.pubkey(),
                source_liquidity,
            )
        );
        
        // 6. Close temp account
        instructions.push(
            spl_token::instruction::close_account(
                &spl_token::id(),
                &temp_account.pubkey(),
                &user_pubkey,
                &user_pubkey,
                &[],
            )?
        );
        
        // Build transaction
        let recent_blockhash = self.get_recent_blockhash().await?;
        let mut tx = Transaction::new_with_payer(&instructions, Some(&user_pubkey));
        tx.sign(&[&self.payer, &temp_account], recent_blockhash);
        
        Ok(tx)
    }
    
    /// Calculate if flash loan is profitable after fees
    pub fn is_flash_loan_profitable(
        &self,
        arbitrage: &Arbitrage,
        flash_amount: u64,
    ) -> bool {
        let flash_fee = flash_amount * SolendFlashLoan::FEE_BPS / 10000;
        let expected_profit = arbitrage.profit_bps as f64 / 10000.0 * flash_amount as f64;
        let gas_estimate = 0.01 * 1_000_000_000.0; // ~0.01 SOL
        
        expected_profit > (flash_fee as f64 + gas_estimate)
    }
}
```

4. **Flash Loan Executor**
```rust
// File: crates/bot/src/executor/flash_executor.rs

pub struct FlashLoanExecutor {
    tx_builder: FlashLoanTxBuilder,
    rpc_client: RpcClient,
    max_flash_amount: u64,  // Max borrow limit
}

impl FlashLoanExecutor {
    pub async fn execute_flash_arbitrage(
        &self,
        opportunity: &Arbitrage,
    ) -> Result<TradeResult> {
        // Calculate optimal flash loan amount
        let flash_amount = self.calculate_optimal_flash_amount(opportunity)?;
        
        // Verify profitability including flash fees
        if !self.tx_builder.is_flash_loan_profitable(opportunity, flash_amount) {
            return Err("Not profitable after flash loan fees".into());
        }
        
        tracing::info!(
            flash_amount = flash_amount,
            expected_profit_after_fees = self.calculate_net_profit(opportunity, flash_amount),
            "Executing flash loan arbitrage"
        );
        
        // Build transaction
        let tx = self.tx_builder.build_transaction(
            opportunity,
            flash_amount,
            self.payer.pubkey(),
        )?;
        
        // Submit with high priority
        let signature = self.submit_with_retry(&tx).await?;
        
        // Wait for confirmation
        let result = self.wait_for_confirmation(&signature).await?;
        
        Ok(result)
    }
    
    fn calculate_optimal_flash_amount(&self, opp: &Arbitrage) -> Result<u64> {
        // Start with max available liquidity on smallest pool
        let min_liquidity = opp.get_min_pool_liquidity();
        
        // Cap at our max flash amount setting
        let capped = min_liquidity.min(self.max_flash_amount);
        
        // Ensure we can repay fee
        let with_fee = capped - (capped * SolendFlashLoan::FEE_BPS / 10000);
        
        Ok(with_fee)
    }
}
```

---

## 7.3 Flash Loan Safety Checks
**Impact**: Prevent failed flash loans (very expensive!)
**Location**: `crates/flash-loans/src/safety.rs`

### Implementation Steps:

1. **Pre-Flight Validation**
```rust
// File: crates/flash-loans/src/safety.rs

pub struct FlashLoanSafetyChecker {
    min_profit_threshold: f64,  // Minimum profit to justify flash loan
    max_compute_units: u32,     // Must fit in one transaction
}

impl FlashLoanSafetyChecker {
    pub async fn validate_flash_loan(
        &self,
        opportunity: &Arbitrage,
        flash_amount: u64,
    ) -> Result<ValidationResult> {
        let mut checks = Vec::new();
        
        // 1. Liquidity check
        if !self.check_sufficient_liquidity(opportunity, flash_amount).await? {
            checks.push("Insufficient liquidity in one or more pools".to_string());
        }
        
        // 2. Profitability check (including flash fee + gas)
        let net_profit = self.calculate_net_profit(opportunity, flash_amount);
        if net_profit < self.min_profit_threshold {
            checks.push(format!(
                "Net profit ${:.2} below threshold ${:.2}",
                net_profit, self.min_profit_threshold
            ));
        }
        
        // 3. Transaction size check
        let estimated_cu = self.estimate_compute_units(opportunity);
        if estimated_cu > self.max_compute_units {
            checks.push(format!(
                "Transaction too large: {} CU > {} CU limit",
                estimated_cu, self.max_compute_units
            ));
        }
        
        // 4. Slippage simulation
        let simulated_slippage = self.simulate_slippage(opportunity, flash_amount).await?;
        if simulated_slippage > opportunity.max_slippage_bps {
            checks.push(format!(
                "Simulated slippage {} bps exceeds max {} bps",
                simulated_slippage, opportunity.max_slippage_bps
            ));
        }
        
        // 5. Reserve health check (is Solend reserve healthy?)
        if !self.check_reserve_health(&opportunity.input_token).await? {
            checks.push("Lending reserve unhealthy or paused".to_string());
        }
        
        if checks.is_empty() {
            Ok(ValidationResult::Valid)
        } else {
            Ok(ValidationResult::Invalid(checks))
        }
    }
    
    async fn check_sufficient_liquidity(
        &self,
        opp: &Arbitrage,
        amount: u64,
    ) -> Result<bool> {
        // Check each pool in the path has enough liquidity
        for hop in &opp.path {
            let pool_liquidity = self.get_pool_liquidity(&hop.pool_address).await?;
            if pool_liquidity < amount {
                return Ok(false);
            }
        }
        Ok(true)
    }
    
    async fn simulate_slippage(
        &self,
        opp: &Arbitrage,
        amount: u64,
    ) -> Result<f64> {
        // Use pool math to simulate exact output
        let mut current_amount = amount;
        
        for hop in &opp.path {
            let pool_state = self.fetch_pool_state(&hop.pool_address).await?;
            current_amount = pool_state.calculate_output(current_amount);
        }
        
        let expected = amount as f64 * (1.0 + opp.profit_bps as f64 / 10000.0);
        let slippage_bps = ((expected - current_amount as f64) / expected * 10000.0).abs();
        
        Ok(slippage_bps)
    }
}

pub enum ValidationResult {
    Valid,
    Invalid(Vec<String>),
}
```

---

## 7.4 Flash Loan Monitoring & Metrics
**Location**: Integration with existing metrics

### Implementation Steps:

1. **Add Flash Loan Metrics**
```rust
// File: crates/bot/src/metrics/prometheus.rs (addition)

impl MetricsCollector {
    pub fn add_flash_loan_metrics(&mut self) -> Result<()> {
        self.flash_loans_attempted = IntCounter::new(
            "arb_flash_loans_attempted_total",
            "Total flash loan arbitrages attempted"
        )?;
        
        self.flash_loans_successful = IntCounter::new(
            "arb_flash_loans_successful_total",
            "Successful flash loan arbitrages"
        )?;
        
        self.flash_loan_amounts = Histogram::with_opts(
            HistogramOpts::new(
                "arb_flash_loan_amount_usd",
                "Flash loan amounts in USD"
            )
            .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0])
        )?;
        
        self.flash_loan_profit = Histogram::with_opts(
            HistogramOpts::new(
                "arb_flash_loan_profit_usd",
                "Profit from flash loan trades in USD"
            )
            .buckets(vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0])
        )?;
        
        Ok(())
    }
}
```

2. **Flash Loan Analytics Dashboard**
```sql
-- File: dashboard/queries/flash_loan_analytics.sql

-- Flash loan performance
SELECT 
    DATE(time) as day,
    COUNT(*) as total_trades,
    AVG(flash_amount_usd) as avg_loan_size,
    SUM(actual_profit) as total_profit,
    AVG(actual_profit) as avg_profit_per_trade
FROM trades
WHERE is_flash_loan = true
GROUP BY day
ORDER BY day DESC;

-- Success rate by loan size
SELECT 
    CASE 
        WHEN flash_amount_usd < 1000 THEN 'Small (<$1k)'
        WHEN flash_amount_usd < 10000 THEN 'Medium ($1-10k)'
        ELSE 'Large (>$10k)'
    END as size_category,
    COUNT(*) FILTER (WHERE status = 'success') * 100.0 / COUNT(*) as success_rate
FROM trades
WHERE is_flash_loan = true
GROUP BY size_category;
```

---

## Phase 7 Deliverables

### Flash Loan System:
- ✅ Solend integration
- ✅ Transaction builder for atomic execution
- ✅ Safety validation system
- ✅ Profitability calculator (including fees)
- ✅ Flash loan metrics

### Configuration:
```toml
# File: .env

[flash_loans]
ENABLE_FLASH_LOANS=false  # Start disabled!
FLASH_LOAN_PROVIDER=solend  # Options: solend, marginfi
MAX_FLASH_AMOUNT_USDC=10000  # Cap at $10k initially
MIN_FLASH_PROFIT_USD=5.0     # Minimum $5 profit after all fees

# Safety
FLASH_LOAN_DRY_RUN=true      # Test mode first!
REQUIRE_VALIDATION=true
MAX_COMPUTE_UNITS=1400000
```

### Testing Protocol (CRITICAL!):
```rust
// Test in this exact order:

// 1. Unit tests
cargo test -p flash-loans

// 2. Devnet testing
SOLANA_CLUSTER=devnet cargo run -p solana-arb-bot

// 3. Mainnet dry-run (no real execution)
FLASH_LOAN_DRY_RUN=true cargo run -p solana-arb-bot

// 4. Small amounts first
MAX_FLASH_AMOUNT_USDC=100 cargo run -p solana-arb-bot

// 5. Gradual increase
MAX_FLASH_AMOUNT_USDC=1000  # After 10+ successful trades
MAX_FLASH_AMOUNT_USDC=5000  # After 50+ successful trades
MAX_FLASH_AMOUNT_USDC=10000 # After 100+ successful trades
```

### Success Criteria:
- [ ] Successfully execute 10+ flash loan arbitrages on devnet
- [ ] 100% success rate in dry-run mode
- [ ] Net profit >$5 after fees for 80%+ of opportunities
- [ ] No failed flash loan transactions (these are expensive!)
- [ ] Transaction confirmation time <30 seconds
- [ ] Comprehensive monitoring in place

**⚠️ WARNING**: Flash loans are high-risk. Start small and test extensively!

---

# PHASE 8: Advanced Execution & MEV Optimization 🎯
**Goal**: Maximize transaction inclusion and minimize MEV risks

## 8.1 Advanced Jito Integration
**Impact**: Higher inclusion rate, MEV protection
**Location**: `crates/core/src/jito/`

### Implementation Steps:

1. **Multi-Validator Bundle Submission**
```rust
// File: crates/core/src/jito/multi_submit.rs

pub struct JitoMultiSubmitter {
    validators: Vec<JitoValidator>,
}

pub struct JitoValidator {
    url: String,
    pubkey: Pubkey,
    avg_response_time_ms: u64,
}

impl JitoMultiSubmitter {
    pub fn new() -> Self {
        Self {
            validators: vec![
                JitoValidator {
                    url: "https://mainnet.block-engine.jito.wtf".to_string(),
                    pubkey: "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh".parse().unwrap(),
                    avg_response_time_ms: 50,
                },
                JitoValidator {
                    url: "https://amsterdam.mainnet.block-engine.jito.wtf".to_string(),
                    pubkey: "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5".parse().unwrap(),
                    avg_response_time_ms: 80,
                },
                JitoValidator {
                    url: "https://frankfurt.mainnet.block-engine.jito.wtf".to_string(),
                    pubkey: "J1to3PQfXidUUhprQWgdKkQAMWPJAEqSJ7amkBDE9qhF".parse().unwrap(),
                    avg_response_time_ms: 100,
                },
            ],
        }
    }
    
    /// Submit bundle to all validators simultaneously
    pub async fn submit_bundle(&self, bundle: &Bundle) -> Result<BundleResult> {
        let mut handles = Vec::new();
        
        // Submit to all validators concurrently
        for validator in &self.validators {
            let bundle_clone = bundle.clone();
            let url = validator.url.clone();
            
            let handle = tokio::spawn(async move {
                Self::submit_to_validator(&url, &bundle_clone).await
            });
            
            handles.push((validator.pubkey, handle));
        }
        
        // Wait for first success or all failures
        let mut results = Vec::new();
        for (pubkey, handle) in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    tracing::info!(
                        validator = %pubkey,
                        "Bundle accepted by validator"
                    );
                    return Ok(result);  // Return on first success
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        validator = %pubkey,
                        error = %e,
                        "Bundle rejected by validator"
                    );
                    results.push(e);
                }
                Err(e) => {
                    tracing::error!(
                        validator = %pubkey,
                        error = %e,
                        "Failed to submit to validator"
                    );
                }
            }
        }
        
        Err(format!("All validators rejected bundle: {:?}", results).into())
    }
    
    async fn submit_to_validator(url: &str, bundle: &Bundle) -> Result<BundleResult> {
        let client = reqwest::Client::new();
        
        let response = client
            .post(format!("{}/api/v1/bundles", url))
            .json(&bundle)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(format!("HTTP {}: {}", response.status(), response.text().await?).into())
        }
    }
}
```

2. **Dynamic Tip Calculation**
```rust
// File: crates/core/src/jito/tip_calculator.rs

pub struct DynamicTipCalculator {
    base_tip_lamports: u64,
    historical_tips: VecDeque<TipRecord>,
}

pub struct TipRecord {
    timestamp: Instant,
    tip_amount: u64,
    was_included: bool,
    block_time: Duration,
}

impl DynamicTipCalculator {
    /// Calculate optimal tip based on expected profit and network conditions
    pub fn calculate_tip(&self, expected_profit_lamports: u64) -> u64 {
        // Start with percentage of expected profit
        let profit_based_tip = (expected_profit_lamports as f64 * 0.05) as u64; // 5% of profit
        
        // Adjust based on recent inclusion rates
        let recent_inclusion_rate = self.get_recent_inclusion_rate();
        let competition_multiplier = if recent_inclusion_rate < 0.5 {
            1.5  // Low inclusion rate = increase tips
        } else if recent_inclusion_rate > 0.8 {
            0.8  // High inclusion rate = can reduce tips
        } else {
            1.0
        };
        
        // Adjust based on time of day (higher competition during US/EU hours)
        let time_multiplier = self.get_time_of_day_multiplier();
        
        // Calculate final tip
        let calculated_tip = (profit_based_tip as f64 * competition_multiplier * time_multiplier) as u64;
        
        // Ensure minimum tip (10k lamports = ~0.00001 SOL)
        let min_tip = 10_000;
        calculated_tip.max(min_tip).max(self.base_tip_lamports)
    }
    
    fn get_recent_inclusion_rate(&self) -> f64 {
        let recent = self.historical_tips
            .iter()
            .rev()
            .take(20)
            .collect::<Vec<_>>();
        
        if recent.is_empty() {
            return 0.5; // Default to 50% if no history
        }
        
        let included = recent.iter().filter(|r| r.was_included).count();
        included as f64 / recent.len() as f64
    }
    
    fn get_time_of_day_multiplier(&self) -> f64 {
        let hour = chrono::Utc::now().hour();
        
        match hour {
            // High competition hours (US/EU overlap: 13:00-20:00 UTC)
            13..=20 => 1.3,
            // Medium competition (8:00-13:00 UTC)
            8..=12 => 1.1,
            // Low competition (21:00-07:00 UTC)
            _ => 0.9,
        }
    }
    
    pub fn record_result(&mut self, tip: u64, included: bool, block_time: Duration) {
        self.historical_tips.push_back(TipRecord {
            timestamp: Instant::now(),
            tip_amount: tip,
            was_included: included,
            block_time,
        });
        
        // Keep last 100 records
        if self.historical_tips.len() > 100 {
            self.historical_tips.pop_front();
        }
    }
}
```

3. **Bundle Simulation**
```rust
// File: crates/core/src/jito/simulator.rs

pub struct BundleSimulator {
    rpc_client: RpcClient,
}

impl BundleSimulator {
    /// Simulate bundle execution before submitting
    pub async fn simulate_bundle(&self, bundle: &Bundle) -> Result<SimulationResult> {
        let mut results = Vec::new();
        
        for tx in &bundle.transactions {
            // Simulate each transaction
            let sim_result = self.rpc_client
                .simulate_transaction(tx)
                .await?;
            
            if let Some(err) = sim_result.err {
                return Ok(SimulationResult::Failed {
                    error: err.to_string(),
                    failed_tx_index: results.len(),
                });
            }
            
            results.push(sim_result);
        }
        
        // Calculate total resources
        let total_compute_units: u64 = results
            .iter()
            .filter_map(|r| r.units_consumed)
            .sum();
        
        Ok(SimulationResult::Success {
            compute_units_used: total_compute_units,
            logs: results.into_iter().flat_map(|r| r.logs).flatten().collect(),
        })
    }
}

pub enum SimulationResult {
    Success {
        compute_units_used: u64,
        logs: Vec<String>,
    },
    Failed {
        error: String,
        failed_tx_index: usize,
    },
}
```

---

## 8.2 Address Lookup Tables (ALTs)
**Impact**: 40-60% smaller transactions
**Location**: `crates/core/src/alt/`

### Implementation Steps:

1. **ALT Manager**
```rust
// File: crates/core/src/alt/manager.rs

use solana_address_lookup_table_program as alt_program;

pub struct AltManager {
    rpc_client: RpcClient,
    alt_address: Option<Pubkey>,
    cached_addresses: Vec<Pubkey>,
}

impl AltManager {
    /// Create new ALT for frequently used addresses
    pub async fn create_lookup_table(&mut self, authority: &Keypair) -> Result<Pubkey> {
        let recent_slot = self.rpc_client.get_slot().await?;
        
        let (create_ix, alt_address) = alt_program::instruction::create_lookup_table(
            authority.pubkey(),
            authority.pubkey(),
            recent_slot,
        );
        
        // Submit creation transaction
        let tx = Transaction::new_signed_with_payer(
            &[create_ix],
            Some(&authority.pubkey()),
            &[authority],
            self.rpc_client.get_latest_blockhash().await?,
        );
        
        self.rpc_client.send_and_confirm_transaction(&tx).await?;
        
        tracing::info!(alt = %alt_address, "Created address lookup table");
        
        self.alt_address = Some(alt_address);
        Ok(alt_address)
    }
    
    /// Add frequently used addresses to ALT
    pub async fn populate_alt(&mut self, authority: &Keypair) -> Result<()> {
        let alt_address = self.alt_address.ok_or("ALT not created")?;
        
        // Common addresses to add:
        let addresses = vec![
            // Token program
            spl_token::id(),
            // Associated token program
            spl_associated_token_account::id(),
            // Common DEX programs
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".parse()?, // Raydium
            "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP".parse()?, // Orca
            "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".parse()?,  // Jupiter
            // Add your wallet's token accounts
            // ... etc
        ];
        
        // Add in batches of 20 (max per instruction)
        for chunk in addresses.chunks(20) {
            let extend_ix = alt_program::instruction::extend_lookup_table(
                alt_address,
                authority.pubkey(),
                Some(authority.pubkey()),
                chunk.to_vec(),
            );
            
            let tx = Transaction::new_signed_with_payer(
                &[extend_ix],
                Some(&authority.pubkey()),
                &[authority],
                self.rpc_client.get_latest_blockhash().await?,
            );
            
            self.rpc_client.send_and_confirm_transaction(&tx).await?;
        }
        
        self.cached_addresses.extend_from_slice(&addresses);
        
        tracing::info!(
            count = self.cached_addresses.len(),
            "Populated ALT with addresses"
        );
        
        Ok(())
    }
    
    /// Build transaction with ALT
    pub fn build_tx_with_alt(
        &self,
        instructions: Vec<Instruction>,
        payer: Pubkey,
    ) -> Result<VersionedTransaction> {
        let alt_address = self.alt_address.ok_or("ALT not created")?;
        
        // Create v0 message with ALT
        let message = v0::Message::try_compile(
            &payer,
            &instructions,
            &[alt_address],  // ALT addresses
            self.rpc_client.get_latest_blockhash().await?,
        )?;
        
        let versioned_tx = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: VersionedMessage::V0(message),
        };
        
        Ok(versioned_tx)
    }
}
```

2. **Integration with Executor**
```rust
// File: crates/bot/src/executor.rs

pub struct TransactionBuilder {
    alt_manager: AltManager,
    use_alt: bool,
}

impl TransactionBuilder {
    pub async fn build_arbitrage_tx(
        &self,
        opportunity: &Arbitrage,
    ) -> Result<VersionedTransaction> {
        let instructions = self.build_swap_instructions(opportunity)?;
        
        if self.use_alt && self.alt_manager.alt_address.is_some() {
            // Use ALT for smaller transaction
            self.alt_manager.build_tx_with_alt(instructions, self.payer.pubkey())
        } else {
            // Legacy transaction
            self.build_legacy_tx(instructions)
        }
    }
}
```

---

## 8.3 Compute Budget Optimization
**Impact**: Lower fees, faster execution
**Location**: `crates/core/src/compute/`

### Implementation Steps:

1. **Dynamic Compute Unit Calculator**
```rust
// File: crates/core/src/compute/calculator.rs

pub struct ComputeOptimizer {
    historical_usage: HashMap<String, Vec<u32>>, // Path -> CU usage
}

impl ComputeOptimizer {
    /// Calculate exact compute units needed
    pub fn calculate_compute_units(&self, path: &TradePath) -> u32 {
        let path_key = path.to_string();
        
        // Get historical average for this path
        if let Some(history) = self.historical_usage.get(&path_key) {
            let avg = history.iter().sum::<u32>() / history.len() as u32;
            let p95 = self.percentile(history, 0.95);
            
            // Use 95th percentile + 10% buffer
            (p95 as f64 * 1.1) as u32
        } else {
            // Conservative default for unknown paths
            let base_swap_cu = 150_000; // Per swap
            let num_swaps = path.hops.len();
            
            (base_swap_cu * num_swaps as u32) + 50_000 // + overhead
        }
    }
    
    /// Calculate optimal priority fee
    pub async fn calculate_priority_fee(&self, urgency: Urgency) -> u64 {
        // Get recent priority fees from network
        let recent_fees = self.fetch_recent_priority_fees().await?;
        
        match urgency {
            Urgency::Low => recent_fees.p50,      // Median
            Urgency::Medium => recent_fees.p75,   // 75th percentile
            Urgency::High => recent_fees.p90,     // 90th percentile
            Urgency::Critical => recent_fees.p95, // 95th percentile
        }
    }
    
    pub fn record_usage(&mut self, path: &str, units_used: u32) {
        self.historical_usage
            .entry(path.to_string())
            .or_insert_with(Vec::new)
            .push(units_used);
        
        // Keep last 50 records
        if let Some(history) = self.historical_usage.get_mut(path) {
            if history.len() > 50 {
                history.remove(0);
            }
        }
    }
}

pub enum Urgency {
    Low,      // Can wait
    Medium,   // Normal arbitrage
    High,     // Highly profitable
    Critical, // Time-sensitive or flash loan
}
```

2. **Smart Compute Budget Instructions**
```rust
// File: crates/bot/src/executor/compute.rs

pub fn add_compute_budget(
    instructions: &mut Vec<Instruction>,
    compute_units: u32,
    priority_fee_lamports: u64,
) {
    // 1. Set compute unit limit (exact amount needed)
    instructions.insert(0, 
        ComputeBudgetInstruction::set_compute_unit_limit(compute_units)
    );
    
    // 2. Set priority fee (micro-lamports per CU)
    let micro_lamports_per_cu = (priority_fee_lamports * 1_000_000) / compute_units as u64;
    instructions.insert(1,
        ComputeBudgetInstruction::set_compute_unit_price(micro_lamports_per_cu)
    );
}
```

---

## 8.4 Transaction Retry Logic
**Impact**: Higher success rate
**Location**: `crates/core/src/retry/`

### Implementation Steps:

1. **Exponential Backoff Retry**
```rust
// File: crates/core/src/retry/strategy.rs

pub struct RetryStrategy {
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl RetryStrategy {
    pub async fn execute_with_retry<F, T>(
        &self,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Pin<Box<dyn Future<Output = Result<T>> + Send>>,
    {
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < self.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    attempt += 1;
                    
                    if attempt < self.max_attempts {
                        // Exponential backoff: 100ms, 200ms, 400ms, 800ms...
                        let delay = self.calculate_delay(attempt);
                        
                        tracing::warn!(
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Operation failed, retrying..."
                        );
                        
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| "Max retries exceeded".into()))
    }
    
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.base_delay_ms * 2u64.pow(attempt - 1);
        Duration::from_millis(delay_ms.min(self.max_delay_ms))
    }
}
```

2. **Smart Transaction Replacement**
```rust
// File: crates/core/src/retry/replacement.rs

pub struct TransactionReplacer {
    priority_fee_increment: u64,
}

impl TransactionReplacer {
    /// Replace stuck transaction with higher priority fee
    pub async fn replace_transaction(
        &self,
        original_tx: &Transaction,
        current_attempt: u32,
    ) -> Result<Transaction> {
        let mut new_tx = original_tx.clone();
        
        // Increase priority fee by 50% each retry
        let multiplier = 1.5f64.powi(current_attempt as i32);
        let new_priority_fee = (self.priority_fee_increment as f64 * multiplier) as u64;
        
        // Rebuild compute budget instructions
        let mut instructions = new_tx.message.instructions.clone();
        
        // Update priority fee instruction
        if let Some(ix) = instructions.iter_mut()
            .find(|ix| ix.program_id == ComputeBudgetProgram::id()) 
        {
            *ix = ComputeBudgetInstruction::set_compute_unit_price(new_priority_fee);
        }
        
        // Get fresh blockhash
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
        new_tx.message.recent_blockhash = recent_blockhash;
        
        // Re-sign
        new_tx.sign(&[&self.payer], recent_blockhash);
        
        tracing::info!(
            attempt = current_attempt,
            new_priority_fee = new_priority_fee,
            "Replacing transaction with higher priority"
        );
        
        Ok(new_tx)
    }
}
```

---

## Phase 8 Deliverables

### Advanced Execution:
- ✅ Multi-validator Jito submission
- ✅ Dynamic tip calculation
- ✅ Bundle simulation
- ✅ Address Lookup Tables
- ✅ Compute budget optimization
- ✅ Retry logic with backoff

### Configuration:
```toml
# File: .env

[jito]
ENABLE_JITO=true
JITO_TIP_PERCENTAGE=5.0     # 5% of expected profit
JITO_MIN_TIP_LAMPORTS=10000
JITO_TIMEOUT_SECONDS=30
SUBMIT_TO_ALL_VALIDATORS=true

[compute]
USE_ADDRESS_LOOKUP_TABLES=true
OPTIMIZE_COMPUTE_UNITS=true
PRIORITY_FEE_STRATEGY=dynamic  # Options: fixed, dynamic

[retry]
MAX_RETRY_ATTEMPTS=3
BASE_RETRY_DELAY_MS=100
MAX_RETRY_DELAY_MS=5000
ENABLE_TX_REPLACEMENT=true
```

### Performance Targets:
- Transaction inclusion rate: >90%
- Average confirmation time: <5 seconds
- MEV protection: 100% (via Jito bundles)
- Transaction size reduction: >40% (with ALTs)
- Priority fee optimization: ±20% of optimal

### Success Criteria:
- [ ] Multi-validator submission working
- [ ] Dynamic tips yielding >85% inclusion rate
- [ ] ALTs created and populated
- [ ] Compute units optimized (±10% of actual usage)
- [ ] Retry logic handling temporary failures
- [ ] No MEV sandwich attacks observed

---

*Continuing with Phase 9 (DEX Integration) and Phase 10 (Multi-Strategy) in next section...*
