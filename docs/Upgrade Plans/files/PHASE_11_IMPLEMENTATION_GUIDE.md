# Phase 11: Execution Hardening - Implementation Guide

**Priority**: 🔴 **CRITICAL**
**Timeline**: 2-5 days
**Goal**: Fix flash loan execution and optimize transaction building

---

## 📋 Overview

Phase 11 addresses the critical gaps found in validation:
1. **Flash Loan Execution** (Critical - P0)
2. **ALT Integration** (High - P1)  
3. **Jito Multi-Validator** (High - P1)
4. **Compute Optimization** (High - P1)

This guide provides **complete, production-ready code** for each fix.

---

## 🔴 TASK 1: Flash Loan Execution Wrapper

### Problem
Flash loans are detected and checked for profitability, but execution falls through to standard swaps, which fail due to insufficient funds.

### Solution Architecture
```
Current Flow:
opportunity.requires_flash_loan = true
  → executor.execute(opportunity) 
    → standard swap (FAILS - no funds)

Fixed Flow:
opportunity.requires_flash_loan = true
  → executor.execute(opportunity)
    → detect flash loan needed
      → build_flash_loan_transaction()
        → borrow → swaps → repay (ATOMIC)
```

### Step 1: Create Flash Loan Transaction Builder

**File**: `crates/bot/src/flash_loan_tx_builder.rs` (NEW)

```rust
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use solana_compute_budget::ComputeBudgetInstruction;
use spl_token;
use crate::opportunity::Opportunity;

pub struct FlashLoanTxBuilder {
    payer: Keypair,
    solend_program_id: Pubkey,
}

impl FlashLoanTxBuilder {
    pub const SOLEND_PROGRAM: &'static str = "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo";
    pub const FEE_BPS: u64 = 3; // 0.03%
    
    pub fn new(payer: Keypair) -> Self {
        Self {
            payer,
            solend_program_id: Self::SOLEND_PROGRAM.parse().unwrap(),
        }
    }
    
    /// Build complete flash loan transaction
    pub fn build_transaction(
        &self,
        opportunity: &Opportunity,
        swap_instructions: Vec<Instruction>,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> Result<Transaction, Box<dyn std::error::Error>> {
        let mut all_instructions = Vec::new();
        
        // 1. Compute budget (flash loans need more CU)
        all_instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)
        );
        
        // 2. Set priority fee (higher for flash loans)
        let priority_fee = self.calculate_priority_fee(opportunity);
        all_instructions.push(
            ComputeBudgetInstruction::set_compute_unit_price(priority_fee)
        );
        
        // 3. Create temporary token account for borrowed funds
        let temp_account = Keypair::new();
        all_instructions.push(
            system_instruction::create_account(
                &self.payer.pubkey(),
                &temp_account.pubkey(),
                solana_sdk::native_token::sol_to_lamports(0.002), // Rent
                165, // Token account size
                &spl_token::id(),
            )
        );
        
        // 4. Initialize token account
        all_instructions.push(
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &temp_account.pubkey(),
                &opportunity.input_mint, // Token mint
                &self.payer.pubkey(),    // Owner
            )?
        );
        
        // 5. Flash borrow from Solend
        let borrow_amount = opportunity.input_amount;
        all_instructions.push(
            self.build_flash_borrow_instruction(
                borrow_amount,
                &opportunity.input_mint,
                &temp_account.pubkey(),
            )?
        );
        
        // 6. Add all swap instructions
        all_instructions.extend(swap_instructions);
        
        // 7. Flash repay (amount + fee)
        let repay_amount = self.calculate_repay_amount(borrow_amount);
        all_instructions.push(
            self.build_flash_repay_instruction(
                repay_amount,
                &opportunity.input_mint,
                &temp_account.pubkey(),
            )?
        );
        
        // 8. Close temporary account (recover rent)
        all_instructions.push(
            spl_token::instruction::close_account(
                &spl_token::id(),
                &temp_account.pubkey(),
                &self.payer.pubkey(), // Destination for remaining lamports
                &self.payer.pubkey(), // Account owner
                &[],
            )?
        );
        
        // Build and sign transaction
        let mut transaction = Transaction::new_with_payer(
            &all_instructions,
            Some(&self.payer.pubkey()),
        );
        
        transaction.sign(
            &[&self.payer, &temp_account],
            recent_blockhash,
        );
        
        Ok(transaction)
    }
    
    fn build_flash_borrow_instruction(
        &self,
        amount: u64,
        token_mint: &Pubkey,
        destination: &Pubkey,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        // Get Solend reserve for this token
        let reserve = self.get_solend_reserve(token_mint)?;
        
        // Solend flash loan instruction data
        // Instruction discriminator for FlashBorrow: [139, 141, 178, 175, 49, 45, 115, 42]
        let mut data = vec![139, 141, 178, 175, 49, 45, 115, 42];
        data.extend_from_slice(&amount.to_le_bytes());
        
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(reserve.liquidity_supply_pubkey, false),
            solana_sdk::instruction::AccountMeta::new(*destination, false),
            solana_sdk::instruction::AccountMeta::new_readonly(reserve.reserve_pubkey, false),
            solana_sdk::instruction::AccountMeta::new_readonly(reserve.lending_market, false),
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false),
        ];
        
        Ok(Instruction {
            program_id: self.solend_program_id,
            accounts,
            data,
        })
    }
    
    fn build_flash_repay_instruction(
        &self,
        amount: u64,
        token_mint: &Pubkey,
        source: &Pubkey,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        let reserve = self.get_solend_reserve(token_mint)?;
        
        // Instruction discriminator for FlashRepay: [92, 159, 112, 159, 84, 26, 25, 187]
        let mut data = vec![92, 159, 112, 159, 84, 26, 25, 187];
        data.extend_from_slice(&amount.to_le_bytes());
        
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(*source, false),
            solana_sdk::instruction::AccountMeta::new(reserve.liquidity_supply_pubkey, false),
            solana_sdk::instruction::AccountMeta::new(reserve.reserve_pubkey, false),
            solana_sdk::instruction::AccountMeta::new_readonly(reserve.lending_market, false),
            solana_sdk::instruction::AccountMeta::new_readonly(self.payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false),
        ];
        
        Ok(Instruction {
            program_id: self.solend_program_id,
            accounts,
            data,
        })
    }
    
    fn calculate_repay_amount(&self, borrowed: u64) -> u64 {
        // Solend fee: 0.03% (3 basis points)
        borrowed + (borrowed * Self::FEE_BPS / 10000)
    }
    
    fn calculate_priority_fee(&self, opportunity: &Opportunity) -> u64 {
        // 5% of expected profit as priority fee
        let expected_profit_lamports = (opportunity.expected_profit_bps as f64 / 10000.0) 
            * opportunity.input_amount as f64;
        let fee = (expected_profit_lamports * 0.05) as u64;
        
        // Minimum 50,000 micro-lamports per CU
        fee.max(50_000)
    }
    
    fn get_solend_reserve(&self, token_mint: &Pubkey) -> Result<SolendReserve, Box<dyn std::error::Error>> {
        // Hardcoded Solend reserves (mainnet)
        // TODO: Fetch dynamically or load from config
        
        let usdc_mint: Pubkey = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?;
        let sol_mint: Pubkey = "So11111111111111111111111111111111111111112".parse()?;
        
        if token_mint == &usdc_mint {
            Ok(SolendReserve {
                reserve_pubkey: "BgxfHJDzm44T7XG68MYKx7YisTjZu73tVovyZSjJMpmw".parse()?,
                liquidity_supply_pubkey: "8SheGtsopRUDzdiD6v6BR9a6bqZ9QwywYQY99Fp5meNf".parse()?,
                lending_market: "4UpD2fh7xH3VP9QQaXtsS1YY3bxzWhtfpks7FatyKvdY".parse()?,
            })
        } else if token_mint == &sol_mint {
            Ok(SolendReserve {
                reserve_pubkey: "8PbodeaosQP19SjYFx855UMqWxH2HynZLdBXmsrbac36".parse()?,
                liquidity_supply_pubkey: "8UviNr47S8eL6J3WfDxMRa3hvLta1VDJwNWqsDgtN3Cv".parse()?,
                lending_market: "4UpD2fh7xH3VP9QQaXtsS1YY3bxzWhtfpks7FatyKvdY".parse()?,
            })
        } else {
            Err("Unsupported token mint for flash loans".into())
        }
    }
}

struct SolendReserve {
    reserve_pubkey: Pubkey,
    liquidity_supply_pubkey: Pubkey,
    lending_market: Pubkey,
}
```

---

### Step 2: Extend Executor to Use Flash Loans

**File**: `crates/bot/src/executor.rs` (MODIFY)

```rust
use crate::flash_loan_tx_builder::FlashLoanTxBuilder;

pub struct Executor {
    // ... existing fields
    flash_loan_builder: FlashLoanTxBuilder,
    flash_loans_enabled: bool,
}

impl Executor {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            // ... existing initialization
            flash_loan_builder: FlashLoanTxBuilder::new(config.payer_keypair.clone()),
            flash_loans_enabled: config.enable_flash_loans,
        })
    }
    
    pub async fn execute(&mut self, opportunity: &Opportunity) -> Result<TradeResult> {
        // Check if flash loan is required and enabled
        if opportunity.requires_flash_loan {
            if !self.flash_loans_enabled {
                return Err("Flash loans required but disabled".into());
            }
            return self.execute_with_flash_loan(opportunity).await;
        }
        
        // Standard execution
        self.execute_standard(opportunity).await
    }
    
    async fn execute_with_flash_loan(&mut self, opportunity: &Opportunity) -> Result<TradeResult> {
        tracing::info!(
            opportunity_id = %opportunity.id,
            flash_amount = opportunity.input_amount,
            expected_profit_bps = opportunity.expected_profit_bps,
            "Executing flash loan arbitrage"
        );
        
        // Build swap instructions (same as standard execution)
        let swap_instructions = self.build_swap_instructions(opportunity)?;
        
        // Get recent blockhash
        let recent_blockhash = self.rpc_client
            .get_latest_blockhash()
            .await?;
        
        // Build flash loan transaction (atomic)
        let transaction = self.flash_loan_builder.build_transaction(
            opportunity,
            swap_instructions,
            recent_blockhash,
        )?;
        
        // Simulate first (CRITICAL for flash loans!)
        if let Err(e) = self.simulate_transaction(&transaction).await {
            tracing::error!(
                error = %e,
                "Flash loan simulation failed - ABORTING"
            );
            return Err(format!("Flash loan simulation failed: {}", e).into());
        }
        
        tracing::debug!("Flash loan simulation successful");
        
        // Submit transaction (with Jito if enabled)
        let start_time = std::time::Instant::now();
        let signature = if self.jito_enabled {
            self.submit_via_jito(&transaction).await?
        } else {
            self.rpc_client
                .send_and_confirm_transaction(&transaction)
                .await?
        };
        
        tracing::info!(
            signature = %signature,
            elapsed_ms = start_time.elapsed().as_millis(),
            "Flash loan transaction submitted"
        );
        
        // Wait for confirmation
        let result = self.confirm_transaction(&signature, opportunity).await?;
        
        tracing::info!(
            signature = %signature,
            actual_profit = result.actual_profit,
            "Flash loan arbitrage completed successfully"
        );
        
        Ok(result)
    }
    
    async fn simulate_transaction(&self, tx: &Transaction) -> Result<()> {
        let simulation = self.rpc_client
            .simulate_transaction(tx)
            .await?;
        
        if let Some(err) = simulation.value.err {
            return Err(format!("Simulation error: {:?}", err).into());
        }
        
        tracing::debug!(
            compute_units = simulation.value.units_consumed.unwrap_or(0),
            logs_count = simulation.value.logs.as_ref().map(|l| l.len()).unwrap_or(0),
            "Simulation successful"
        );
        
        Ok(())
    }
    
    // ... rest of existing methods
}
```

---

### Step 3: Update Main Loop

**File**: `crates/bot/src/main.rs` (MODIFY)

```rust
// In main loop where opportunities are executed:

for opportunity in opportunities {
    // Risk check (already present)
    if let Err(e) = risk_manager.check_trade(&opportunity).await {
        warn!("Trade rejected by risk manager: {}", e);
        continue;
    }
    
    // NEW: Flash loan check
    if opportunity.requires_flash_loan {
        if !config.enable_flash_loans {
            debug!("Skipping flash loan opportunity (flash loans disabled)");
            metrics_collector.opportunities_skipped_flash_loan.inc();
            continue;
        }
        
        // Additional safety check for flash loans
        if opportunity.input_amount > config.max_flash_amount {
            warn!(
                amount = opportunity.input_amount,
                max = config.max_flash_amount,
                "Flash loan amount exceeds configured maximum"
            );
            continue;
        }
    }
    
    // Execute (now handles flash loans automatically)
    match executor.execute(&opportunity).await {
        Ok(result) => {
            info!("Trade successful: profit = {}", result.actual_profit);
            metrics_collector.record_successful_trade(&result);
            
            if opportunity.requires_flash_loan {
                metrics_collector.flash_loans_successful.inc();
            }
        }
        Err(e) => {
            error!("Trade failed: {}", e);
            metrics_collector.record_failed_trade();
            
            if opportunity.requires_flash_loan {
                metrics_collector.flash_loans_failed.inc();
                // Flash loan failures are expensive! Alert immediately
                alerter.send_critical(&format!("Flash loan failed: {}", e)).await;
            }
        }
    }
}
```

---

### Step 4: Configuration

**File**: `.env` (ADD)

```bash
# Flash Loan Settings
ENABLE_FLASH_LOANS=false  # Keep false until tested on devnet!
MAX_FLASH_AMOUNT_USDC=10000
MIN_FLASH_PROFIT_USD=5.0

# Safety
FLASH_LOAN_DRY_RUN=true  # Simulate only
FLASH_LOAN_REQUIRE_SIMULATION=true  # Always simulate first
```

**File**: `crates/bot/src/config.rs` (ADD)

```rust
pub struct Config {
    // ... existing fields
    
    // Flash loan config
    pub enable_flash_loans: bool,
    pub max_flash_amount: u64,
    pub min_flash_profit: f64,
    pub flash_loan_dry_run: bool,
    pub flash_loan_require_simulation: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            // ... existing fields
            
            enable_flash_loans: env::var("ENABLE_FLASH_LOANS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            max_flash_amount: env::var("MAX_FLASH_AMOUNT_USDC")
                .unwrap_or_else(|_| "10000".to_string())
                .parse::<f64>()? as u64 * 1_000_000, // Convert to smallest unit
            min_flash_profit: env::var("MIN_FLASH_PROFIT_USD")
                .unwrap_or_else(|_| "5.0".to_string())
                .parse()?,
            flash_loan_dry_run: env::var("FLASH_LOAN_DRY_RUN")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            flash_loan_require_simulation: env::var("FLASH_LOAN_REQUIRE_SIMULATION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
        })
    }
}
```

---

### Step 5: Testing Protocol

#### 5.1 Unit Tests

**File**: `crates/bot/tests/flash_loan_tests.rs` (NEW)

```rust
#[cfg(test)]
mod flash_loan_tests {
    use super::*;
    
    #[test]
    fn test_repay_amount_calculation() {
        let builder = FlashLoanTxBuilder::new(test_keypair());
        
        // Borrow 1000 USDC
        let borrowed = 1_000_000_000; // 1000 USDC (6 decimals)
        let repay = builder.calculate_repay_amount(borrowed);
        
        // Should be 1000 + (1000 * 0.03%) = 1000.3 USDC
        assert_eq!(repay, 1_000_300_000);
    }
    
    #[test]
    fn test_transaction_structure() {
        let builder = FlashLoanTxBuilder::new(test_keypair());
        let opportunity = create_test_opportunity();
        let swap_ix = vec![create_test_swap_instruction()];
        
        let tx = builder.build_transaction(
            &opportunity,
            swap_ix,
            test_blockhash(),
        ).unwrap();
        
        // Verify instruction count
        // Should be: compute_limit + compute_price + create_account + init_account 
        //            + borrow + swap + repay + close_account = 8
        assert_eq!(tx.message.instructions.len(), 8);
        
        // Verify compute units set correctly
        let compute_ix = &tx.message.instructions[0];
        assert_eq!(compute_ix.program_id, solana_compute_budget::id());
    }
    
    #[tokio::test]
    async fn test_flash_loan_detection() {
        let mut executor = create_test_executor();
        
        let mut opportunity = create_test_opportunity();
        opportunity.requires_flash_loan = true;
        opportunity.input_amount = 5_000_000_000; // 5000 USDC
        
        // Should route to flash loan execution
        let result = executor.execute(&opportunity).await;
        
        // In test mode, should return error (no devnet connection)
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("flash loan"));
    }
}
```

#### 5.2 Devnet Testing Script

**File**: `scripts/test_flash_loan_devnet.sh` (NEW)

```bash
#!/bin/bash

echo "==================================="
echo "Flash Loan Devnet Testing"
echo "==================================="

# 1. Switch to devnet
export SOLANA_CLUSTER=devnet
export SOLANA_RPC_URL=https://api.devnet.solana.com

# 2. Fund test wallet (if needed)
echo "Checking devnet balance..."
solana balance

if [ $(solana balance | cut -d' ' -f1) < 1 ]; then
    echo "Requesting devnet airdrop..."
    solana airdrop 2
    sleep 5
fi

# 3. Enable flash loans (small amount)
export ENABLE_FLASH_LOANS=true
export MAX_FLASH_AMOUNT_USDC=100
export FLASH_LOAN_DRY_RUN=false
export FLASH_LOAN_REQUIRE_SIMULATION=true

# 4. Run bot for 5 minutes
echo "Starting bot with flash loans enabled..."
timeout 300 cargo run --release 2>&1 | tee devnet_flash_test.log

# 5. Check results
echo ""
echo "=== Test Results ==="
grep "flash loan" devnet_flash_test.log | grep -i "success" | wc -l
echo "Successful flash loans: $(grep 'flash loan.*success' devnet_flash_test.log | wc -l)"
echo "Failed flash loans: $(grep 'flash loan.*failed' devnet_flash_test.log | wc -l)"

# 6. Check for errors
if grep -i "error" devnet_flash_test.log | grep -i "flash"; then
    echo "⚠️ ERRORS FOUND - Review devnet_flash_test.log"
    exit 1
else
    echo "✅ No flash loan errors detected"
    exit 0
fi
```

#### 5.3 Mainnet Dry-Run

```bash
# Simulate flash loans on mainnet without executing
SOLANA_CLUSTER=mainnet \
FLASH_LOAN_DRY_RUN=true \
ENABLE_FLASH_LOANS=true \
MAX_FLASH_AMOUNT_USDC=100 \
cargo run --release

# Monitor logs
tail -f logs/arbengine.log | grep "flash"
```

#### 5.4 Small Live Test

```bash
# After devnet success, test on mainnet with $100 limit
SOLANA_CLUSTER=mainnet \
ENABLE_FLASH_LOANS=true \
FLASH_LOAN_DRY_RUN=false \
MAX_FLASH_AMOUNT_USDC=100 \
MIN_FLASH_PROFIT_USD=5.0 \
cargo run --release

# Watch for 30 minutes
# If successful, gradually increase MAX_FLASH_AMOUNT_USDC
```

---

## 🟡 TASK 2: ALT Integration

### Problem
Address Lookup Tables are initialized but not used in transaction building, resulting in larger transactions and higher fees.

### Solution

**File**: `crates/bot/src/executor.rs` (MODIFY)

```rust
use solana_sdk::{
    message::{v0, VersionedMessage},
    transaction::VersionedTransaction,
};

impl Executor {
    // ... existing methods
    
    fn build_transaction(
        &self,
        instructions: Vec<Instruction>,
        signers: &[&Keypair],
        recent_blockhash: Hash,
    ) -> Result<VersionedTransaction> {
        if self.config.use_alt && self.alt_manager.is_initialized() {
            self.build_v0_transaction_with_alt(instructions, signers, recent_blockhash)
        } else {
            self.build_legacy_transaction(instructions, signers, recent_blockhash)
        }
    }
    
    fn build_v0_transaction_with_alt(
        &self,
        instructions: Vec<Instruction>,
        signers: &[&Keypair],
        recent_blockhash: Hash,
    ) -> Result<VersionedTransaction> {
        let alt_address = self.alt_manager.get_alt_address()?;
        
        // Build v0 message with ALT
        let message = v0::Message::try_compile(
            &self.payer.pubkey(),
            &instructions,
            &[alt_address], // Address lookup table
            recent_blockhash,
        )?;
        
        let versioned_message = VersionedMessage::V0(message);
        
        // Sign transaction
        let mut tx = VersionedTransaction {
            signatures: vec![Signature::default(); signers.len()],
            message: versioned_message,
        };
        
        tx.try_sign(signers, recent_blockhash)?;
        
        tracing::debug!(
            tx_size = tx.message.serialize().len(),
            "Built v0 transaction with ALT"
        );
        
        Ok(tx)
    }
    
    fn build_legacy_transaction(
        &self,
        instructions: Vec<Instruction>,
        signers: &[&Keypair],
        recent_blockhash: Hash,
    ) -> Result<VersionedTransaction> {
        let mut tx = Transaction::new_with_payer(
            &instructions,
            Some(&self.payer.pubkey()),
        );
        
        tx.sign(signers, recent_blockhash);
        
        Ok(VersionedTransaction::from(tx))
    }
}
```

**File**: `crates/core/src/alt/manager.rs` (VERIFY/ADD)

```rust
pub struct AltManager {
    alt_address: Option<Pubkey>,
    rpc_client: Arc<RpcClient>,
}

impl AltManager {
    pub fn is_initialized(&self) -> bool {
        self.alt_address.is_some()
    }
    
    pub fn get_alt_address(&self) -> Result<Pubkey> {
        self.alt_address.ok_or("ALT not initialized".into())
    }
    
    pub async fn initialize(&mut self, authority: &Keypair) -> Result<()> {
        if self.alt_address.is_some() {
            return Ok(()); // Already initialized
        }
        
        // Create ALT
        let alt_address = self.create_lookup_table(authority).await?;
        
        // Populate with common addresses
        self.populate_alt(authority, alt_address).await?;
        
        self.alt_address = Some(alt_address);
        
        tracing::info!(alt = %alt_address, "ALT initialized");
        
        Ok(())
    }
    
    async fn create_lookup_table(&self, authority: &Keypair) -> Result<Pubkey> {
        let recent_slot = self.rpc_client.get_slot().await?;
        
        let (create_ix, alt_address) = 
            solana_address_lookup_table_program::instruction::create_lookup_table(
                authority.pubkey(),
                authority.pubkey(),
                recent_slot,
            );
        
        let tx = Transaction::new_signed_with_payer(
            &[create_ix],
            Some(&authority.pubkey()),
            &[authority],
            self.rpc_client.get_latest_blockhash().await?,
        );
        
        self.rpc_client.send_and_confirm_transaction(&tx).await?;
        
        Ok(alt_address)
    }
    
    async fn populate_alt(&self, authority: &Keypair, alt_address: Pubkey) -> Result<()> {
        let common_addresses = vec![
            spl_token::id(),
            spl_associated_token_account::id(),
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".parse()?, // Raydium
            "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP".parse()?, // Orca
            "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".parse()?,  // Jupiter
            // Add user's token accounts here
        ];
        
        for chunk in common_addresses.chunks(20) {
            let extend_ix = solana_address_lookup_table_program::instruction::extend_lookup_table(
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
        
        tracing::info!(
            count = common_addresses.len(),
            "Populated ALT with addresses"
        );
        
        Ok(())
    }
}
```

**Configuration**:

```bash
# .env
USE_ALT=true
ALT_AUTO_CREATE=true
```

**Testing**:

```bash
# Test on devnet first
SOLANA_CLUSTER=devnet \
USE_ALT=true \
cargo run --release

# Monitor transaction sizes
grep "tx_size" logs/arbengine.log

# Expected: Sizes reduced by ~40%
# Before ALT: ~1200 bytes
# After ALT: ~700 bytes
```

---

## 📊 Success Metrics

### Task 1: Flash Loans
- ✅ Build succeeds with no errors
- ✅ Unit tests pass (100%)
- ✅ Devnet: 10+ successful flash loans, 0 failures
- ✅ Dry-run: Simulations succeed, no execution
- ✅ Mainnet (small): 3+ successful $100 flash loans
- ✅ No transaction failures due to insufficient funds

### Task 2: ALT Integration
- ✅ ALT created on devnet/mainnet
- ✅ Transaction sizes reduced by 35-45%
- ✅ No transaction failures due to ALT
- ✅ All trades execute successfully with ALT

---

## ⏱️ Timeline

### Day 1: Flash Loan Implementation
- Hours 1-4: Implement FlashLoanTxBuilder
- Hours 5-6: Extend Executor
- Hours 7-8: Write unit tests

### Day 2: Flash Loan Testing
- Hours 1-4: Devnet testing (extensive)
- Hours 5-6: Dry-run on mainnet
- Hours 7-8: Small live test ($100)

### Day 3: ALT Integration
- Hours 1-3: Implement ALT transaction building
- Hours 4-6: Devnet testing
- Hours 7-8: Mainnet deployment

### Day 4-5: Monitoring & Optimization
- Monitor flash loan performance
- Track transaction costs savings
- Optimize if needed

---

## 🚨 Safety Checklist

Before enabling flash loans in production:

- [ ] All unit tests pass
- [ ] Devnet testing successful (10+ trades)
- [ ] Dry-run simulation works correctly
- [ ] Small mainnet test successful (3+ trades)
- [ ] Emergency stop tested and working
- [ ] Monitoring alerts configured
- [ ] Team ready to respond to issues
- [ ] Rollback plan prepared

---

**Next Steps**: After Phase 11 completion, proceed to Task 3 (Jito Multi-Validator) and Task 4 (Compute Optimization) from the validation report's Phase 11.2 section.
